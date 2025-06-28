use crate::shader::SHADER;
use crate::shader::compute_f1::cpu_tests::compute_f1;
use ab_chacha8::{ChaCha8Block, ChaCha8State};
use ab_core_primitives::pos::PosProof;
use futures::executor::block_on;
use spirv_std::glam::UVec2;
use std::slice;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    BackendOptions, Backends, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, BufferAddress, BufferBindingType, BufferDescriptor,
    BufferUsages, CommandEncoderDescriptor, ComputePipelineDescriptor, DeviceDescriptor, Features,
    Instance, InstanceDescriptor, InstanceFlags, Limits, MapMode, MemoryHints,
    PipelineLayoutDescriptor, PollType, ShaderStages, Trace,
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

    let Some(actual_output) = block_on(chacha8_keystream_10_blocks(
        chacha8_keystream.as_flattened(),
        num_x,
    )) else {
        if cfg!(feature = "__force-gpu-tests") {
            panic!("Skipping tests, no compatible device detected");
        } else {
            eprintln!("Skipping tests, no compatible device detected");
            return;
        }
    };

    let expected_output = (0..num_x)
        .map(|x| UVec2 {
            x,
            y: compute_f1::<{ PosProof::K }>(x, &seed),
        })
        .collect::<Vec<_>>();

    assert_eq!(actual_output.len(), expected_output.len());
    for (x, (actual, expected)) in actual_output.into_iter().zip(expected_output).enumerate() {
        assert_eq!(expected, actual, "X={x}");
    }
}

async fn chacha8_keystream_10_blocks(chacha8_keystream: &[u32], num_x: u32) -> Option<Vec<UVec2>> {
    let backends = Backends::from_env().unwrap_or(Backends::METAL | Backends::VULKAN);
    let instance = Instance::new(&InstanceDescriptor {
        backends,
        flags: InstanceFlags::GPU_BASED_VALIDATION.with_env(),
        backend_options: BackendOptions::from_env_or_default(),
    });

    let adapter = instance.enumerate_adapters(backends).into_iter().next()?;

    let (device, queue) = adapter
        .request_device(&DeviceDescriptor {
            label: None,
            required_features: Features::empty(),
            required_limits: Limits::default(),
            memory_hints: MemoryHints::Performance,
            trace: Trace::default(),
        })
        .await
        .unwrap();

    let module = device.create_shader_module(SHADER);

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

    let initial_state = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: unsafe {
            slice::from_raw_parts(
                chacha8_keystream.as_ptr().cast::<u8>(),
                size_of_val(chacha8_keystream),
            )
        },
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
    });

    let xys_host = device.create_buffer(&BufferDescriptor {
        label: None,
        size: (size_of::<UVec2>() * num_x as usize) as BufferAddress,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let xys_gpu = device.create_buffer(&BufferDescriptor {
        label: None,
        size: (size_of::<UVec2>() * num_x as usize) as BufferAddress,
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
                resource: xys_gpu.as_entire_binding(),
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

    encoder.copy_buffer_to_buffer(&xys_gpu, 0, &xys_host, 0, xys_host.size());

    queue.submit([encoder.finish()]);

    let keystream_host_slice = xys_host.slice(..);
    keystream_host_slice.map_async(MapMode::Read, |r| r.unwrap());
    device.poll(PollType::Wait).unwrap();

    let keystream = keystream_host_slice
        .get_mapped_range()
        .array_chunks::<{ size_of::<UVec2>() }>()
        .map(|xy| unsafe { xy.as_ptr().cast::<UVec2>().read_unaligned() })
        .collect();
    xys_host.unmap();

    Some(keystream)
}
