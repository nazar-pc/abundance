use crate::shader::{SHADER_U32, SHADER_U64};
use ab_chacha8::{ChaCha8Block, ChaCha8State, block_to_bytes, bytes_to_block};
use futures::executor::block_on;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    Adapter, BackendOptions, Backends, BindGroupDescriptor, BindGroupEntry,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferAddress, BufferBindingType,
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePipelineDescriptor,
    DeviceDescriptor, Features, Instance, InstanceDescriptor, InstanceFlags, Limits, MapMode,
    MemoryHints, PipelineLayoutDescriptor, PollType, ShaderStages, Trace,
};

#[test]
fn chacha8_keystream_gpu() {
    let seed = [1; 32];
    let num_blocks = 1024;

    let Some(actual_output) = block_on(chacha8_keystream_10_blocks(&seed, num_blocks)) else {
        if cfg!(feature = "__force-gpu-tests") {
            panic!("Skipping tests, no compatible device detected");
        } else {
            eprintln!("Skipping tests, no compatible device detected");
            return;
        }
    };

    let initial_state = ChaCha8State::init(&seed, &[0; _]);
    let expected_output = (0..num_blocks)
        .map(|counter| initial_state.compute_block(counter as u32))
        .collect::<Vec<_>>();

    assert_eq!(actual_output.len(), expected_output.len());
    for (counter, (actual, expected)) in actual_output.into_iter().zip(expected_output).enumerate()
    {
        assert_eq!(expected, actual, "Block #{counter}");
    }
}

async fn chacha8_keystream_10_blocks(
    seed: &[u8; 32],
    num_blocks: usize,
) -> Option<Vec<ChaCha8Block>> {
    let backends = Backends::from_env().unwrap_or(Backends::METAL | Backends::VULKAN);
    let instance = Instance::new(&InstanceDescriptor {
        backends,
        flags: InstanceFlags::GPU_BASED_VALIDATION.with_env(),
        backend_options: BackendOptions::from_env_or_default(),
    });

    let adapters = instance.enumerate_adapters(backends);
    let mut result = None;

    for adapter in adapters {
        println!("Testing adapter {:?}", adapter.get_info());

        let adapter_result = chacha8_keystream_10_blocks_adapter(seed, num_blocks, adapter).await?;

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

async fn chacha8_keystream_10_blocks_adapter(
    seed: &[u8; 32],
    num_blocks: usize,
    adapter: Adapter,
) -> Option<Vec<ChaCha8Block>> {
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
        entry_point: Some("chacha8_keystream"),
    });

    let initial_state = ChaCha8State::init(seed, &[0; _]);
    let initial_state = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: &block_to_bytes(&initial_state.to_repr()),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
    });

    let keystream_host = device.create_buffer(&BufferDescriptor {
        label: None,
        size: (size_of::<ChaCha8Block>() * num_blocks) as BufferAddress,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let keystream_gpu = device.create_buffer(&BufferDescriptor {
        label: None,
        size: (size_of::<ChaCha8Block>() * num_blocks) as BufferAddress,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: initial_state.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: keystream_gpu.as_entire_binding(),
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

    encoder.copy_buffer_to_buffer(&keystream_gpu, 0, &keystream_host, 0, keystream_host.size());

    queue.submit([encoder.finish()]);

    let keystream_host_slice = keystream_host.slice(..);
    keystream_host_slice.map_async(MapMode::Read, |r| r.unwrap());
    device.poll(PollType::Wait).unwrap();

    let keystream = keystream_host_slice
        .get_mapped_range()
        .array_chunks()
        .map(bytes_to_block)
        .collect();
    keystream_host.unmap();

    Some(keystream)
}
