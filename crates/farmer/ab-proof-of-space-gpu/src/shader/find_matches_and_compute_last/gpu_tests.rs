use crate::shader::compute_fn::cpu_tests::random_metadata;
use crate::shader::constants::{
    MAX_BUCKET_SIZE, MAX_TABLE_SIZE, NUM_BUCKETS, NUM_MATCH_BUCKETS, NUM_S_BUCKETS, PARAM_BC,
    REDUCED_BUCKET_SIZE, REDUCED_MATCHES_COUNT,
};
use crate::shader::find_matches_and_compute_last::cpu_tests::find_matches_and_compute_last_correct;
use crate::shader::find_matches_and_compute_last::{NUM_ELEMENTS_PER_S_BUCKET, TABLE_NUMBER};
use crate::shader::find_matches_in_buckets::LeftTargets;
use crate::shader::find_matches_in_buckets::cpu_tests::calculate_left_targets;
use crate::shader::find_matches_in_buckets::rmap::Rmap;
use crate::shader::select_shader_features_limits;
use crate::shader::types::{Metadata, Position, PositionExt, PositionY, Y};
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
    PipelineLayoutDescriptor, PollType, ShaderStages,
};

#[test]
fn find_matches_and_compute_last_gpu() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());

    // Generate `y`s within `0..PARAM_BC*NUM_BUCKETS` range to fill the first `NUM_BUCKETS` buckets
    let parent_table_ys = (0..MAX_TABLE_SIZE)
        .map(|_| Y::from(rng.next_u32() % (PARAM_BC as u32 * NUM_BUCKETS as u32)))
        .collect::<Vec<_>>();
    let parent_buckets = {
        let mut buckets = unsafe {
            Box::<[[MaybeUninit<PositionY>; MAX_BUCKET_SIZE]; NUM_BUCKETS]>::new_uninit()
                .assume_init()
        };

        let mut bucket_offsets = [0_usize; NUM_BUCKETS];
        for (position, &y) in parent_table_ys.iter().enumerate() {
            let bucket_index = u32::from(y) / PARAM_BC as u32;
            let next_index = bucket_offsets[bucket_index as usize];
            if next_index < REDUCED_BUCKET_SIZE {
                buckets[bucket_index as usize][next_index].write(PositionY {
                    position: Position::from_u32(position as u32),
                    y,
                });
                bucket_offsets[bucket_index as usize] += 1;
            }
        }

        for (bucket, initialized) in buckets.iter_mut().zip(bucket_offsets) {
            bucket[initialized..].write_filled(PositionY {
                position: Position::SENTINEL,
                y: Y::SENTINEL,
            });
        }

        let ptr = Box::into_raw(buckets);

        unsafe { Box::from_raw(ptr.cast::<[[PositionY; MAX_BUCKET_SIZE]; NUM_BUCKETS]>()) }
    };
    let parent_metadatas = {
        let mut parent_metadatas = unsafe {
            Box::<[[MaybeUninit<Metadata>; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS]>::new_uninit()
                .assume_init()
        };
        for metadata in parent_metadatas.as_flattened_mut() {
            metadata.write(random_metadata::<TABLE_NUMBER>(&mut rng));
        }

        let ptr = Box::into_raw(parent_metadatas);

        unsafe {
            Box::from_raw(ptr.cast::<[[Metadata; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS]>())
        }
    };
    let left_targets = calculate_left_targets();

    let Some((actual_table_6_proof_target_counts, table_6_proof_targets)) = block_on(
        find_matches_and_compute_last(&left_targets, &parent_buckets, &parent_metadatas),
    ) else {
        if cfg!(feature = "__force-gpu-tests") {
            panic!("Skipping tests, no compatible device detected");
        } else {
            eprintln!("Skipping tests, no compatible device detected");
            return;
        }
    };

    let mut expected_table_6_proof_targets = unsafe {
        Box::<[[MaybeUninit<[Position; 2]>; NUM_ELEMENTS_PER_S_BUCKET]; NUM_S_BUCKETS]>::new_uninit(
        )
        .assume_init()
    };
    let expected_table_6_proof_targets = find_matches_and_compute_last_correct(
        &left_targets,
        &parent_buckets,
        &parent_metadatas,
        &mut expected_table_6_proof_targets,
    );

    for (bucket_index, (expected_bucket, (&actual_bucket_size, actual_bucket))) in
        expected_table_6_proof_targets
            .iter()
            .zip(
                actual_table_6_proof_target_counts
                    .iter()
                    .zip(table_6_proof_targets.iter()),
            )
            .enumerate()
    {
        let expected_bucket_size = expected_bucket
            .iter()
            .take_while(|entry| entry != &&[Position::SENTINEL; 2])
            .count();
        // Actual bucket size can be larger simply because GPU implementation is concurrent and
        // doesn't truncate buckets
        assert_eq!(
            expected_bucket_size as u32, actual_bucket_size,
            "bucket_index={bucket_index}"
        );

        let mut expected_bucket = expected_bucket[..expected_bucket_size].to_vec();
        expected_bucket.sort();

        for (index, (expected, actual)) in expected_bucket.iter().zip(actual_bucket).enumerate() {
            assert_eq!(
                expected, actual,
                "bucket_index={bucket_index}, index={index}"
            );
        }
    }
}

async fn find_matches_and_compute_last(
    left_targets: &LeftTargets,
    parent_buckets: &[[PositionY; MAX_BUCKET_SIZE]; NUM_BUCKETS],
    parent_metadatas: &[[Metadata; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS],
) -> Option<(
    Box<[u32; NUM_S_BUCKETS]>,
    Box<[[[Position; 2]; NUM_ELEMENTS_PER_S_BUCKET]; NUM_S_BUCKETS]>,
)> {
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

        let Some(mut adapter_result) = find_matches_and_compute_last_adapter(
            left_targets,
            parent_buckets,
            parent_metadatas,
            adapter,
        )
        .await
        else {
            continue;
        };

        // Fix up order within buckets, since sorting is a separate step on GPU and before that the
        // results are non-deterministic
        adapter_result
            .0
            .iter()
            .zip(adapter_result.1.iter_mut())
            .for_each(|(&bucket_size, bucket)| {
                bucket[..bucket_size as usize].sort();
            });

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

async fn find_matches_and_compute_last_adapter(
    left_targets: &LeftTargets,
    parent_buckets: &[[PositionY; MAX_BUCKET_SIZE]; NUM_BUCKETS],
    parent_metadatas: &[[Metadata; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS],
    adapter: Adapter,
) -> Option<(
    Box<[u32; NUM_S_BUCKETS]>,
    Box<[[[Position; 2]; NUM_ELEMENTS_PER_S_BUCKET]; NUM_S_BUCKETS]>,
)> {
    let (shader, required_features, required_limits, modern) =
        select_shader_features_limits(&adapter)?;
    println!("modern={modern}");

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
        entry_point: Some("find_matches_and_compute_last"),
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

    let parent_buckets_gpu = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: unsafe {
            slice::from_raw_parts(
                parent_buckets.as_ptr().cast::<u8>(),
                size_of_val(parent_buckets),
            )
        },
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
    });

    let parent_metadatas_gpu = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: unsafe {
            slice::from_raw_parts(
                parent_metadatas.as_ptr().cast::<u8>(),
                size_of_val(parent_metadatas),
            )
        },
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
    });

    let table_6_proof_target_counts_host = device.create_buffer(&BufferDescriptor {
        label: None,
        size: size_of::<[u32; NUM_S_BUCKETS]>() as BufferAddress,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let table_6_proof_target_counts_gpu = device.create_buffer(&BufferDescriptor {
        label: None,
        size: table_6_proof_target_counts_host.size(),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let table_6_proof_targets_host = device.create_buffer(&BufferDescriptor {
        label: None,
        size: size_of::<[[PositionY; NUM_ELEMENTS_PER_S_BUCKET]; NUM_S_BUCKETS]>() as BufferAddress,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let table_6_proof_targets_gpu = device.create_buffer(&BufferDescriptor {
        label: None,
        size: table_6_proof_targets_host.size(),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let rmap_gpu = device.create_buffer(&BufferDescriptor {
        label: None,
        size: if modern {
            // A dummy buffer is `4` byte just because it can't be zero in wgpu
            4
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
                resource: parent_buckets_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: parent_metadatas_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 3,
                resource: table_6_proof_target_counts_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 4,
                resource: table_6_proof_targets_gpu.as_entire_binding(),
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
        cpass.dispatch_workgroups(
            (NUM_MATCH_BUCKETS as u32).min(device.limits().max_compute_workgroups_per_dimension),
            1,
            1,
        );
    }

    encoder.copy_buffer_to_buffer(
        &table_6_proof_target_counts_gpu,
        0,
        &table_6_proof_target_counts_host,
        0,
        table_6_proof_target_counts_host.size(),
    );
    encoder.copy_buffer_to_buffer(
        &table_6_proof_targets_gpu,
        0,
        &table_6_proof_targets_host,
        0,
        table_6_proof_targets_host.size(),
    );

    encoder.map_buffer_on_submit(&table_6_proof_target_counts_host, MapMode::Read, .., |r| {
        r.unwrap()
    });
    encoder.map_buffer_on_submit(&table_6_proof_targets_host, MapMode::Read, .., |r| {
        r.unwrap()
    });

    queue.submit([encoder.finish()]);

    device.poll(PollType::wait_indefinitely()).unwrap();

    let table_6_proof_target_counts = {
        let table_6_proof_target_counts_host_ptr = table_6_proof_target_counts_host
            .get_mapped_range(..)
            .as_ptr()
            .cast::<[u32; NUM_S_BUCKETS]>();
        let table_6_proof_target_counts_ref = unsafe { &*table_6_proof_target_counts_host_ptr };

        let mut table_6_proof_target_counts =
            unsafe { Box::<[MaybeUninit<u32>; NUM_S_BUCKETS]>::new_uninit().assume_init() };
        table_6_proof_target_counts.write_copy_of_slice(table_6_proof_target_counts_ref);
        unsafe {
            let ptr = Box::into_raw(table_6_proof_target_counts);
            Box::from_raw(ptr.cast::<[u32; NUM_S_BUCKETS]>())
        }
    };
    let table_6_proof_targets = {
        let buckets_host_ptr = table_6_proof_targets_host
            .get_mapped_range(..)
            .as_ptr()
            .cast::<[[[Position; 2]; NUM_ELEMENTS_PER_S_BUCKET]; NUM_S_BUCKETS]>();
        let buckets_ref = unsafe { &*buckets_host_ptr };

        let mut table_6_proof_targets = unsafe {
            Box::<[MaybeUninit<[[Position; 2]; NUM_ELEMENTS_PER_S_BUCKET]>; NUM_S_BUCKETS]>::new_uninit()
                .assume_init()
        };
        table_6_proof_targets.write_copy_of_slice(buckets_ref);
        unsafe {
            let ptr = Box::into_raw(table_6_proof_targets);
            Box::from_raw(ptr.cast::<[[[Position; 2]; NUM_ELEMENTS_PER_S_BUCKET]; NUM_S_BUCKETS]>())
        }
    };
    table_6_proof_target_counts_host.unmap();
    table_6_proof_targets_host.unmap();

    Some((table_6_proof_target_counts, table_6_proof_targets))
}
