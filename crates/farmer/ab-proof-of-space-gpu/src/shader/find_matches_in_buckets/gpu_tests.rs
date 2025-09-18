use crate::shader::constants::{PARAM_BC, REDUCED_BUCKETS_SIZE, REDUCED_MATCHES_COUNT};
use crate::shader::find_matches_in_buckets::cpu_tests::{
    calculate_left_targets, find_matches_in_buckets_correct,
};
use crate::shader::find_matches_in_buckets::rmap::Rmap;
use crate::shader::find_matches_in_buckets::{LeftTargets, Match};
use crate::shader::select_shader_features_limits;
use crate::shader::types::{Position, PositionExt, Y};
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{RngCore, SeedableRng};
use futures::executor::block_on;
use std::mem::MaybeUninit;
use std::slice;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    Adapter, BackendOptions, Backends, BindGroupDescriptor, BindGroupEntry,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferAddress, BufferBindingType,
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePipelineDescriptor,
    DeviceDescriptor, Instance, InstanceDescriptor, InstanceFlags, MapMode, MemoryBudgetThresholds,
    MemoryHints, PipelineLayoutDescriptor, PollType, ShaderStages, Trace,
};

const NUM_BUCKETS: usize = 3;

#[test]
fn find_matches_in_buckets_gpu() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let parent_table_size = 1000_usize;

    // Generate `y`s within `0..PARAM_BC*NUM_BUCKETS` range to fill the first `NUM_BUCKETS` buckets
    let parent_table_ys = (0..parent_table_size)
        .map(|_| Y::from(rng.next_u32() % (PARAM_BC as u32 * NUM_BUCKETS as u32)))
        .collect::<Vec<_>>();
    let buckets = {
        let mut buckets = [[Position::SENTINEL; REDUCED_BUCKETS_SIZE]; 3];

        let mut total_found = [0_usize; 3];
        for (position, &y) in parent_table_ys.iter().enumerate() {
            let bucket_index = u32::from(y) / PARAM_BC as u32;
            let next_index = total_found[bucket_index as usize];
            if next_index < REDUCED_BUCKETS_SIZE {
                buckets[bucket_index as usize][next_index] = Position::from_u32(position as u32);
                total_found[bucket_index as usize] += 1;
            }
        }

        buckets
    };
    let left_targets = calculate_left_targets();

    let Some(actual_matches) = block_on(find_matches_in_buckets(
        &left_targets,
        &buckets,
        &parent_table_ys,
    )) else {
        if cfg!(feature = "__force-gpu-tests") {
            panic!("Skipping tests, no compatible device detected");
        } else {
            eprintln!("Skipping tests, no compatible device detected");
            return;
        }
    };

    let expected_matches = buckets
        .array_windows()
        .enumerate()
        .map(|(left_bucket_index, [left_bucket, right_bucket])| {
            let mut matches = [MaybeUninit::uninit(); _];
            unsafe {
                find_matches_in_buckets_correct(
                    left_bucket_index as u32,
                    left_bucket,
                    right_bucket,
                    &parent_table_ys,
                    &mut matches,
                    &left_targets,
                )
            }
            .to_vec()
        })
        .collect::<Vec<_>>();

    assert_eq!(actual_matches.len(), expected_matches.len());
    for (bucket_pair, (expected, actual)) in
        expected_matches.into_iter().zip(actual_matches).enumerate()
    {
        assert_eq!(expected.len(), actual.len(), "bucket_pair={bucket_pair}");
        for (index, (expected, actual)) in expected.into_iter().zip(actual).enumerate() {
            assert_eq!(expected, actual, "bucket_pair={bucket_pair}, index={index}");
        }
    }
}

async fn find_matches_in_buckets(
    left_targets: &LeftTargets,
    buckets: &[[Position; REDUCED_BUCKETS_SIZE]],
    parent_table_ys: &[Y],
) -> Option<Vec<Vec<Match>>> {
    let backends = Backends::from_env().unwrap_or(Backends::METAL | Backends::VULKAN);
    let instance = Instance::new(&InstanceDescriptor {
        backends,
        flags: InstanceFlags::GPU_BASED_VALIDATION.with_env(),
        memory_budget_thresholds: MemoryBudgetThresholds::default(),
        backend_options: BackendOptions::from_env_or_default(),
    });

    let adapters = instance.enumerate_adapters(backends);
    let mut result = None;

    for adapter in adapters {
        println!("Testing adapter {:?}", adapter.get_info());

        let adapter_result =
            find_matches_in_buckets_adapter(left_targets, buckets, parent_table_ys, adapter)
                .await?;

        match &result {
            Some(result) => {
                assert!(result == &adapter_result);
            }
            None => {
                result.replace(adapter_result);
            }
        }
    }

    result
}

async fn find_matches_in_buckets_adapter(
    left_targets: &LeftTargets,
    buckets: &[[Position; REDUCED_BUCKETS_SIZE]],
    parent_table_ys: &[Y],
    adapter: Adapter,
) -> Option<Vec<Vec<Match>>> {
    let num_bucket_pairs = buckets.len() - 1;

    let (shader, required_features, required_limits, modern) =
        select_shader_features_limits(adapter.features());
    println!("modern={modern}");

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
                    ty: BufferBindingType::Storage { read_only: true },
                },
            },
            BindGroupLayoutEntry {
                binding: 2,
                count: None,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    has_dynamic_offset: false,
                    min_binding_size: None,
                    ty: BufferBindingType::Storage { read_only: true },
                },
            },
            BindGroupLayoutEntry {
                binding: 3,
                count: None,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    has_dynamic_offset: false,
                    min_binding_size: None,
                    ty: BufferBindingType::Storage { read_only: false },
                },
            },
            BindGroupLayoutEntry {
                binding: 4,
                count: None,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    has_dynamic_offset: false,
                    min_binding_size: None,
                    ty: BufferBindingType::Storage { read_only: false },
                },
            },
            BindGroupLayoutEntry {
                binding: 5,
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
        entry_point: Some("find_matches_in_buckets"),
    });

    let left_targets_gpu = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: unsafe {
            slice::from_raw_parts(
                left_targets.as_ptr().cast::<u8>(),
                size_of_val(left_targets),
            )
        },
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
    });

    let buckets_gpu = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: unsafe {
            slice::from_raw_parts(buckets.as_ptr().cast::<u8>(), size_of_val(buckets))
        },
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
    });

    let parent_table_ys_gpu = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: unsafe {
            slice::from_raw_parts(
                parent_table_ys.as_ptr().cast::<u8>(),
                size_of_val(parent_table_ys),
            )
        },
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
    });

    let matches_host = device.create_buffer(&BufferDescriptor {
        label: None,
        size: (size_of::<[Match; REDUCED_MATCHES_COUNT]>() * num_bucket_pairs) as BufferAddress,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let matches_gpu = device.create_buffer(&BufferDescriptor {
        label: None,
        size: matches_host.size(),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let matches_counts_host = device.create_buffer(&BufferDescriptor {
        label: None,
        size: (size_of::<u32>() * num_bucket_pairs) as BufferAddress,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let matches_counts_gpu = device.create_buffer(&BufferDescriptor {
        label: None,
        size: matches_counts_host.size(),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let rmap_gpu = device.create_buffer(&BufferDescriptor {
        label: None,
        size: if modern {
            // A dummy buffer if `1` byte just because it can't be zero in wgpu
            1
        } else {
            size_of::<Rmap>() as BufferAddress
        },
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });

    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: left_targets_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: buckets_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: parent_table_ys_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 3,
                resource: matches_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 4,
                resource: matches_counts_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 5,
                resource: rmap_gpu.as_entire_binding(),
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

    encoder.copy_buffer_to_buffer(&matches_gpu, 0, &matches_host, 0, matches_host.size());
    encoder.copy_buffer_to_buffer(
        &matches_counts_gpu,
        0,
        &matches_counts_host,
        0,
        matches_counts_host.size(),
    );

    queue.submit([encoder.finish()]);

    matches_host.map_async(MapMode::Read, .., |r| r.unwrap());
    matches_counts_host.map_async(MapMode::Read, .., |r| r.unwrap());
    device.poll(PollType::Wait).unwrap();

    let matches = {
        let matches_host_ptr = matches_host
            .get_mapped_range(..)
            .as_ptr()
            .cast::<[Match; REDUCED_MATCHES_COUNT]>();
        let matches_counts_host_ptr = matches_counts_host
            .get_mapped_range(..)
            .as_ptr()
            .cast::<u32>();

        let matches = unsafe { slice::from_raw_parts(matches_host_ptr, num_bucket_pairs) };
        let matches_counts =
            unsafe { slice::from_raw_parts(matches_counts_host_ptr, num_bucket_pairs) };

        matches
            .iter()
            .zip(matches_counts)
            .map(|(matches, &matches_count)| matches[..matches_count as usize].to_vec())
            .collect()
    };
    matches_host.unmap();
    matches_counts_host.unmap();

    Some(matches)
}
