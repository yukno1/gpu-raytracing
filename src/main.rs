use {
    crate::render::PathTracer,
    anyhow::{Context, Ok, Result},
    std::sync::Arc,
    winit::{
        application::ApplicationHandler,
        event::{Event, WindowEvent},
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::{Window, WindowId},
    },
};

mod algebra;
mod camera;
mod render;

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    // device: Option<wgpu::Device>,
    // queue: Option<wgpu::Queue>,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    renderer: Option<PathTracer>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_size = winit::dpi::PhysicalSize::new(WIDTH, HEIGHT);
        let window_attributes = Window::default_attributes()
            .with_title("GPU Path Tracer")
            .with_inner_size(window_size);
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        let (device, queue, surface, config) = pollster::block_on(async {
            connect_to_gpu(&window)
                .await
                .expect("Failed to connect to GPU")
        });

        let mut renderer = render::PathTracer::new(device, queue, WIDTH, HEIGHT);

        self.window = Some(window);
        // self.device = Some(device);
        // self.queue = Some(queue);
        self.surface = Some(surface);
        self.surface_config = Some(config);
        self.renderer = Some(renderer);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.

                // Draw.

                // Queue a RedrawRequested event.
                //
                // You only need to call this if you've determined that you need to redraw in
                // applications which do not always need to. Applications that redraw continuously
                // can render here instead.
                // Wait for the next available frame buffer.
                let frame: wgpu::SurfaceTexture =
                    match self.surface.as_ref().unwrap().get_current_texture() {
                        // 1. 成功获取纹理，正常渲染
                        wgpu::CurrentSurfaceTexture::Success(texture) => texture,

                        // 2. 纹理可用，但表面状态不是最优（例如刚调整完窗口大小）。
                        // 稳妥的做法是丢弃它并重新配置，但为了简单，这里也可以直接渲染。
                        wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,

                        // 3. 表面已过期（通常是因为窗口大小改变，但还没触发 Resized 事件重新配置）。
                        // 必须重新配置 surface，然后跳过本帧。
                        wgpu::CurrentSurfaceTexture::Outdated => {
                            println!("Surface outdated, reconfiguring...");
                            // TODO: 在这里调用你的 surface.configure(...) 逻辑
                            return; // 跳过本帧渲染
                        }

                        // 4. 表面彻底丢失（例如设备断开或显存重置）。
                        // 需要重新创建 surface 并配置，然后跳过本帧。
                        wgpu::CurrentSurfaceTexture::Lost => {
                            println!("Surface lost, recreating...");
                            // TODO: 重新 instance.create_surface(...) 并 configure
                            return;
                        }

                        // 5. 获取超时 或 6. 窗口被完全遮挡（例如 macOS 上窗口被其他窗口盖住）。
                        // 此时无法渲染，直接跳过本帧即可。
                        wgpu::CurrentSurfaceTexture::Timeout
                        | wgpu::CurrentSurfaceTexture::Occluded => {
                            return; // 跳过本帧
                        }

                        // 7. 验证错误（通常意味着你的渲染代码有 bug，wgpu 会直接 panic）
                        wgpu::CurrentSurfaceTexture::Validation => {
                            unreachable!("Validation errors will panic");
                        }
                    };

                let render_target = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                self.renderer.as_mut().unwrap().render_frame(&render_target);

                frame.present();

                self.window.as_ref().unwrap().request_redraw();
            }
            _ => (),
        }
    }
}

fn main() -> Result<()> {
    let event_loop = EventLoop::new().unwrap();

    // ControlFlow::Poll continuously runs the event loop, even if the OS hasn't
    // dispatched any events. This is ideal for games and similar applications.
    event_loop.set_control_flow(ControlFlow::Poll);

    // ControlFlow::Wait pauses the event loop if no events are available to process.
    // This is ideal for non-game applications that only update in response to user
    // input, and uses significantly less power/CPU time than ControlFlow::Poll.
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();

    // run_app 会阻塞直到退出，返回 Result<(), EventLoopError>
    // 使用 context 将其转换为 anyhow::Result
    event_loop
        .run_app(&mut app)
        .context("Failed to run event loop")?;

    Ok(())
}

async fn connect_to_gpu(
    window: &Arc<Window>,
) -> Result<(
    wgpu::Device,
    wgpu::Queue,
    wgpu::Surface<'static>,
    wgpu::SurfaceConfiguration,
)> {
    use wgpu::TextureFormat::{Bgra8Unorm, Rgba8Unorm};

    // Create an "instance" of wgpu. This is the entry-point to the API.
    let instance = wgpu::Instance::default();

    // Create a drawable "surface" that is associated with the window.
    let surface = instance.create_surface(window.clone())?;

    // Request a GPU that is compatible with the surface. If the system has multiple GPUs then
    // pick the high performance one.
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })
        .await
        .context("failed to find a compatible adapter")?;

    // Connect to the GPU. "device" represents the connection to the GPU and allows us to create
    // resources like buffers, textures, and pipelines. "queue" represents the command queue that
    // we use to submit commands to the GPU.
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .context("failed to connect to the GPU")?;

    // Configure the texture memory backing the surface. Our renderer will draw to a surface
    // texture every frame.
    let caps = surface.get_capabilities(&adapter);
    let format = caps
        .formats
        .into_iter()
        .find(|it| matches!(it, Rgba8Unorm | Bgra8Unorm))
        .context("could not find preferred texture format (Rgba8Unorm or Bgra8Unorm)")?;

    let size = window.inner_size();
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::AutoVsync,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 3,
    };
    surface.configure(&device, &config);

    Ok((device, queue, surface, config))
}
