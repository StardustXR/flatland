use crate::{Flatland, Shell, pointer_input::PointerPlane, touch_input::TouchPlane};
use glam::Vec2;
use stardust_xr_asteroids::{
	CustomElement as _, Element,
	components::{KeyboardHandler, MouseHandler, ScrollSource as AsteroidsScrollSource},
	elements::Spatial,
};
use stardust_xr_molecules::DebugSettings;
use stardust_xr_panels::panel_item::{ModifierState, ScrollSource, SurfaceId};

pub fn keyboard_handler(shell: Option<&Shell>, surface_id: SurfaceId) -> KeyboardHandler<Flatland> {
	let item = shell.map(|s| s.item().clone());
	KeyboardHandler::<Flatland>::new().on_key_async(move |key, timestamp| {
		if let Some(item) = &item {
			_ = item.key(
				surface_id,
				key.keycode,
				key.pressed,
				ModifierState {
					depressed: key.modifiers.depressed,
					latched: key.modifiers.latched,
					locked: key.modifiers.locked,
					layout_group: key.modifiers.layout_group,
				},
				key.keymap,
				timestamp,
			);
		}
	})
}

pub fn mouse_handler(
	shell: Option<&Shell>,
	surface_id: SurfaceId,
	scroll_multiplier: f32,
) -> MouseHandler<Flatland> {
	let item = shell.map(|s| s.item().clone());
	let m = scroll_multiplier;
	MouseHandler::<Flatland>::new()
		.on_button_async({
			let item = item.clone();
			move |button, pressed, timestamp| {
				if let Some(item) = &item {
					let _ = item.pointer_button(surface_id, button, pressed, timestamp);
				}
			}
		})
		.on_motion(move |state, motion, timestamp| {
			state.move_pointer(surface_id, [motion.x, -motion.y], timestamp);
		})
		.on_scroll_discrete_async({
			let item = item.clone();
			move |v, source, timestamp| {
				if let Some(item) = &item {
					_ = item.pointer_scroll_discrete(
						surface_id,
						[v.x * m, -v.y * m].into(),
						scroll_source(source),
						timestamp,
					);
				}
			}
		})
		.on_scroll_continuous_async(move |v, source, timestamp| {
			if let Some(item) = &item {
				_ = item.pointer_scroll_pixels(
					surface_id,
					[v.x * m, -v.y * m].into(),
					scroll_source(source),
					timestamp,
				);
			}
		})
}

fn scroll_source(source: AsteroidsScrollSource) -> ScrollSource {
	match source {
		AsteroidsScrollSource::Wheel => ScrollSource::Wheel,
		AsteroidsScrollSource::Finger => ScrollSource::Touch,
		AsteroidsScrollSource::Continuous => ScrollSource::Continuous,
		AsteroidsScrollSource::WheelTilt => ScrollSource::WheelTilt,
	}
}

// TODO: somehow get timestamps for these events
pub fn input_planes(
	surface_id: SurfaceId,
	size: Vec2,
	thickness: f32,
	debug: Option<DebugSettings>,
) -> impl Element<Flatland> + use<> {
	Spatial::default()
		.build()
		.child(
			PointerPlane::<Flatland> {
				debug_line_settings: debug,
				..Default::default()
			}
			.physical_size(size)
			.thickness(thickness)
			.on_mouse_button(move |state, button, pressed| {
				if let Some(item) = &state.panel_item {
					let _ = item.item.pointer_button(surface_id, button, pressed, None);
				}
			})
			.on_pointer_motion(move |state, pos| {
				let p = [pos.x * state.density, pos.y * state.density];
				state.set_pointer(surface_id, None, p, None);
			})
			.on_scroll(move |state, scroll| {
				let Some(item) = &state.panel_item else {
					return;
				};
				let m = state.mouse_scroll_multiplier;
				let mut scrolled = false;
				for v in [scroll.scroll_continuous, scroll.scroll_discrete]
					.into_iter()
					.flatten()
				{
					scrolled = true;
					_ = item.item.pointer_scroll_pixels(
						surface_id,
						[v.x * m, -v.y * m].into(),
						ScrollSource::Continuous,
						None,
					);
				}
				// TODO: figure out how to send this only when scroll actually stops,
				// instead of every frame without scroll
				if !scrolled {
					_ = item.item.pointer_scroll_stop(surface_id, None);
				}
			})
			.build(),
		)
		.child(
			TouchPlane::<Flatland> {
				debug_line_settings: debug,
				..Default::default()
			}
			.physical_size(size)
			.thickness(thickness)
			.on_touch_down(move |state, id, pos| {
				if let Some(item) = &state.panel_item {
					let p = [pos.x * state.density, pos.y * state.density];
					let _ = item.item.touch_down(surface_id, id, p.into(), None);
				}
			})
			.on_touch_move(|state, id, pos| {
				if let Some(item) = &state.panel_item {
					let p = [pos.x * state.density, pos.y * state.density];
					let _ = item.item.touch_move(id, p.into(), None);
				}
			})
			.on_touch_up(|state, id| {
				if let Some(item) = &state.panel_item {
					let _ = item.item.touch_up(id, None);
				}
			})
			.on_hover(move |state, hovers| {
				if hovers.is_empty() {
					state.touch_hovers.remove(&surface_id);
				} else {
					state.touch_hovers.insert(surface_id, hovers);
				}
			})
			.build(),
		)
}
