use crate::shader::constants::{
    K, MAX_BUCKET_SIZE, NUM_BUCKETS, PARAM_BC, REDUCED_BUCKET_SIZE, y_size_bits,
};
use crate::shader::find_matches_in_buckets::rmap::{Rmap, RmapBitPosition, RmapBitPositionExt};
use crate::shader::select_shader_features_limits;
use crate::shader::types::{Position, PositionExt, PositionR, Y};
use chacha20::ChaCha8Rng;
use futures::executor::block_on;
use rand::prelude::*;
use std::mem::MaybeUninit;
use std::slice;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    Adapter, BackendOptions, Backends, BindGroupDescriptor, BindGroupEntry,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferAddress, BufferBindingType,
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePipelineDescriptor,
    DeviceDescriptor, Instance, InstanceDescriptor, InstanceFlags, MapMode, MemoryBudgetThresholds,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PollType, ShaderStages,
};

#[test]
fn sort_buckets_with_rmap_details_gpu() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());

    let mut buckets =
        unsafe { Box::<[[PositionR; MAX_BUCKET_SIZE]; NUM_BUCKETS]>::new_zeroed().assume_init() };

    for (index, position_y) in buckets.as_flattened_mut().iter_mut().enumerate() {
        let y = Y::from(rng.next_u32() >> (u32::BITS - y_size_bits(K) as u32));
        let (_bucket_index, r) = y.into_bucket_index_and_r();
        *position_y = PositionR {
            position: Position::from_u32(index as u32),
            // Limit `y` to the appropriate number of bits
            r,
        };
    }

    buckets.as_flattened_mut().shuffle(&mut rng);

    let reduced_last_bucket_size = MAX_BUCKET_SIZE as u32 - 10;
    let mut bucket_sizes = Box::new([MAX_BUCKET_SIZE as u32; NUM_BUCKETS]);
    *bucket_sizes.last_mut().unwrap() = reduced_last_bucket_size;

    let Some(actual_output) = block_on(sort_buckets_with_rmap_details(&bucket_sizes, &buckets))
    else {
        panic!("No compatible device detected, can't run tests");
    };

    let mut expected_output = buckets;
    for entry in &mut expected_output.last_mut().unwrap()[reduced_last_bucket_size as usize..] {
        *entry = PositionR::SENTINEL;
    }
    for bucket in expected_output.iter_mut() {
        bucket.sort_by_key(|entry| entry.position);
        bucket[REDUCED_BUCKET_SIZE..].fill(PositionR::SENTINEL);
        bucket.sort_by_key(|position_r| (position_r.r, position_r.position));
        unsafe {
            Rmap::update_local_bucket_r_data(0, 1, bucket);
        }
        bucket.sort_by_key(|entry| entry.position);
    }

    for (bucket_index, (expected, actual)) in
        expected_output.iter().zip(actual_output.iter()).enumerate()
    {
        let mut rmap_expected = Rmap::new();
        for position_r in expected {
            if position_r.position == Position::SENTINEL {
                break;
            }

            unsafe {
                rmap_expected.add_with_data_parallel(position_r.r, position_r.position);
            }
        }
        let mut rmap_actual = Rmap::new();
        for position_r in actual {
            if position_r.position == Position::SENTINEL {
                break;
            }

            unsafe {
                rmap_actual.add_with_data_parallel(position_r.r, position_r.position);
            }
        }
        for r in 0..u32::from(PARAM_BC) {
            let rmap_bit_position = unsafe { RmapBitPosition::new(r) };
            assert_eq!(
                rmap_expected.get(rmap_bit_position),
                rmap_actual.get(rmap_bit_position),
                "bucket_index={bucket_index}, r={r:?}"
            );
        }
    }
}

async fn sort_buckets_with_rmap_details(
    bucket_sizes: &[u32; NUM_BUCKETS],
    buckets: &[[PositionR; MAX_BUCKET_SIZE]; NUM_BUCKETS],
) -> Option<Box<[[PositionR; MAX_BUCKET_SIZE]; NUM_BUCKETS]>> {
    let backends = Backends::from_env().unwrap_or(Backends::METAL | Backends::VULKAN);
    let instance = Instance::new(&InstanceDescriptor {
        backends,
        flags: InstanceFlags::GPU_BASED_VALIDATION.with_env(),
        memory_budget_thresholds: MemoryBudgetThresholds::default(),
        backend_options: BackendOptions::from_env_or_default(),
    });

    let adapters = instance.enumerate_adapters(backends);
    let mut result = None::<Box<[[PositionR; MAX_BUCKET_SIZE]; NUM_BUCKETS]>>;

    for adapter in adapters {
        println!("Testing adapter {:?}", adapter.get_info());

        let Some(adapter_result) =
            sort_buckets_with_rmap_details_adapter(bucket_sizes, buckets, adapter).await
        else {
            continue;
        };

        match &result {
            Some(result) => {
                for (bucket_index, (result, adapter_result)) in
                    result.iter().zip(adapter_result.iter()).enumerate()
                {
                    let mut rmap_previous = Rmap::new();
                    for position_r in adapter_result {
                        if position_r.position == Position::SENTINEL {
                            break;
                        }

                        unsafe {
                            rmap_previous.add_with_data_parallel(position_r.r, position_r.position);
                        }
                    }
                    let mut rmap_current = Rmap::new();
                    for position_r in result {
                        if position_r.position == Position::SENTINEL {
                            break;
                        }

                        unsafe {
                            rmap_current.add_with_data_parallel(position_r.r, position_r.position);
                        }
                    }
                    for r in 0..u32::from(PARAM_BC) {
                        let rmap_bit_position = unsafe { RmapBitPosition::new(r) };
                        assert_eq!(
                            rmap_previous.get(rmap_bit_position),
                            rmap_current.get(rmap_bit_position),
                            "bucket_index={bucket_index}, r={r:?}"
                        );
                    }
                }
            }
            None => {
                result.replace(adapter_result);
            }
        }
    }

    result
}

async fn sort_buckets_with_rmap_details_adapter(
    bucket_sizes: &[u32; NUM_BUCKETS],
    buckets: &[[PositionR; MAX_BUCKET_SIZE]; NUM_BUCKETS],
    adapter: Adapter,
) -> Option<Box<[[PositionR; MAX_BUCKET_SIZE]; NUM_BUCKETS]>> {
    // TODO: Test both versions of the shader here
    let (shader, required_features, required_limits, _modern) =
        select_shader_features_limits(&adapter)?;

    let (device, queue) = adapter
        .request_device(&DeviceDescriptor {
            label: None,
            required_features,
            required_limits,
            ..DeviceDescriptor::default()
        })
        .await
        .unwrap();

    let module = device.create_shader_module(shader);

    let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                count: None,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    has_dynamic_offset: false,
                    min_binding_size: None,
                    ty: BufferBindingType::Storage { read_only: false },
                },
            },
            BindGroupLayoutEntry {
                binding: 1,
                count: None,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    has_dynamic_offset: false,
                    min_binding_size: None,
                    ty: BufferBindingType::Storage { read_only: false },
                },
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
        compilation_options: PipelineCompilationOptions {
            constants: &[],
            zero_initialize_workgroup_memory: false,
        },
        cache: None,
        label: None,
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: Some("sort_buckets_with_rmap_details"),
    });

    let bucket_sizes_gpu = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: unsafe {
            slice::from_raw_parts(
                bucket_sizes.as_ptr().cast::<u8>(),
                size_of_val(bucket_sizes),
            )
        },
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
    });

    let buckets_host = device.create_buffer(&BufferDescriptor {
        label: None,
        size: size_of_val(buckets) as BufferAddress,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let buckets_gpu = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: unsafe {
            slice::from_raw_parts(buckets.as_ptr().cast::<u8>(), size_of_val(buckets))
        },
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
    });

    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: bucket_sizes_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: buckets_gpu.as_entire_binding(),
            },
        ],
    });

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: None });

    {
        let mut cpass = encoder.begin_compute_pass(&Default::default());
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.set_pipeline(&compute_pipeline);
        cpass.dispatch_workgroups(
            (NUM_BUCKETS as u32).min(device.limits().max_compute_workgroups_per_dimension),
            1,
            1,
        );
    }

    encoder.copy_buffer_to_buffer(&buckets_gpu, 0, &buckets_host, 0, buckets_host.size());

    encoder.map_buffer_on_submit(&buckets_host, MapMode::Read, .., |r| r.unwrap());

    queue.submit([encoder.finish()]);

    device.poll(PollType::wait_indefinitely()).unwrap();

    let buckets = {
        let buckets_host_ptr = buckets_host
            .get_mapped_range(..)
            .as_ptr()
            .cast::<[[PositionR; MAX_BUCKET_SIZE]; NUM_BUCKETS]>();
        let buckets_ref = unsafe { &*buckets_host_ptr };

        let mut buckets = unsafe {
            Box::<[MaybeUninit<[PositionR; MAX_BUCKET_SIZE]>; NUM_BUCKETS]>::new_uninit()
                .assume_init()
        };
        buckets.write_copy_of_slice(buckets_ref);
        unsafe {
            let ptr = Box::into_raw(buckets);
            Box::from_raw(ptr.cast::<[[PositionR; MAX_BUCKET_SIZE]; NUM_BUCKETS]>())
        }
    };
    buckets_host.unmap();

    Some(buckets)
}
