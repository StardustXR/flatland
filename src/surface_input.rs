use crate::{
	Flatland, Shell,
	pointer_2d::{Pointer2D, Pointer2DDepth},
	pointer_input::PointerPlane,
	touch_input::TouchPlane,
};
use glam::{Vec2, vec2};
use stardust_xr_asteroids::{
	CustomElement as _, Element,
	components::{KeyboardHandler, MouseHandler, ScrollSource as AsteroidsScrollSource},
	elements::Spatial,
};
use stardust_xr_molecules::DebugSettings;
use stardust_xr_panels::panel_item::{ModifierState, PanelItem, ScrollSource, SurfaceId};
use tokio::sync::watch;

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
	item: &PanelItem,
	cursor: watch::Sender<Pointer2D>,
	scroll_multiplier: f32,
) -> MouseHandler<Flatland> {
	let m = scroll_multiplier;
	MouseHandler::<Flatland>::new()
		.on_motion_async({
			let cursor = cursor.clone();
			let item = item.clone();
			move |motion, timestamp| {
				// sent from inside the modify so a button can't reach a surface before its motion does
				cursor.send_modify(|p| p.move_by(&item, vec2(motion.x, -motion.y), timestamp));
			}
		})
		.on_motion(|state, _, _| {
			// the async side already moved it, this only switches how deep it's drawn
			if let Some(item) = &mut state.panel {
				item.cursor_depth = Pointer2DDepth::Blend;
			}
		})
		.on_button_async({
			let cursor = cursor.clone();
			let item = item.clone();
			move |button, pressed, timestamp| {
				let c = cursor.borrow();
				let _ = item.pointer_button(c.target, button, pressed, timestamp);
			}
		})
		.on_scroll_discrete_async({
			let cursor = cursor.clone();
			let item = item.clone();
			move |v, source, timestamp| {
				let c = cursor.borrow();
				_ = item.pointer_scroll_discrete(
					c.target,
					[v.x * m, -v.y * m].into(),
					scroll_source(source),
					timestamp,
				);
			}
		})
		.on_scroll_continuous_async({
			let item = item.clone();
			move |v, source, timestamp| {
				let p = cursor.borrow();
				_ = item.pointer_scroll_pixels(
					p.target,
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
				// a ray through stacked surfaces hits every plane, only the one driving the pointer
				// presses, releases always go through so nothing gets stuck down
				if let Some(item) = &state.panel
					&& (!pressed || item.cursor_depth == Pointer2DDepth::On(surface_id))
				{
					let target = item.pointer.borrow().target;
					let _ = item.item.pointer_button(target, button, pressed, None);
				}
			})
			.on_pointer_motion(move |state, pos| {
				let p = [pos.x * state.density, pos.y * state.density];
				state.set_pointer(surface_id, p, None);
			})
			.on_scroll(move |state, scroll| {
				let Some(item) = &state.panel else {
					return;
				};
				if item.cursor_depth != Pointer2DDepth::On(surface_id) {
					return;
				}
				let m = state.mouse_scroll_multiplier;
				let target = item.pointer.borrow().target;
				let mut scrolled = false;
				for v in [scroll.scroll_continuous, scroll.scroll_discrete]
					.into_iter()
					.flatten()
				{
					scrolled = true;
					_ = item.item.pointer_scroll_pixels(
						target,
						[v.x * m, -v.y * m].into(),
						ScrollSource::Continuous,
						None,
					);
				}
				// TODO: figure out how to send this only when scroll actually stops,
				// instead of every frame without scroll
				if !scrolled {
					_ = item.item.pointer_scroll_stop(target, None);
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
				if let Some(item) = &state.panel {
					let p = [pos.x * state.density, pos.y * state.density];
					let _ = item.item.touch_down(surface_id, id, p.into(), None);
				}
			})
			.on_touch_move(|state, id, pos| {
				if let Some(item) = &state.panel {
					let p = [pos.x * state.density, pos.y * state.density];
					let _ = item.item.touch_move(id, p.into(), None);
				}
			})
			.on_touch_up(|state, id| {
				if let Some(item) = &state.panel {
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
