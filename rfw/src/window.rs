use crate::{
    event::Events,
    math::*,
    prelude::{CoreStage, Input},
};
use bevy_ecs::prelude::{IntoSystem, Res, ResMut};
use std::path::PathBuf;
pub use winit::event::DeviceId;
pub use winit::event::MouseButton;
pub use winit::event::ScanCode;
pub use winit::event::VirtualKeyCode;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{
        AxisId, ElementState, KeyboardInput, ModifiersState, MouseScrollDelta, Touch, TouchPhase,
    },
    window::Theme,
};

#[derive(Debug)]
pub struct InputBundle {}

impl InputBundle {
    fn keycode_system(
        events: Res<Events<WindowEvent>>,
        mut key_input: ResMut<Input<VirtualKeyCode>>,
    ) {
        for event in events.iter() {
            if let &WindowEvent::KeyboardInput { input, .. } = event {
                if let Some(code) = input.virtual_keycode {
                    match input.state {
                        ElementState::Pressed => {
                            key_input.insert(code, true);
                        }
                        ElementState::Released => {
                            key_input.insert(code, false);
                        }
                    }
                }
            }
        }
    }

    fn mousebutton_system(events: Res<Events<WindowEvent>>, mut input: ResMut<Input<MouseButton>>) {
        for event in events.iter() {
            if let &WindowEvent::MouseInput { state, button, .. } = event {
                match state {
                    ElementState::Pressed => {
                        input.insert(button, true);
                    }
                    ElementState::Released => {
                        input.insert(button, false);
                    }
                }
            }
        }
    }
}

impl crate::ecs::Bundle for InputBundle {
    fn init(self, instance: &mut crate::Instance) {
        instance.add_resource(crate::input::Input::<VirtualKeyCode>::new());
        instance.add_system_at_stage(CoreStage::PreUpdate, InputBundle::keycode_system.system());

        instance.add_resource(crate::input::Input::<MouseButton>::new());
        instance.add_system_at_stage(
            CoreStage::PreUpdate,
            InputBundle::mousebutton_system.system(),
        );
    }
}

#[derive(Debug, PartialEq)]
pub enum WindowEvent {
    Suspended,
    Resumed,
    RedrawRequested,
    /// The size of the window has changed. Contains the client area's new dimensions.
    Resized(PhysicalSize<u32>),

    /// The position of the window has changed. Contains the window's new position.
    Moved(PhysicalPosition<i32>),

    /// The window has been requested to close.
    CloseRequested,

    /// The window has been destroyed.
    Destroyed,

    /// A file has been dropped into the window.
    ///
    /// When the user drops multiple files at once, this event will be emitted for each file
    /// separately.
    DroppedFile(PathBuf),

    /// A file is being hovered over the window.
    ///
    /// When the user hovers multiple files at once, this event will be emitted for each file
    /// separately.
    HoveredFile(PathBuf),

    /// A file was hovered, but has exited the window.
    ///
    /// There will be a single `HoveredFileCancelled` event triggered even if multiple files were
    /// hovered.
    HoveredFileCancelled,

    /// The window received a unicode character.
    ReceivedCharacter(char),

    /// The window gained or lost focus.
    ///
    /// The parameter is true if the window has gained focus, and false if it has lost focus.
    Focused(bool),

    /// An event from the keyboard has been received.
    KeyboardInput {
        device_id: DeviceId,
        input: KeyboardInput,
        /// If `true`, the event was generated synthetically by winit
        /// in one of the following circumstances:
        ///
        /// * Synthetic key press events are generated for all keys pressed
        ///   when a window gains focus. Likewise, synthetic key release events
        ///   are generated for all keys pressed when a window goes out of focus.
        ///   ***Currently, this is only functional on X11 and Windows***
        ///
        /// Otherwise, this value is always `false`.
        is_synthetic: bool,
    },

    /// The keyboard modifiers have changed.
    ///
    /// Platform-specific behavior:
    /// - **Web**: This API is currently unimplemented on the web. This isn't by design - it's an
    ///   issue, and it should get fixed - but it's the current state of the API.
    ModifiersChanged(ModifiersState),

    /// The cursor has moved on the window.
    CursorMoved {
        device_id: DeviceId,
        /// (x,y) coords in pixels relative to the top-left corner of the window. Because the range of this data is
        /// limited by the display area and it may have been transformed by the OS to implement effects such as cursor
        /// acceleration, it should not be used to implement non-cursor-like interactions such as 3D camera control.
        position: PhysicalPosition<f64>,
    },

    /// The cursor has entered the window.
    CursorEntered {
        device_id: DeviceId,
    },

    /// The cursor has left the window.
    CursorLeft {
        device_id: DeviceId,
    },

    /// A mouse wheel movement or touchpad scroll occurred.
    MouseWheel {
        device_id: DeviceId,
        delta: MouseScrollDelta,
        phase: TouchPhase,
    },

    /// An mouse button press has been received.
    MouseInput {
        device_id: DeviceId,
        state: ElementState,
        button: MouseButton,
    },

    /// Touchpad pressure event.
    ///
    /// At the moment, only supported on Apple forcetouch-capable macbooks.
    /// The parameters are: pressure level (value between 0 and 1 representing how hard the touchpad
    /// is being pressed) and stage (integer representing the click level).
    TouchpadPressure {
        device_id: DeviceId,
        pressure: f32,
        stage: i64,
    },

    /// Motion on some analog axis. May report data redundant to other, more specific events.
    AxisMotion {
        device_id: DeviceId,
        axis: AxisId,
        value: f64,
    },

    /// Touch event has been received
    Touch(Touch),

    /// The window's scale factor has changed.
    ///
    /// The following user actions can cause DPI changes:
    ///
    /// * Changing the display's resolution.
    /// * Changing the display's scale factor (e.g. in Control Panel on Windows).
    /// * Moving the window to a display with a different scale factor.
    ///
    /// After this event callback has been processed, the window will be resized to whatever value
    /// is pointed to by the `new_inner_size` reference. By default, this will contain the size suggested
    /// by the OS, but it can be changed to any value.
    ///
    /// For more information about DPI in general, see the [`dpi`](crate::dpi) module.
    ScaleFactorChanged {
        scale_factor: f64,
        new_inner_size: PhysicalSize<u32>,
    },

    /// The system window theme has changed.
    ///
    /// Applications might wish to react to this to change the theme of the content of the window
    /// when the system changes the window theme.
    ///
    /// At the moment this is only supported on Windows.
    ThemeChanged(Theme),
}

impl From<winit::event::WindowEvent<'_>> for WindowEvent {
    fn from(event: winit::event::WindowEvent<'_>) -> Self {
        match event {
            winit::event::WindowEvent::Resized(s) => Self::Resized(s),
            winit::event::WindowEvent::Moved(m) => Self::Moved(m),
            winit::event::WindowEvent::CloseRequested => Self::CloseRequested,
            winit::event::WindowEvent::Destroyed => Self::Destroyed,
            winit::event::WindowEvent::DroppedFile(f) => Self::DroppedFile(f),
            winit::event::WindowEvent::HoveredFile(f) => Self::HoveredFile(f),
            winit::event::WindowEvent::HoveredFileCancelled => Self::HoveredFileCancelled,
            winit::event::WindowEvent::ReceivedCharacter(c) => Self::ReceivedCharacter(c),
            winit::event::WindowEvent::Focused(f) => Self::Focused(f),
            winit::event::WindowEvent::KeyboardInput {
                device_id,
                input,
                is_synthetic,
            } => Self::KeyboardInput {
                device_id,
                input,
                is_synthetic,
            },
            winit::event::WindowEvent::ModifiersChanged(m) => Self::ModifiersChanged(m),
            winit::event::WindowEvent::CursorMoved {
                device_id,
                position,
                ..
            } => Self::CursorMoved {
                device_id,
                position,
            },
            winit::event::WindowEvent::CursorEntered { device_id } => {
                Self::CursorEntered { device_id }
            }
            winit::event::WindowEvent::CursorLeft { device_id } => Self::CursorLeft { device_id },
            winit::event::WindowEvent::MouseWheel {
                device_id,
                delta,
                phase,
                ..
            } => Self::MouseWheel {
                device_id,
                delta,
                phase,
            },
            winit::event::WindowEvent::MouseInput {
                device_id,
                state,
                button,
                ..
            } => Self::MouseInput {
                device_id,
                state,
                button,
            },
            winit::event::WindowEvent::TouchpadPressure {
                device_id,
                pressure,
                stage,
            } => Self::TouchpadPressure {
                device_id,
                pressure,
                stage,
            },
            winit::event::WindowEvent::AxisMotion {
                device_id,
                axis,
                value,
            } => Self::AxisMotion {
                device_id,
                axis,
                value,
            },
            winit::event::WindowEvent::Touch(t) => Self::Touch(t),
            winit::event::WindowEvent::ScaleFactorChanged {
                scale_factor,
                new_inner_size,
            } => Self::ScaleFactorChanged {
                scale_factor,
                new_inner_size: *new_inner_size,
            },
            winit::event::WindowEvent::ThemeChanged(t) => Self::ThemeChanged(t),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum DeviceEvent {
    Added(DeviceId),
    Removed(DeviceId),
    MouseMotion(DeviceId, DVec2),
    MouseWheel(DeviceId, MouseScrollDelta),
    Motion(DeviceId, u32, f64),
    Button(DeviceId, u32, ElementState),
    Key(DeviceId, ScanCode, ElementState, Option<VirtualKeyCode>),
    Text(DeviceId, char),
}

impl From<(DeviceId, winit::event::DeviceEvent)> for DeviceEvent {
    fn from(e: (DeviceId, winit::event::DeviceEvent)) -> Self {
        let (d, event) = e;
        match event {
            winit::event::DeviceEvent::Added => DeviceEvent::Added(d),
            winit::event::DeviceEvent::Removed => DeviceEvent::Removed(d),
            winit::event::DeviceEvent::MouseMotion { delta } => {
                DeviceEvent::MouseMotion(d, DVec2::new(delta.0, delta.1))
            }
            winit::event::DeviceEvent::MouseWheel { delta } => DeviceEvent::MouseWheel(d, delta),
            winit::event::DeviceEvent::Motion { axis, value } => {
                DeviceEvent::Motion(d, axis, value)
            }
            winit::event::DeviceEvent::Button { button, state } => {
                DeviceEvent::Button(d, button, state)
            }
            winit::event::DeviceEvent::Key(k) => {
                DeviceEvent::Key(d, k.scancode, k.state, k.virtual_keycode)
            }
            winit::event::DeviceEvent::Text { codepoint } => DeviceEvent::Text(d, codepoint),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowData {
    pub position: IVec2,
    pub mouse_position: DVec2,
}
