// #![allow(dead_code)]
use crossbeam::queue::SegQueue;
use rfw::prelude::*;
use rfw::{ecs::Plugin, event::Events};
use std::{
    sync::{atomic::AtomicBool, Arc},
    thread::{self, JoinHandle},
};
use winit::window::Window;

mod types;
pub use types::*;
pub struct WindowPlugin {
    title: Option<String>,
    width: u32,
    height: u32,
    running_thread: Option<JoinHandle<()>>,
    window: Option<Arc<Window>>,
    kill_switch: Arc<AtomicBool>,
    pub(crate) incoming: Arc<SegQueue<WindowEvent>>,
}

enum WindowCommand {
    SetTitle(String),
}

impl WindowPlugin {
    pub fn new(width: u32, height: u32, title: String) -> Self {
        Self {
            title: Some(title),
            width,
            height,
            running_thread: None,
            window: None,
            kill_switch: Arc::new(AtomicBool::new(false)),
            incoming: Arc::new(SegQueue::new()),
        }
    }
}

impl Plugin for WindowPlugin {
    fn init(&mut self, instance: &mut rfw::Instance) {
        instance.add_resource(Events::<WindowEvent>::default());

        let title = self.title.clone().unwrap_or_default();
        let size = LogicalSize::new(self.width, self.height);
        let kill_switch = self.kill_switch.clone();
        let incoming = self.incoming.clone();

        let event_loop = winit::event_loop::EventLoop::new();
        let window = Arc::new(
            winit::window::WindowBuilder::new()
                .with_inner_size(size)
                .with_title(title)
                .build(&event_loop)
                .unwrap(),
        );
        self.window = Some(window.clone());

        self.running_thread = Some(thread::spawn(move || {
            event_loop.run(move |event, _, cf| {
                if kill_switch.load(std::sync::atomic::Ordering::SeqCst) {
                    *cf = ControlFlow::Exit;
                }

                match event {
                    winit::event::Event::WindowEvent { window_id, event }
                        if window.id() == window_id =>
                    {
                        incoming.push(WindowEvent::from(event));
                    }
                    winit::event::Event::Suspended => incoming.push(WindowEvent::Suspended),
                    winit::event::Event::Resumed => incoming.push(WindowEvent::Resumed),
                    winit::event::Event::RedrawRequested(_) => {
                        incoming.push(WindowEvent::RedrawRequested)
                    }
                    winit::event::Event::LoopDestroyed => {
                        incoming.push(WindowEvent::CloseRequested)
                    }
                    _ => {}
                }
            });
        }));
    }
}

fn process_window_events(
    plugin: Res<WindowPlugin>,
    mut system: ResMut<RenderSystem>,
    mut events: ResMut<Events<WindowEvent>>,
) {
    while let Some(event) = plugin.incoming.pop() {
        match &event {
            WindowEvent::Suspended => {}
            WindowEvent::Resumed => {}
            WindowEvent::RedrawRequested => {}
            WindowEvent::Resized(size) => system.resize(size.width, size.height, None),
            WindowEvent::Moved(_) => {}
            WindowEvent::CloseRequested => {}
            WindowEvent::Destroyed => {}
            WindowEvent::DroppedFile(_) => {}
            WindowEvent::HoveredFile(_) => {}
            WindowEvent::HoveredFileCancelled => {}
            WindowEvent::ReceivedCharacter(_) => {}
            WindowEvent::Focused(_) => {}
            WindowEvent::KeyboardInput {
                device_id,
                input,
                is_synthetic,
            } => {}
            WindowEvent::ModifiersChanged(_) => {}
            WindowEvent::CursorMoved {
                device_id,
                position,
            } => {}
            WindowEvent::CursorEntered { device_id } => {}
            WindowEvent::CursorLeft { device_id } => {}
            WindowEvent::MouseWheel {
                device_id,
                delta,
                phase,
            } => {}
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => {}
            WindowEvent::TouchpadPressure {
                device_id,
                pressure,
                stage,
            } => {}
            WindowEvent::AxisMotion {
                device_id,
                axis,
                value,
            } => {}
            WindowEvent::Touch(_) => {}
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                new_inner_size,
            } => {}
            WindowEvent::ThemeChanged(_) => {}
        }

        events.push(event);
    }
}
