pub mod ecs;
pub mod event;
pub mod input;
pub mod resources;
pub mod system;
pub mod window;
use std::time::Duration;

use event::Events;
pub use rfw_backend as backend;
pub use rfw_math as math;
pub use rfw_scene as scene;
pub use rfw_utils as utils;
use utils::Timer;
use window::{DeviceEvent, InputBundle, WindowEvent};

pub mod prelude {
    pub use crate::ecs::*;
    pub use crate::input;
    pub use crate::input::*;
    pub use crate::resources::*;
    pub use crate::system::*;

    pub use winit::event::{MouseButton, VirtualKeyCode};

    pub use rfw_backend::*;
    pub use rfw_math::*;
    pub use rfw_scene::bvh::*;
    pub use rfw_scene::*;
    pub use rfw_utils::collections::*;
    pub use rfw_utils::task::*;
    pub use rfw_utils::*;
}

use crate::ecs::component::Component;
use crate::ecs::schedule::SystemDescriptor;
use crate::resources::Resource;
use backend::FromWindowHandle;
use ecs::*;
use prelude::Input;
use rfw_backend::{Backend, RenderMode};
use rfw_math::*;
use rfw_scene::{Camera2D, Camera3D, Scene};
use system::RenderSystem;
use winit::event_loop::{ControlFlow, EventLoop};

pub struct Instance {
    scheduler: ecs::Scheduler,
    world: ecs::World,
    event_loop: EventLoop<()>,
    window: winit::window::Window,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GameTimer {
    start: Timer,
    frame: Timer,
    dt_duration: Duration,
    dt: f32,
}

impl GameTimer {
    fn reset_at_start(mut timer: ResMut<GameTimer>) {
        timer.dt = timer.frame.elapsed_in_millis();
        timer.dt_duration = timer.frame.elapsed();
        timer.frame.reset();
    }

    pub fn elapsed_ms_since_start(&self) -> f32 {
        self.start.elapsed_in_millis()
    }

    pub fn elapsed_since_start(&self) -> Duration {
        self.start.elapsed()
    }

    pub fn elapsed_ms(&self) -> f32 {
        self.dt
    }

    pub fn elapsed(&self) -> Duration {
        self.dt_duration
    }
}

impl Bundle for GameTimer {
    fn init(self, instance: &mut Instance) {
        instance
            .add_resource(GameTimer::default())
            .add_system_at_stage(CoreStage::PreUpdate, GameTimer::reset_at_start.system());
    }
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum ScaleMode {
    HiDpi,
    Regular,
    Custom(f64),
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct Settings {
    pub scale_mode: ScaleMode,
}

impl Instance {
    pub fn new<T: 'static + Backend + FromWindowHandle>(width: u32, height: u32) -> Self {
        env_logger::init();
        let event_loop = EventLoop::new();
        let window = winit::window::WindowBuilder::new()
            .with_inner_size(winit::dpi::LogicalSize::new(width, height))
            .with_title("rfw")
            .build(&event_loop)
            .expect("Could not create window.");
        let renderer = T::init(&window, width, height, 1.0).expect("Could not initialize renderer");

        let world = World::new();
        let scheduler = Scheduler::default();

        let mut this = Self {
            scheduler,
            world,
            event_loop,
            window,
        };

        this.add_plugin(RenderSystem {
            width,
            height,
            scale_factor: 1.0,
            renderer,
            mode: RenderMode::Default,
        })
        .add_resource(bevy_tasks::ComputeTaskPool(
            bevy_tasks::TaskPoolBuilder::new().build(),
        ))
        .add_resource(Scene::new())
        .add_resource(Camera3D::new().with_aspect_ratio(width as f32 / height as f32))
        .add_resource(Camera2D::from_width_height(width, height, None))
        .add_resource(Input::<winit::event::VirtualKeyCode>::new())
        .add_resource(Input::<winit::event::MouseButton>::new())
        .add_system_at_stage(ecs::CoreStage::PostUpdate, render_system.system());

        this
    }

    pub fn spawn(&mut self) -> ecs::world::EntityMut {
        self.world.spawn()
    }

    pub fn get_resource<T: Component>(&self) -> Option<&T> {
        self.world.get_resource()
    }

    pub fn get_resource_mut<T: Component>(&mut self) -> Option<Mut<T>> {
        self.world.get_resource_mut()
    }

    pub fn get_scene(&self) -> &Scene {
        self.world.get_resource::<Scene>().unwrap()
    }

    pub fn get_scene_mut(&mut self) -> ecs::Mut<Scene> {
        self.world.get_resource_mut().unwrap()
    }

    pub fn get_camera_2d(&mut self) -> ecs::Mut<Camera2D> {
        self.world.get_resource_mut().unwrap()
    }

    pub fn get_camera_3d(&mut self) -> ecs::Mut<Camera3D> {
        self.world.get_resource_mut().unwrap()
    }

    pub fn with_resource(mut self, resource: impl Component) -> Self {
        self.world.insert_resource(resource);
        self
    }

    pub fn add_resource(&mut self, resource: impl Component) -> &mut Self {
        self.world.insert_resource(resource);
        self
    }

    pub fn add_plugin<P: ecs::Plugin + Resource>(&mut self, mut plugin: P) -> &mut Self {
        plugin.init(self);
        self.world.insert_resource(plugin);
        self
    }

    pub fn with_plugin<P: ecs::Plugin + Resource>(mut self, mut plugin: P) -> Self {
        plugin.init(&mut self);
        self.world.insert_resource(plugin);
        self
    }

    pub fn add_bundle<P: ecs::Bundle + Resource>(&mut self, bundle: P) -> &mut Self {
        bundle.init(self);
        self
    }

    pub fn with_bundle<P: ecs::Bundle + Resource>(mut self, bundle: P) -> Self {
        bundle.init(&mut self);
        self
    }

    pub fn with_startup_system(mut self, system: impl Into<SystemDescriptor>) -> Self {
        self.scheduler
            .add_startup_system(StartupStage::Startup, system);
        self
    }

    pub fn with_system(mut self, system: impl Into<SystemDescriptor>) -> Self {
        self.scheduler.add_system(CoreStage::Update, system);
        self
    }

    pub fn with_system_at_stage(
        mut self,
        stage: impl StageLabel,
        system: impl Into<SystemDescriptor>,
    ) -> Self {
        self.scheduler.add_system(stage, system);
        self
    }

    pub fn with_system_set(mut self, system_set: SystemSet) -> Self {
        self.scheduler.add_system_set(CoreStage::Update, system_set);
        self
    }

    pub fn with_system_set_at_stage(
        mut self,
        stage: impl StageLabel,
        system_set: SystemSet,
    ) -> Self {
        self.scheduler.add_system_set(stage, system_set);
        self
    }

    pub fn add_startup_system(&mut self, system: impl Into<SystemDescriptor>) -> &mut Self {
        self.scheduler
            .add_startup_system(StartupStage::Startup, system);
        self
    }

    pub fn add_system(&mut self, system: impl Into<SystemDescriptor>) -> &mut Self {
        self.scheduler.add_system(CoreStage::Update, system);
        self
    }

    pub fn add_system_at_stage(
        &mut self,
        stage: impl StageLabel,
        system: impl Into<SystemDescriptor>,
    ) -> &mut Self {
        self.scheduler.add_system(stage, system);
        self
    }

    pub fn add_system_set(&mut self, system_set: SystemSet) -> &mut Self {
        self.scheduler.add_system_set(CoreStage::Update, system_set);
        self
    }

    pub fn add_system_set_at_stage(
        &mut self,
        stage: impl StageLabel,
        system_set: SystemSet,
    ) -> &mut Self {
        self.scheduler.add_system_set(stage, system_set);
        self
    }

    pub fn resize(&mut self, window_size: (u32, u32), scale_factor: Option<f64>) {
        let mut system = self.world.get_resource_mut::<RenderSystem>().unwrap();
        let scale_factor = scale_factor.unwrap_or(system.scale_factor);
        system.renderer.resize(window_size, scale_factor);
        system.width = window_size.0;
        system.height = window_size.1;
        system.scale_factor = scale_factor;
    }

    pub fn render(&mut self) {
        self.scheduler.run(&mut self.world);
    }

    #[cfg(feature = "serde")]
    pub fn save_scene<B: AsRef<Path>>(&self, path: B) -> Result<(), ()> {
        match self.scene.serialize(path) {
            Ok(_) => Ok(()),
            _ => Err(()),
        }
    }

    pub fn run(mut self, settings: Settings) {
        // TODO: we need to make a proper distinction between what plugins and bundles are, instead of this mess..
        self.add_bundle(Events::<WindowEvent>::new())
            .add_bundle(Events::<ResizeEvent>::new())
            .add_bundle(InputBundle {})
            .add_bundle(GameTimer::default());

        let (event_loop, window, mut world, mut scheduler) =
            (self.event_loop, self.window, self.world, self.scheduler);

        let mut scale: f64 = match settings.scale_mode {
            ScaleMode::HiDpi => 1.0,
            ScaleMode::Regular => window
                .current_monitor()
                .map(|m| 1.0 / m.scale_factor())
                .unwrap_or(1.0),
            ScaleMode::Custom(scale) => scale,
        };

        // Update scale with user preference.
        if let Some(mut events) = world.get_resource_mut::<Events<ResizeEvent>>() {
            let size = window.inner_size();
            let (width, height) = (size.width, size.height);

            events.push(ResizeEvent {
                width,
                height,
                scale,
            });
        }

        event_loop.run(move |event, _, cf| {
            *cf = ControlFlow::Poll;

            match event {
                winit::event::Event::WindowEvent { window_id, event }
                    if window.id() == window_id =>
                {
                    match &event {
                        winit::event::WindowEvent::CloseRequested => *cf = ControlFlow::Exit,
                        winit::event::WindowEvent::ScaleFactorChanged {
                            scale_factor,
                            new_inner_size,
                        } => {
                            scale = match settings.scale_mode {
                                ScaleMode::HiDpi => 1.0,
                                ScaleMode::Regular => 1.0 / scale_factor,
                                ScaleMode::Custom(scale) => scale,
                            };

                            if let Some(mut events) =
                                world.get_resource_mut::<Events<ResizeEvent>>()
                            {
                                events.push(ResizeEvent {
                                    width: new_inner_size.width,
                                    height: new_inner_size.height,
                                    scale: *scale_factor,
                                });
                            }
                        }
                        winit::event::WindowEvent::Resized(size) => {
                            if let Some(mut events) =
                                world.get_resource_mut::<Events<ResizeEvent>>()
                            {
                                events.push(ResizeEvent {
                                    width: size.width,
                                    height: size.height,
                                    scale,
                                });
                            }
                        }
                        _ => {}
                    }

                    if let Some(mut events) = world.get_resource_mut::<Events<WindowEvent>>() {
                        events.push(WindowEvent::from(event));
                    }
                }
                winit::event::Event::DeviceEvent { device_id, event } => {
                    if let Some(mut events) = world.get_resource_mut::<Events<DeviceEvent>>() {
                        events.push(DeviceEvent::from((device_id, event)));
                    }
                }
                winit::event::Event::Suspended => {
                    if let Some(mut events) = world.get_resource_mut::<Events<WindowEvent>>() {
                        events.push(WindowEvent::Suspended);
                    }
                }
                winit::event::Event::Resumed => {
                    if let Some(mut events) = world.get_resource_mut::<Events<WindowEvent>>() {
                        events.push(WindowEvent::Resumed);
                    }
                }
                winit::event::Event::RedrawRequested(window_id) if window_id == window.id() => {
                    scheduler.run(&mut world);
                }
                winit::event::Event::LoopDestroyed => *cf = ControlFlow::Exit,
                winit::event::Event::MainEventsCleared => window.request_redraw(),
                _ => {}
            };
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct ResizeEvent {
    width: u32,
    height: u32,
    scale: f64,
}

fn render_system(
    mut camera_2d: ResMut<Camera2D>,
    mut camera_3d: ResMut<Camera3D>,
    resize_event: Res<Events<ResizeEvent>>,
    mut system: ResMut<RenderSystem>,
) {
    if let Some(event) = resize_event.iter().last() {
        *camera_2d = Camera2D::from_width_height(event.width, event.height, Some(event.scale));
        camera_3d.set_aspect_ratio(event.width as f32 / event.height as f32);

        system.width = event.width;
        system.height = event.height;
        system.resize(event.width, event.height, Some(event.scale));
    }

    let view_2d = camera_2d.get_view();
    let view_3d = camera_3d.get_view(system.width, system.height);
    let mode = system.mode;
    system.renderer.render(view_2d, view_3d, mode);
}
