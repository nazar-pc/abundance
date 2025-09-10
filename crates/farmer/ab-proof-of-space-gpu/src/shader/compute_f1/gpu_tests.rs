use crate::shader::compute_f1::cpu_tests::correct_compute_f1;
use crate::shader::{SHADER_U32, SHADER_U64};
use ab_chacha8::{ChaCha8Block, ChaCha8State};
use ab_core_primitives::pos::PosProof;
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
fn compute_f1_gpu() {
    let seed = [1; 32];
    let num_x = 100;

    // Calculate the necessary number of ChaCha8 blocks
    let keystream_length_blocks =
        (num_x * u32::from(PosProof::K)).div_ceil(size_of::<ChaCha8Block>() as u32 * u8::BITS);
    let initial_state = ChaCha8State::init(&seed, &[0; _]);

    let chacha8_keystream = (0..keystream_length_blocks)
        .map(|counter| initial_state.compute_block(counter))
        .collect::<Vec<_>>();

    let Some(actual_output) = block_on(compute_f1(chacha8_keystream.as_flattened(), num_x)) else {
        if cfg!(feature = "__force-gpu-tests") {
            panic!("Skipping tests, no compatible device detected");
        } else {
            eprintln!("Skipping tests, no compatible device detected");
            return;
        }
    };

    let expected_output = (0..num_x)
        .map(|x| correct_compute_f1::<{ PosProof::K }>(x, &seed))
        .collect::<Vec<_>>();

    assert_eq!(actual_output.len(), expected_output.len());
    for (x, (expected, actual)) in expected_output.into_iter().zip(actual_output).enumerate() {
        assert_eq!(expected, actual, "X={x}");
    }
}

async fn compute_f1(chacha8_keystream: &[u32], num_x: u32) -> Option<Vec<u32>> {
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

        let adapter_result = compute_f1_adapter(chacha8_keystream, num_x, adapter).await?;

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

async fn compute_f1_adapter(
    chacha8_keystream: &[u32],
    num_x: u32,
    adapter: Adapter,
) -> Option<Vec<u32>> {
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

    let ys_host = device.create_buffer(&BufferDescriptor {
        label: None,
        size: (size_of::<u32>() * num_x as usize) as BufferAddress,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let ys_gpu = device.create_buffer(&BufferDescriptor {
        label: None,
        size: ys_host.size(),
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
                resource: ys_gpu.as_entire_binding(),
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

    queue.submit([encoder.finish()]);

    ys_host.map_async(MapMode::Read, .., |r| r.unwrap());
    device.poll(PollType::Wait).unwrap();

    let ys = {
        let ys_host_ptr = ys_host.get_mapped_range(..).as_ptr().cast::<u32>();
        unsafe { slice::from_raw_parts(ys_host_ptr, num_x as usize) }.to_vec()
    };
    ys_host.unmap();

    Some(ys)
}
