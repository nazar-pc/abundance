use crate::shader::compute_fn::WORKGROUP_SIZE;
use crate::shader::compute_fn::cpu_tests::{correct_compute_fn, random_metadata, random_y};
use crate::shader::constants::MAX_TABLE_SIZE;
use crate::shader::select_shader_features_limits;
use crate::shader::types::{Match, Metadata, Position, Y};
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{RngCore, SeedableRng};
use futures::executor::block_on;
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
fn compute_f2_gpu() {
    compute_fn_gpu::<2, 1>();
}

#[test]
fn compute_f3_gpu() {
    compute_fn_gpu::<3, 2>();
}

#[test]
fn compute_f4_gpu() {
    compute_fn_gpu::<4, 3>();
}

#[test]
fn compute_f5_gpu() {
    compute_fn_gpu::<5, 4>();
}

#[test]
fn compute_f6_gpu() {
    compute_fn_gpu::<6, 5>();
}

#[test]
fn compute_f7_gpu() {
    compute_fn_gpu::<7, 6>();
}

fn compute_fn_gpu<const TABLE_NUMBER: u8, const PARENT_TABLE_NUMBER: u8>() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let parent_table_size = 100_usize;
    let num_matches = 100_usize;

    let parent_ys = (0..parent_table_size)
        .map(|_| random_y(&mut rng))
        .collect::<Vec<_>>();
    let parent_metadatas = (0..parent_table_size)
        .map(|_| random_metadata::<PARENT_TABLE_NUMBER>(&mut rng))
        .collect::<Vec<_>>();
    let matches = (0..num_matches)
        .map(|_| {
            let left_position = Position::from(rng.next_u32() % parent_table_size as u32);
            let right_position = Position::from(rng.next_u32() % parent_table_size as u32);

            unsafe { Match::new(left_position, left_position, right_position) }
        })
        .collect::<Vec<_>>();

    let Some((actual_ys, actual_metadatas)) = block_on(compute_fn::<TABLE_NUMBER>(
        &matches,
        &parent_ys,
        &parent_metadatas,
    )) else {
        panic!("No compatible device detected, can't run tests");
    };

    let (expected_ys, expected_metadatas) = matches
        .iter()
        .map(|m| {
            // TODO: Correct version currently doesn't compile:
            //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
            // let left_metadata = parent_metadatas[usize::from(m.left_position())];
            // let right_metadata = parent_metadatas[usize::from(m.right_position())];
            let left_metadata = parent_metadatas[m.left_position() as usize];
            let right_metadata = parent_metadatas[m.right_position() as usize];
            correct_compute_fn::<TABLE_NUMBER, PARENT_TABLE_NUMBER>(
                parent_ys[m.bucket_offset() as usize],
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
    parent_ys: &[Y],
    parent_metadatas: &[Metadata],
) -> Option<(Vec<Y>, Vec<Metadata>)> {
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

        let Some(adapter_result) =
            compute_fn_adapter::<TABLE_NUMBER>(matches, parent_ys, parent_metadatas, adapter).await
        else {
            continue;
        };

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
    parent_ys: &[Y],
    parent_metadatas: &[Metadata],
    adapter: Adapter,
) -> Option<(Vec<Y>, Vec<Metadata>)> {
    let num_matches = matches.len();

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
        entry_point: Some(&format!("compute_f{TABLE_NUMBER}")),
    });

    let matches_gpu = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: unsafe {
            slice::from_raw_parts(matches.as_ptr().cast::<u8>(), size_of_val(matches))
        },
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
    });

    let parent_ys_gpu = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: unsafe {
            slice::from_raw_parts(parent_ys.as_ptr().cast::<u8>(), size_of_val(parent_ys))
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
        size: (size_of::<Y>() * num_matches) as BufferAddress,
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
        size: (size_of::<Metadata>() * num_matches) as BufferAddress,
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
                resource: parent_ys_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: parent_metadatas_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 3,
                resource: ys_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 4,
                resource: metadatas_gpu.as_entire_binding(),
            },
        ],
    });

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: None });

    {
        let mut cpass = encoder.begin_compute_pass(&Default::default());
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.set_pipeline(&compute_pipeline);
        cpass.dispatch_workgroups(MAX_TABLE_SIZE.div_ceil(WORKGROUP_SIZE), 1, 1);
    }

    encoder.copy_buffer_to_buffer(&ys_gpu, 0, &ys_host, 0, ys_host.size());
    encoder.copy_buffer_to_buffer(&metadatas_gpu, 0, &metadatas_host, 0, metadatas_host.size());

    encoder.map_buffer_on_submit(&ys_host, MapMode::Read, .., |r| r.unwrap());
    encoder.map_buffer_on_submit(&metadatas_host, MapMode::Read, .., |r| r.unwrap());

    queue.submit([encoder.finish()]);

    device.poll(PollType::wait_indefinitely()).unwrap();

    let ys = {
        let ys_host_ptr = ys_host.get_mapped_range(..).as_ptr().cast::<Y>();

        unsafe { slice::from_raw_parts(ys_host_ptr, num_matches) }.to_vec()
    };
    let metadatas = {
        let metadatas_host_ptr = metadatas_host
            .get_mapped_range(..)
            .as_ptr()
            .cast::<Metadata>();

        unsafe { slice::from_raw_parts(metadatas_host_ptr, num_matches) }.to_vec()
    };
    ys_host.unmap();
    metadatas_host.unmap();

    Some((ys, metadatas))
}
