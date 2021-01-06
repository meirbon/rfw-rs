use imgui::FontSource;
use imgui_wgpu::RendererConfig;
use std::time::Instant;
use winit::window::Window;

pub use imgui::Ui;

pub struct WgpuImGuiContext {
    pub(crate) context: imgui::Context,
    pub(crate) platform: imgui_winit_support::WinitPlatform,
    pub(crate) last_cursor: Option<Option<imgui::MouseCursor>>,
    pub(crate) renderer: imgui_wgpu::Renderer,
    pub(crate) last_frame: Instant,
    pub(crate) render: Option<wgpu::CommandBuffer>,
}

impl std::fmt::Debug for WgpuImGuiContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WgpuImGuiContext")
            .field("context", &self.context)
            .field("platform", &self.platform)
            .field("last_cursor", &self.last_cursor)
            .field("last_frame", &self.last_frame)
            .finish()
    }
}

impl WgpuImGuiContext {
    pub(crate) fn from_winit(window: &Window, device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let mut context = imgui::Context::create();
        let mut platform = imgui_winit_support::WinitPlatform::init(&mut context);

        platform.attach_window(
            context.io_mut(),
            window,
            imgui_winit_support::HiDpiMode::Default,
        );
        context.set_ini_filename(None);

        let font_size = (13.0 * window.scale_factor()) as f32;
        context.io_mut().font_global_scale = (1.0 / window.scale_factor()) as f32;

        context.fonts().add_font(&[FontSource::DefaultFontData {
            config: Some(imgui::FontConfig {
                oversample_h: 1,
                pixel_snap_h: true,
                size_pixels: font_size,
                ..Default::default()
            }),
        }]);

        let renderer_config = RendererConfig {
            texture_format: super::output::WgpuOutput::OUTPUT_FORMAT,
            depth_format: None,
            ..Default::default()
        };

        let renderer = imgui_wgpu::Renderer::new(&mut context, device, queue, renderer_config);

        Self {
            context,
            platform,
            last_cursor: None,
            renderer,
            last_frame: Instant::now(),
            render: None,
        }
    }

    pub fn update_ui<T: 'static>(&mut self, window: &Window, event: &winit::event::Event<T>) {
        self.platform
            .handle_event(self.context.io_mut(), window, event);
    }

    pub fn draw_ui<CB>(
        &mut self,
        window: &Window,
        mut draw: CB,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        attachment: wgpu::RenderPassColorAttachmentDescriptor,
    ) where
        CB: FnMut(&mut Ui<'_>),
    {
        let mut frame = self.context.frame();
        draw(&mut frame);

        if self.last_cursor != Some(frame.mouse_cursor()) {
            self.last_cursor = Some(frame.mouse_cursor());
            self.platform.prepare_render(&frame, window);
        }

        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[attachment],
                depth_stencil_attachment: None,
            });
            self.renderer
                .render(frame.render(), queue, device, &mut render_pass)
                .expect("Could not render imgui.");
        }

        self.render = Some(encoder.finish());
    }
}
