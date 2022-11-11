use crate::flatland::Flatland;
use anyhow::Result;
use mint::Vector2;
use parking_lot::Mutex;
use softbuffer::GraphicsContext;
use std::{mem::ManuallyDrop, sync::Arc};
use winit::{
	dpi::{LogicalPosition, PhysicalPosition, Size},
	event::{
		ElementState, Event, KeyboardInput, ModifiersState, MouseButton, MouseScrollDelta,
		VirtualKeyCode, WindowEvent,
	},
	event_loop::EventLoop,
	platform::unix::WindowExtUnix,
	window::{CursorGrabMode, Window, WindowBuilder},
};
use xcb::ffi::xcb_connection_t;
use xkbcommon::xkb::{
	self,
	x11::{get_core_keyboard_device_id, keymap_new_from_device},
	Keymap, KEYMAP_COMPILE_NO_FLAGS, KEYMAP_FORMAT_TEXT_V1,
};

const RADIUS: u32 = 8;
pub struct InputWindow {
	flatland: Arc<Mutex<Flatland>>,
	graphics_context: GraphicsContext<Window>,
	cursor_position: Option<LogicalPosition<u32>>,
	grabbed: bool,
	modifiers: ModifiersState,
	keymap: Keymap,
}
impl InputWindow {
	pub fn new(event_loop: &EventLoop<()>, flatland: Arc<Mutex<Flatland>>) -> Result<Self> {
		let size = Size::Logical([512, 512].into());
		let window = WindowBuilder::new()
			.with_title("Flatland")
			.with_min_inner_size(size)
			.with_max_inner_size(size)
			.with_inner_size(size)
			.with_resizable(false)
			.with_always_on_top(true)
			.build(event_loop)?;

		let keymap = match window.xcb_connection() {
			Some(raw_conn) => {
				let connection = unsafe {
					ManuallyDrop::new(xcb::Connection::from_raw_conn(
						raw_conn as *mut xcb_connection_t,
					))
				};
				keymap_new_from_device(
					&xkb::Context::new(0),
					&connection,
					get_core_keyboard_device_id(&connection),
					KEYMAP_COMPILE_NO_FLAGS,
				)
			}
			None => Keymap::new_from_names(&xkb::Context::new(0), "", "", "", "", None, 0).unwrap(),
		};

		let graphics_context = unsafe { GraphicsContext::new(window) }.unwrap();

		let mut input_window = InputWindow {
			flatland,
			graphics_context,
			cursor_position: None,
			grabbed: true,
			modifiers: ModifiersState::empty(),
			keymap,
		};
		input_window.set_grab(false);

		Ok(input_window)
	}

	fn window(&mut self) -> &mut Window {
		self.graphics_context.window_mut()
	}

	pub fn handle_event(&mut self, event: Event<()>) {
		match event {
			Event::WindowEvent { event, .. } => self.handle_window_event(event),
			Event::RedrawRequested(_window_id) => {
				let window_size = self.window().inner_size();
				let buffer_len = window_size.width * window_size.height;
				let mut buffer = vec![0; buffer_len as usize];
				if let Some(mouse_position) = self.cursor_position {
					for x in (mouse_position.x.checked_sub(RADIUS).unwrap_or(0))..(mouse_position.x.checked_add(RADIUS).unwrap_or(u32::MAX)) {
						for y in (mouse_position.y.checked_sub(RADIUS).unwrap_or(0))..(mouse_position.y.checked_add(RADIUS).unwrap_or(u32::MAX)) {
							if let Some(pixel) =
								buffer.get_mut((x + (y * window_size.width)) as usize)
							{
								*pixel = u32::MAX;
							}
						}
					}
				}

				self.graphics_context.set_buffer(
					&buffer,
					window_size.width as u16,
					window_size.height as u16,
				);
			}
			_ => (),
		}
	}

	fn handle_window_event(&mut self, event: WindowEvent) {
		match event {
			WindowEvent::MouseInput { state, button, .. } => self.handle_mouse_input(state, button),
			WindowEvent::MouseWheel { delta, .. } => self.handle_axis(delta),
			WindowEvent::CursorMoved { position, .. } => self.handle_mouse_move(position),
			WindowEvent::KeyboardInput { input, .. } => self.handle_keyboard_input(input),
			WindowEvent::ModifiersChanged(state) => self.modifiers = state,
			WindowEvent::CloseRequested => self.flatland.lock().client.stop_loop(),
			WindowEvent::Destroyed => self.flatland.lock().client.stop_loop(),
			_ => (),
		}
	}

	fn handle_mouse_move(&mut self, position: PhysicalPosition<f64>) {
		self.cursor_position = if self.grabbed {
			self.window().request_redraw();
			Some(position.to_logical::<u32>(self.window().scale_factor()))
		} else {
			None
		};

		if self.grabbed {
			let window_size = self.window().inner_size();
			let cursor_position = position.to_logical::<f64>(self.window().scale_factor());
			let center_position = LogicalPosition::new(
				window_size.width as f64 / 2.0,
				window_size.height as f64 / 2.0,
			);
			let cursor_delta = Vector2::from_slice(&[
				(cursor_position.x - center_position.x) as f32,
				(cursor_position.y - center_position.y) as f32,
			]);

			if let Some(focused) = self.flatland.lock().focused.clone().upgrade() {
				focused.lock().pointer_delta(cursor_delta);
			}

			self.window().set_cursor_position(center_position).unwrap();
		}
	}

	fn handle_mouse_input(&mut self, state: ElementState, button: MouseButton) {
		if !self.grabbed {
			if state == ElementState::Released && button == MouseButton::Left {
				self.set_grab(true);
			}
		} else {
			self.flatland.lock().with_focused(|item| {
				item.pointer_button(
					match button {
						MouseButton::Left => input_event_codes::BTN_LEFT!(),
						MouseButton::Right => input_event_codes::BTN_RIGHT!(),
						MouseButton::Middle => input_event_codes::BTN_MIDDLE!(),
						MouseButton::Other(_) => {
							return;
						}
					},
					match state {
						ElementState::Released => 0,
						ElementState::Pressed => 1,
					},
				)
				.unwrap();
			});
		}
	}

	fn handle_axis(&mut self, delta: MouseScrollDelta) {
		if self.grabbed {
			self.flatland.lock().with_focused(|item| {
				let (scroll_distance, scroll_steps) = match delta {
					MouseScrollDelta::LineDelta(right, down) => {
						(Vector2::from([0.0, 0.0]), Vector2::from([-right, -down]))
					}
					MouseScrollDelta::PixelDelta(offset) => (
						Vector2::from([-offset.x as f32, -offset.y as f32]),
						Vector2::from([0.0, 0.0]),
					),
				};

				item.pointer_scroll(scroll_distance, scroll_steps).unwrap();
			});
		}
	}

	fn handle_keyboard_input(&mut self, input: KeyboardInput) {
		if input.virtual_keycode == Some(VirtualKeyCode::Escape)
			&& input.state == ElementState::Released
			&& self.modifiers.ctrl()
		{
			self.set_grab(false);
		} else {
			self.flatland.lock().with_focused(|item| {
				item.keyboard_key_state(input.scancode, input.state == ElementState::Pressed)
					.unwrap();
			});
		}
	}

	const GRABBED_WINDOW_TITLE: &'static str = "Flatland Input (ctrl+esc to release cursor)";
	const UNGRABBED_WINDOW_TITLE: &'static str = "Flatland Input (click to grab input)";
	fn set_grab(&mut self, grab: bool) {
		if grab == self.grabbed {
			return;
		}
		self.grabbed = grab;

		self.window().set_cursor_visible(!grab);
		if grab {
			let window_size = self.window().inner_size();
			let center_position =
				LogicalPosition::new(window_size.width / 2, window_size.height / 2);
			self.window().set_cursor_position(center_position).unwrap();
			self.flatland.lock().with_focused(|item| {
				let keymap = self.keymap.get_as_string(KEYMAP_FORMAT_TEXT_V1);
				item.keyboard_activate(&keymap).unwrap();
			});
		} else {
			self.flatland.lock().with_focused(|item| {
				item.keyboard_deactivate().unwrap();
			});
		}
		let window_title = if grab {
			Self::GRABBED_WINDOW_TITLE
		} else {
			Self::UNGRABBED_WINDOW_TITLE
		};

		let grab = if grab {
			CursorGrabMode::Confined
		} else {
			CursorGrabMode::None
		};

		if self.window().set_cursor_grab(grab).is_ok() {
			self.window().set_title(window_title);
		}
	}
}
