use crate::shader::compute_fn::Match;
use crate::shader::compute_fn::cpu_tests::{correct_compute_fn, random_metadata, random_y};
use crate::shader::{SHADER_U32, SHADER_U64};
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{RngCore, SeedableRng};
use futures::executor::block_on;
use std::slice;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    Adapter, BackendOptions, Backends, BindGroupDescriptor, BindGroupEntry,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferAddress, BufferBindingType,
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePipelineDescriptor,
    DeviceDescriptor, Features, Instance, InstanceDescriptor, InstanceFlags, Limits, MapMode,
    MemoryBudgetThresholds, MemoryHints, PipelineLayoutDescriptor, PollType, ShaderStages, Trace,
};

#[test]
fn compute_f2_gpu() {
    test_compute_fn_gpu_impl::<2, 1>(&mut ChaCha8Rng::from_seed(Default::default()));
}

#[test]
fn compute_f3_gpu() {
    test_compute_fn_gpu_impl::<3, 2>(&mut ChaCha8Rng::from_seed(Default::default()));
}

#[test]
fn compute_f4_gpu() {
    test_compute_fn_gpu_impl::<4, 3>(&mut ChaCha8Rng::from_seed(Default::default()));
}

#[test]
fn compute_f5_gpu() {
    test_compute_fn_gpu_impl::<5, 4>(&mut ChaCha8Rng::from_seed(Default::default()));
}

#[test]
fn compute_f6_gpu() {
    test_compute_fn_gpu_impl::<6, 5>(&mut ChaCha8Rng::from_seed(Default::default()));
}

#[test]
fn compute_f7_gpu() {
    test_compute_fn_gpu_impl::<7, 6>(&mut ChaCha8Rng::from_seed(Default::default()));
}

fn test_compute_fn_gpu_impl<const TABLE_NUMBER: u8, const PARENT_TABLE_NUMBER: u8>(
    rng: &mut ChaCha8Rng,
) {
    let parent_table_size = 100_usize;
    let num_matches = 100_usize;

    let parent_ys = (0..parent_table_size)
        .map(|_| random_y(rng))
        .collect::<Vec<_>>();
    let parent_metadatas = (0..parent_table_size)
        .map(|_| random_metadata::<PARENT_TABLE_NUMBER>(rng))
        .collect::<Vec<_>>();
    let matches = (0..num_matches)
        .map(|_| {
            let left_position = rng.next_u32() % parent_table_size as u32;
            let right_position = rng.next_u32() % parent_table_size as u32;

            Match {
                left_position,
                left_y: parent_ys[left_position as usize],
                right_position,
            }
        })
        .collect::<Vec<_>>();

    let Some((actual_ys, actual_metadatas)) =
        block_on(compute_fn::<TABLE_NUMBER>(&matches, &parent_metadatas))
    else {
        if cfg!(feature = "__force-gpu-tests") {
            panic!("Skipping tests, no compatible device detected");
        } else {
            eprintln!("Skipping tests, no compatible device detected");
            return;
        }
    };

    let (expected_ys, expected_metadatas) = matches
        .iter()
        .map(|m| {
            let left_metadata = parent_metadatas[m.left_position as usize];
            let right_metadata = parent_metadatas[m.right_position as usize];
            correct_compute_fn::<TABLE_NUMBER, PARENT_TABLE_NUMBER>(
                m.left_y,
                left_metadata,
                right_metadata,
            )
        })
        .unzip::<_, _, Vec<_>, Vec<_>>();

    assert_eq!(actual_ys.len(), expected_ys.len());
    assert_eq!(actual_metadatas.len(), expected_metadatas.len());
    for (index, (expected, actual)) in expected_ys
        .into_iter()
        .zip(expected_metadatas)
        .zip(actual_ys.into_iter().zip(actual_metadatas))
        .enumerate()
    {
        assert_eq!(expected, actual, "index={index}");
    }
}

async fn compute_fn<const TABLE_NUMBER: u8>(
    matches: &[Match],
    parent_metadatas: &[u128],
) -> Option<(Vec<u32>, Vec<u128>)> {
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
            compute_fn_adapter::<TABLE_NUMBER>(matches, parent_metadatas, adapter).await?;

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

async fn compute_fn_adapter<const TABLE_NUMBER: u8>(
    matches: &[Match],
    parent_metadatas: &[u128],
    adapter: Adapter,
) -> Option<(Vec<u32>, Vec<u128>)> {
    let num_matches = matches.len();

    let (required_features, shader) = if adapter.features().contains(Features::SHADER_INT64) {
        (Features::SHADER_INT64, SHADER_U64)
    } else {
        (Features::default(), SHADER_U32)
    };

    let (device, queue) = adapter
        .request_device(&DeviceDescriptor {
            label: None,
            required_features,
            required_limits: Limits::default(),
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
                    ty: BufferBindingType::Storage { read_only: false },
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
        entry_point: Some(&format!("compute_f{TABLE_NUMBER}")),
    });

    let matches_gpu = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: unsafe {
            slice::from_raw_parts(matches.as_ptr().cast::<u8>(), size_of_val(matches))
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

    let ys_host = device.create_buffer(&BufferDescriptor {
        label: None,
        size: (size_of::<u32>() * num_matches) as BufferAddress,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let ys_gpu = device.create_buffer(&BufferDescriptor {
        label: None,
        size: ys_host.size(),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let metadatas_host = device.create_buffer(&BufferDescriptor {
        label: None,
        size: (size_of::<u128>() * num_matches) as BufferAddress,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let metadatas_gpu = device.create_buffer(&BufferDescriptor {
        label: None,
        size: metadatas_host.size(),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: matches_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: parent_metadatas_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: ys_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 3,
                resource: metadatas_gpu.as_entire_binding(),
            },
        ],
    });

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: None });

    {
        let mut cpass = encoder.begin_compute_pass(&Default::default());
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.set_pipeline(&compute_pipeline);
        cpass.dispatch_workgroups(device.limits().max_compute_workgroup_size_x, 1, 1);
    }

    encoder.copy_buffer_to_buffer(&ys_gpu, 0, &ys_host, 0, ys_host.size());
    encoder.copy_buffer_to_buffer(&metadatas_gpu, 0, &metadatas_host, 0, metadatas_host.size());

    queue.submit([encoder.finish()]);

    ys_host.map_async(MapMode::Read, .., |r| r.unwrap());
    metadatas_host.map_async(MapMode::Read, .., |r| r.unwrap());
    device.poll(PollType::Wait).unwrap();

    let ys = {
        // Statically ensure it is aligned correctly
        const _: () = {
            assert!(align_of::<u32>() <= wgpu::MAP_ALIGNMENT as usize);
        };
        let ys_host_ptr = ys_host.get_mapped_range(..).as_ptr().cast::<u32>();
        // Assert just in case
        assert!(ys_host_ptr.is_aligned());

        unsafe { slice::from_raw_parts(ys_host_ptr, num_matches) }.to_vec()
    };
    let metadatas = {
        let metadatas_host_ptr = metadatas_host.get_mapped_range(..).as_ptr().cast::<u128>();
        // Alignment seems to always be larger than `u128`, so to simplify code, just assert and
        // move on
        assert!(
            metadatas_host_ptr.is_aligned(),
            "Alignment wasn't sufficient"
        );

        unsafe { slice::from_raw_parts(metadatas_host_ptr, num_matches) }.to_vec()
    };
    ys_host.unmap();
    metadatas_host.unmap();

    Some((ys, metadatas))
}
