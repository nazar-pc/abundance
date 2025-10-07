use crate::shader::compute_fn::cpu_tests::random_metadata;
use crate::shader::constants::{
    MAX_BUCKET_SIZE, MAX_TABLE_SIZE, NUM_BUCKETS, NUM_MATCH_BUCKETS, PARAM_BC, REDUCED_BUCKET_SIZE,
    REDUCED_MATCHES_COUNT,
};
use crate::shader::find_matches_and_compute_fn::cpu_tests::find_matches_and_compute_fn_correct;
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
    MemoryHints, PipelineLayoutDescriptor, PollType, ShaderStages, Trace,
};

#[test]
fn find_matches_and_compute_f2_gpu() {
    find_matches_and_compute_fn_gpu::<2, 1>();
}

#[test]
fn find_matches_and_compute_f3_gpu() {
    find_matches_and_compute_fn_gpu::<3, 2>();
}

#[test]
fn find_matches_and_compute_f4_gpu() {
    find_matches_and_compute_fn_gpu::<4, 3>();
}

#[test]
fn find_matches_and_compute_f5_gpu() {
    find_matches_and_compute_fn_gpu::<5, 4>();
}

#[test]
fn find_matches_and_compute_f6_gpu() {
    find_matches_and_compute_fn_gpu::<6, 5>();
}

#[test]
fn find_matches_and_compute_f7_gpu() {
    find_matches_and_compute_fn_gpu::<7, 6>();
}

fn find_matches_and_compute_fn_gpu<const TABLE_NUMBER: u8, const PARENT_TABLE_NUMBER: u8>() {
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

    let Some((actual_bucket_counts, actual_buckets, actual_positions, actual_metadatas)) =
        block_on(find_matches_and_compute_fn::<TABLE_NUMBER>(
            &left_targets,
            &parent_buckets,
            &parent_metadatas,
        ))
    else {
        if cfg!(feature = "__force-gpu-tests") {
            panic!("Skipping tests, no compatible device detected");
        } else {
            eprintln!("Skipping tests, no compatible device detected");
            return;
        }
    };

    let mut expected_buckets = unsafe {
        Box::<[[MaybeUninit<PositionY>; MAX_BUCKET_SIZE]; NUM_BUCKETS]>::new_uninit().assume_init()
    };
    let mut expected_positions = unsafe {
        Box::<[[MaybeUninit<[Position; 2]>; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS]>::new_uninit(
        )
        .assume_init()
    };
    let mut expected_metadatas = unsafe {
        Box::<[[MaybeUninit<Metadata>; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS]>::new_uninit()
            .assume_init()
    };
    let expected_buckets = find_matches_and_compute_fn_correct::<TABLE_NUMBER, PARENT_TABLE_NUMBER>(
        &left_targets,
        &parent_buckets,
        &parent_metadatas,
        &mut expected_buckets,
        &mut expected_positions,
        &mut expected_metadatas,
    );

    let expected_metadatas = expected_metadatas.as_flattened();
    let expected_positions = expected_positions.as_flattened();
    let actual_metadatas = actual_metadatas.as_flattened();
    let actual_positions = actual_positions.as_flattened();

    for (bucket_index, (expected_bucket, (&actual_bucket_size, actual_bucket))) in expected_buckets
        .iter()
        .zip(actual_bucket_counts.iter().zip(actual_buckets.iter()))
        .enumerate()
    {
        let expected_bucket_size = expected_bucket
            .iter()
            .take_while(|entry| entry.position != Position::SENTINEL)
            .count();
        // Actual bucket size can be larger simply because GPU implementation is concurrent and
        // doesn't truncate buckets
        assert!(
            expected_bucket_size as u32 <= actual_bucket_size,
            "bucket_index={bucket_index} expected_bucket_size={expected_bucket_size} <= \
            actual_bucket_size={actual_bucket_size}"
        );

        for (index, (expected, actual)) in expected_bucket[..expected_bucket_size]
            .iter()
            .zip(actual_bucket)
            .enumerate()
        {
            assert_eq!(
                expected.position, actual.position,
                "bucket_index={bucket_index}, index={index}"
            );
            let position = expected.position;
            if position != Position::SENTINEL {
                assert_eq!(
                    expected.y, actual.y,
                    "bucket_index={bucket_index}, index={index}"
                );

                assert_eq!(
                    unsafe { expected_metadatas[position as usize].assume_init() },
                    actual_metadatas[position as usize],
                    "bucket_index={bucket_index}, index={index}"
                );
                assert_eq!(
                    unsafe { expected_positions[position as usize].assume_init() },
                    actual_positions[position as usize],
                    "bucket_index={bucket_index}, index={index}"
                );
            }
        }
    }
}

async fn find_matches_and_compute_fn<const TABLE_NUMBER: u8>(
    left_targets: &LeftTargets,
    parent_buckets: &[[PositionY; MAX_BUCKET_SIZE]; NUM_BUCKETS],
    parent_metadatas: &[[Metadata; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS],
) -> Option<(
    Box<[u32; NUM_BUCKETS]>,
    Box<[[PositionY; MAX_BUCKET_SIZE]; NUM_BUCKETS]>,
    Box<[[[Position; 2]; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS]>,
    Box<[[Metadata; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS]>,
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

        let Some(mut adapter_result) = find_matches_and_compute_fn_adapter::<TABLE_NUMBER>(
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

async fn find_matches_and_compute_fn_adapter<const TABLE_NUMBER: u8>(
    left_targets: &LeftTargets,
    parent_buckets: &[[PositionY; MAX_BUCKET_SIZE]; NUM_BUCKETS],
    parent_metadatas: &[[Metadata; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS],
    adapter: Adapter,
) -> Option<(
    Box<[u32; NUM_BUCKETS]>,
    Box<[[PositionY; MAX_BUCKET_SIZE]; NUM_BUCKETS]>,
    Box<[[[Position; 2]; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS]>,
    Box<[[Metadata; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS]>,
)> {
    let (shader, required_features, required_limits, modern) =
        select_shader_features_limits(&adapter)?;
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
            BindGroupLayoutEntry {
                binding: 6,
                count: None,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    has_dynamic_offset: false,
                    min_binding_size: None,
                    ty: BufferBindingType::Storage { read_only: false },
                },
            },
            BindGroupLayoutEntry {
                binding: 7,
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
        entry_point: Some(&format!("find_matches_and_compute_f{TABLE_NUMBER}")),
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

    let positions_host = device.create_buffer(&BufferDescriptor {
        label: None,
        size: size_of::<[[[Position; 2]; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS]>()
            as BufferAddress,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let positions_gpu = device.create_buffer(&BufferDescriptor {
        label: None,
        size: positions_host.size(),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let metadatas_host = device.create_buffer(&BufferDescriptor {
        label: None,
        size: size_of::<[[Metadata; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS]>() as BufferAddress,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let metadatas_gpu = device.create_buffer(&BufferDescriptor {
        label: None,
        size: metadatas_host.size(),
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
                resource: parent_buckets_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: parent_metadatas_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 3,
                resource: bucket_counts_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 4,
                resource: buckets_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 5,
                resource: positions_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 6,
                resource: metadatas_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 7,
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
        &bucket_counts_gpu,
        0,
        &bucket_counts_host,
        0,
        bucket_counts_host.size(),
    );
    encoder.copy_buffer_to_buffer(&buckets_gpu, 0, &buckets_host, 0, buckets_host.size());
    encoder.copy_buffer_to_buffer(&positions_gpu, 0, &positions_host, 0, positions_host.size());
    encoder.copy_buffer_to_buffer(&metadatas_gpu, 0, &metadatas_host, 0, metadatas_host.size());

    queue.submit([encoder.finish()]);

    bucket_counts_host.map_async(MapMode::Read, .., |r| r.unwrap());
    buckets_host.map_async(MapMode::Read, .., |r| r.unwrap());
    positions_host.map_async(MapMode::Read, .., |r| r.unwrap());
    metadatas_host.map_async(MapMode::Read, .., |r| r.unwrap());
    device.poll(PollType::Wait).unwrap();

    let bucket_counts = {
        let bucket_counts_host_ptr = bucket_counts_host
            .get_mapped_range(..)
            .as_ptr()
            .cast::<[u32; NUM_BUCKETS]>();
        let bucket_counts_ref = unsafe { &*bucket_counts_host_ptr };

        let mut bucket_counts =
            unsafe { Box::<[MaybeUninit<u32>; NUM_BUCKETS]>::new_uninit().assume_init() };
        bucket_counts.write_copy_of_slice(bucket_counts_ref);
        unsafe {
            let ptr = Box::into_raw(bucket_counts);
            Box::from_raw(ptr.cast::<[u32; NUM_BUCKETS]>())
        }
    };
    let buckets = {
        let buckets_host_ptr = buckets_host
            .get_mapped_range(..)
            .as_ptr()
            .cast::<[[PositionY; MAX_BUCKET_SIZE]; NUM_BUCKETS]>();
        let buckets_ref = unsafe { &*buckets_host_ptr };

        let mut buckets = unsafe {
            Box::<[MaybeUninit<[PositionY; MAX_BUCKET_SIZE]>; NUM_BUCKETS]>::new_uninit()
                .assume_init()
        };
        buckets.write_copy_of_slice(buckets_ref);
        unsafe {
            let ptr = Box::into_raw(buckets);
            Box::from_raw(ptr.cast::<[[PositionY; MAX_BUCKET_SIZE]; NUM_BUCKETS]>())
        }
    };
    let positions = {
        let positions_host_ptr = positions_host
            .get_mapped_range(..)
            .as_ptr()
            .cast::<[[[Position; 2]; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS]>();
        let positions_ref = unsafe { &*positions_host_ptr };

        let mut positions = unsafe {
            Box::<[MaybeUninit<[[Position;2]; REDUCED_MATCHES_COUNT]>; NUM_MATCH_BUCKETS]>::new_uninit()
                .assume_init()
        };
        positions.write_copy_of_slice(positions_ref);
        unsafe {
            let ptr = Box::into_raw(positions);
            Box::from_raw(ptr.cast::<[[[Position; 2]; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS]>())
        }
    };
    let metadatas = {
        let metadatas_host_ptr = metadatas_host
            .get_mapped_range(..)
            .as_ptr()
            .cast::<[[Metadata; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS]>();
        let metadatas_ref = unsafe { &*metadatas_host_ptr };

        let mut metadatas = unsafe {
            Box::<[MaybeUninit<[Metadata; REDUCED_MATCHES_COUNT]>; NUM_MATCH_BUCKETS]>::new_uninit()
                .assume_init()
        };
        metadatas.write_copy_of_slice(metadatas_ref);
        unsafe {
            let ptr = Box::into_raw(metadatas);
            Box::from_raw(ptr.cast::<[[Metadata; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS]>())
        }
    };
    bucket_counts_host.unmap();
    buckets_host.unmap();
    positions_host.unmap();
    metadatas_host.unmap();

    Some((bucket_counts, buckets, positions, metadatas))
}
