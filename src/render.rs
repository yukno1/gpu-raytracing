use std::{num::NonZero, sync::Arc};

pub struct PathTracer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
}

impl PathTracer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> PathTracer {
        device.on_uncaptured_error(Arc::new(|error| {
            panic!("Aborting due to an error: {}", error);
        }));

        let shader_module = compile_shader_module(&device);
        let pipeline = create_pipeline(&device, &shader_module);

        PathTracer {
            device,
            queue,
            pipeline,
        }
    }

    pub fn render_frame(&self, target: &wgpu::TextureView) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render frame"),
            });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("path tracer render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });

        render_pass.set_pipeline(&self.pipeline);

        // Draw 1 instance of a polygon with 6 vertices.
        render_pass.draw(0..6, 0..1);

        // End the render pass by consuming the object.
        drop(render_pass);

        let command_buffer = encoder.finish();
        self.queue.submit(Some(command_buffer));
    }
}

fn compile_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    use std::borrow::Cow;

    let code = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), r"\src\shaders.wgsl"));
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(code)),
    })
}

fn create_pipeline(
    device: &wgpu::Device,
    shader_module: &wgpu::ShaderModule,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("path tracer"),
        layout: None,
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            polygon_mode: wgpu::PolygonMode::Fill,
            ..Default::default()
        },
        vertex: wgpu::VertexState {
            module: shader_module,
            entry_point: Some("path_tracer_vs"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader_module,
            entry_point: Some("path_tracer_fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Bgra8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: NonZero::new(0 as u32),
        cache: None,
    })
}
