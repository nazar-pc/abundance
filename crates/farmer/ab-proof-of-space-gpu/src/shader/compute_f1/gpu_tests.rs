use crate::shader::compute_f1::KEYSTREAM_LEN_WORDS;
use crate::shader::compute_f1::cpu_tests::correct_compute_f1;
use crate::shader::constants::{MAX_BUCKET_SIZE, MAX_TABLE_SIZE, NUM_BUCKETS, PARAM_BC};
use crate::shader::select_shader_features_limits;
use crate::shader::types::{PositionY, X};
use ab_chacha8::ChaCha8State;
use ab_core_primitives::pos::PosProof;
use futures::executor::block_on;
use std::slice;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    Adapter, BackendOptions, Backends, BindGroupDescriptor, BindGroupEntry,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferAddress, BufferBindingType,
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePipelineDescriptor,
    DeviceDescriptor, Instance, InstanceDescriptor, InstanceFlags, MapMode, MemoryBudgetThresholds,
    MemoryHints, PipelineLayoutDescriptor, PollType, ShaderStages, Trace,
};

#[test]
fn compute_f1_gpu() {
    let seed = [1; 32];

    let initial_state = ChaCha8State::init(&seed, &[0; _]);
    let mut chacha8_keystream =
        unsafe { Box::<[u32; KEYSTREAM_LEN_WORDS]>::new_zeroed().assume_init() };

    for (counter, block) in chacha8_keystream.as_chunks_mut().0.iter_mut().enumerate() {
        *block = initial_state.compute_block(counter as u32);
    }

    let Some(actual_output) = block_on(compute_f1(&chacha8_keystream)) else {
        if cfg!(feature = "__force-gpu-tests") {
            panic!("Skipping tests, no compatible device detected");
        } else {
            eprintln!("Skipping tests, no compatible device detected");
            return;
        }
    };

    let expected_output = (X::ZERO..)
        .take(MAX_TABLE_SIZE as usize)
        .map(|x| correct_compute_f1::<{ PosProof::K }>(x, &seed))
        .collect::<Vec<_>>();

    assert_eq!(
        actual_output
            .iter()
            .map(|bucket| bucket.len())
            .sum::<usize>(),
        MAX_TABLE_SIZE as usize
    );
    for (bucket_index, bucket) in actual_output.iter().enumerate() {
        for &PositionY { position, y } in bucket {
            let correct_bucket_index = (u32::from(y) / u32::from(PARAM_BC)) as usize;
            assert_eq!(
                bucket_index, correct_bucket_index,
                "position={position:?}, y={y:?}"
            );
            // TODO: This doesn't compile right now, but will be once this is resolved:
            //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
            // let expected_y = expected_output[usize::from(position)];
            let expected_y = expected_output[position as usize];
            assert_eq!(y, expected_y, "position={position:?}, y={y:?}");
        }
    }
}

async fn compute_f1(chacha8_keystream: &[u32; KEYSTREAM_LEN_WORDS]) -> Option<Vec<Vec<PositionY>>> {
    let backends = Backends::from_env().unwrap_or(Backends::METAL | Backends::VULKAN);
    let instance = Instance::new(&InstanceDescriptor {
        backends,
        flags: InstanceFlags::GPU_BASED_VALIDATION.with_env(),
        memory_budget_thresholds: MemoryBudgetThresholds::default(),
        backend_options: BackendOptions::from_env_or_default(),
    });

    let adapters = instance.enumerate_adapters(backends);
    let mut result = None::<Vec<Vec<PositionY>>>;

    for adapter in adapters {
        println!("Testing adapter {:?}", adapter.get_info());

        let adapter_result = compute_f1_adapter(chacha8_keystream, adapter).await?;

        match &result {
            Some(result) => {
                // Since output is non-deterministic here, sort buckets before comparing
                for (bucket_index, (result, adapter_result)) in
                    result.iter().zip(adapter_result).enumerate()
                {
                    let mut result = result.clone();
                    let mut adapter_result = adapter_result.clone();

                    result.sort();
                    adapter_result.sort();

                    assert!(result == adapter_result, "bucket_index={bucket_index}");
                }
            }
            None => {
                result.replace(adapter_result);
            }
        }
    }

    result
}

async fn compute_f1_adapter(
    chacha8_keystream: &[u32; KEYSTREAM_LEN_WORDS],
    adapter: Adapter,
) -> Option<Vec<Vec<PositionY>>> {
    let (shader, required_features, required_limits, _modern) =
        select_shader_features_limits(adapter.features());

    let (device, queue) = adapter
        .request_device(&DeviceDescriptor {
            label: None,
            required_features,
            required_limits,
            memory_hints: MemoryHints::Performance,
            trace: Trace::default(),
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
                    ty: BufferBindingType::Storage { read_only: true },
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
            BindGroupLayoutEntry {
                binding: 2,
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
        compilation_options: Default::default(),
        cache: None,
        label: None,
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: Some("compute_f1"),
    });

    let initial_state_gpu = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: unsafe {
            slice::from_raw_parts(
                chacha8_keystream.as_ptr().cast::<u8>(),
                size_of_val(chacha8_keystream),
            )
        },
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
    });

    let bucket_counts_host = device.create_buffer(&BufferDescriptor {
        label: None,
        size: size_of::<[u32; NUM_BUCKETS]>() as BufferAddress,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bucket_counts_gpu = device.create_buffer(&BufferDescriptor {
        label: None,
        size: bucket_counts_host.size(),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let buckets_host = device.create_buffer(&BufferDescriptor {
        label: None,
        size: size_of::<[[PositionY; MAX_BUCKET_SIZE]; NUM_BUCKETS]>() as BufferAddress,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let buckets_gpu = device.create_buffer(&BufferDescriptor {
        label: None,
        size: buckets_host.size(),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: initial_state_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: bucket_counts_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: buckets_gpu.as_entire_binding(),
            },
        ],
    });

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: None });

    {
        let mut cpass = encoder.begin_compute_pass(&Default::default());
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.set_pipeline(&compute_pipeline);
        cpass.dispatch_workgroups(device.limits().max_compute_workgroups_per_dimension, 1, 1);
    }

    encoder.copy_buffer_to_buffer(
        &bucket_counts_gpu,
        0,
        &bucket_counts_host,
        0,
        bucket_counts_host.size(),
    );
    encoder.copy_buffer_to_buffer(&buckets_gpu, 0, &buckets_host, 0, buckets_host.size());

    queue.submit([encoder.finish()]);

    bucket_counts_host.map_async(MapMode::Read, .., |r| r.unwrap());
    buckets_host.map_async(MapMode::Read, .., |r| r.unwrap());
    device.poll(PollType::Wait).unwrap();

    let buckets = {
        let bucket_counts_host_ptr = bucket_counts_host
            .get_mapped_range(..)
            .as_ptr()
            .cast::<[u32; NUM_BUCKETS]>();
        let bucket_counts = unsafe { &*bucket_counts_host_ptr };

        let buckets_host_ptr = buckets_host
            .get_mapped_range(..)
            .as_ptr()
            .cast::<[[PositionY; MAX_BUCKET_SIZE]; NUM_BUCKETS]>();
        let buckets = unsafe { &*buckets_host_ptr };

        buckets
            .iter()
            .zip(bucket_counts)
            .map(|(bucket, &bucket_count)| bucket[..bucket_count as usize].to_vec())
            .collect()
    };
    bucket_counts_host.unmap();
    buckets_host.unmap();

    Some(buckets)
}
