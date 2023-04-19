use crate::{cursor::Cursor, keyboard::Keyboard, mouse::Mouse};
use glam::{Quat, Vec3Swizzles};
use lazy_static::lazy_static;
use mint::{Vector2, Vector3};
use stardust_xr_fusion::{
	client::FrameInfo,
	core::values::Transform,
	data::PulseReceiver,
	drawable::{Alignment, Model, ResourceID, Text, TextStyle},
	input::InputDataType,
	items::panel::{
		CursorInfo, PanelItem, PanelItemHandler, PanelItemInitData, PopupInfo, PositionerData,
		RequestedState, State, SurfaceID, ToplevelInfo,
	},
	node::NodeType,
	HandlerWrapper,
};
use stardust_xr_molecules::{touch_plane::TouchPlane, GrabData, Grabbable};
use std::{f32::consts::PI, sync::Weak};
use tracing::debug;

lazy_static! {
	pub static ref PANEL_RESOURCE: ResourceID = ResourceID::new_namespaced("flatland", "panel");
}

pub const PPM: f32 = 1000.0;
pub struct PanelItemUI {
	pub item: PanelItem,
	pub model: Model,
	cursor: Cursor,
	mapped: bool,
	size: Vector2<f32>,
	title: Text,
	toplevel_info: Option<ToplevelInfo>,
	keyboard: HandlerWrapper<PulseReceiver, Keyboard>,
	pub mouse: HandlerWrapper<PulseReceiver, Mouse>,
	grabbable: Grabbable,
	touch_plane: TouchPlane,
}
impl PanelItemUI {
	pub fn new(init_data: PanelItemInitData, item: PanelItem) -> Self {
		// println!("Panel item created with {:#?}", init_data);
		// if init_data.size.x < 200 || init_data.size.y < 200 {
		// 	item.resize(1600, 900).unwrap();
		// }

		item.configure_toplevel(
			// Some([1000; 2].into()),
			None,
			&[
				State::Maximized,
				// State::Fullscreen,
				// State::Resizing,
				State::Activated,
				State::TiledLeft,
				State::TiledRight,
				State::TiledTop,
				State::TiledBottom,
			],
			None,
		)
		.unwrap();
		item.set_toplevel_capabilities(&[]).unwrap();
		item.set_transform(
			Some(item.client().unwrap().get_hmd()),
			Transform::from_position_rotation_scale([0.0, 0.0, -0.5], Quat::IDENTITY, [1.0; 3]),
		)
		.unwrap();

		let touch_plane = TouchPlane::new(
			&item,
			Transform::from_position([0.0, 0.0, 0.005]),
			[0.0; 2],
			0.01,
		)
		.unwrap();
		let grabbable = Grabbable::new(
			item.client().unwrap().get_root(),
			Transform::default(),
			&touch_plane.field(),
			GrabData::default(),
		)
		.unwrap();
		grabbable
			.content_parent()
			.set_transform(Some(&item), Transform::default())
			.unwrap();
		item.set_spatial_parent_in_place(grabbable.content_parent())
			.unwrap();
		let keyboard = Keyboard::new(
			&item,
			Transform::default(),
			&touch_plane.field(),
			Some(item.alias()),
			SurfaceID::Toplevel,
		)
		.unwrap();
		let mouse = Mouse::new(
			&item,
			Transform::default(),
			&touch_plane.field(),
			Some(item.alias()),
			Weak::new(),
			SurfaceID::Toplevel,
		)
		.unwrap();
		let model = Model::create(&item, Transform::default(), &PANEL_RESOURCE).unwrap();

		let title_style = TextStyle {
			character_height: 0.0075,
			text_align: Alignment::XLeft | Alignment::YCenter,
			..Default::default()
		};
		let title = Text::create(
			&item,
			Transform::from_rotation(
				Quat::from_rotation_x(-PI * 0.5) * Quat::from_rotation_y(-PI * 0.5),
			),
			"",
			title_style,
		)
		.unwrap();

		let cursor = Cursor::new(&item, &init_data.cursor, &item);
		cursor.update_info(&None, &item);

		let mut ui = PanelItemUI {
			item,
			model,
			cursor,
			mapped: false,
			size: Vector2::from([0.0; 2]),
			title,
			toplevel_info: None,
			keyboard,
			mouse,
			grabbable,
			touch_plane,
		};
		if init_data.toplevel.is_some() {
			ui.item
				.apply_surface_material(&SurfaceID::Toplevel, &ui.model, 0)
				.unwrap();
		}
		ui.update_toplevel_info(init_data.toplevel);
		ui
	}

	pub fn frame(&mut self, info: &FrameInfo) -> f32 {
		self.grabbable.update(info);
		self.touch_plane.update();

		if let Some(closest_hover) = self
			.touch_plane
			.hovering_inputs()
			.into_iter()
			.reduce(|a, b| if a.distance > b.distance { b } else { a })
		{
			let interact_point = match &closest_hover.input {
				InputDataType::Pointer(p) => p.deepest_point,
				InputDataType::Hand(h) => h.index.tip.position,
				InputDataType::Tip(t) => t.origin,
			};
			self.set_pointer_pos(Vector2 {
				x: self.size.x * 0.5 + (interact_point.x * PPM),
				y: self.size.y * 0.5 - (interact_point.y * PPM),
			})
		}

		if self.touch_plane.touch_started() {
			self.item
				.pointer_button(&SurfaceID::Toplevel, input_event_codes::BTN_LEFT!(), true)
				.unwrap();
		} else if self.touch_plane.touch_stopped() {
			self.item
				.pointer_button(&SurfaceID::Toplevel, input_event_codes::BTN_LEFT!(), false)
				.unwrap();
		}
		self.grabbable.min_distance()
	}

	pub fn pointer_delta(&mut self, delta: mint::Vector2<f32>) {
		debug!(?delta, mapped = self.mapped, "Pointer delta");
		if self.mapped {
			let pos = Vector2::from([
				(self.cursor.pos.x + delta.x).clamp(0.0, self.size.x - 1.0),
				(self.cursor.pos.y + delta.y).clamp(0.0, self.size.y - 1.0),
			]);
			self.set_pointer_pos(pos);
		}
	}

	pub fn set_pointer_pos(&mut self, pos: mint::Vector2<f32>) {
		self.cursor.pos = pos;
		let _ = self.item.pointer_motion(&SurfaceID::Toplevel, pos);
		self.cursor.update_position(self.size, pos);
	}

	pub fn update_toplevel_info(&mut self, toplevel_info: Option<ToplevelInfo>) {
		debug!(?toplevel_info, "Update toplevel info");
		self.mapped = toplevel_info.is_some();
		if let Some(toplevel_info) = &toplevel_info {
			self.size =
				Vector2::from_slice(&[toplevel_info.size.x as f32, toplevel_info.size.y as f32]);
			let size = glam::vec3(self.size.x / PPM, self.size.y / PPM, 0.01);
			self.model.set_scale(None, size).unwrap();
			self.touch_plane.set_size(size.xy()).unwrap();
			// self.touch_plane.set_debug(Some(DebugSettings::default()));
			self.keyboard
				.node()
				.set_position(None, Vector3::from([-0.01, size.y * -0.5, 0.0]))
				.unwrap();
			self.mouse
				.node()
				.set_position(None, Vector3::from([0.01, size.y * -0.5, 0.0]))
				.unwrap();
			let app_name = toplevel_info
				.app_id
				.clone()
				.map(|id| id.split('.').last().unwrap_or_default().to_string());
			let title = match (toplevel_info.title.clone(), app_name) {
				(Some(title), Some(app_name)) => {
					if title == app_name {
						title
					} else {
						format!("{title} - {app_name}")
					}
				}
				(Some(title), None) => title,
				(None, Some(app_name)) => app_name,
				(None, None) => String::new(),
			};
			self.title.set_text(title).unwrap();
			self.title
				.set_position(None, [size.x / 2.0, (size.y / 2.0) - 0.005, -0.005])
				.unwrap();
		}
		self.toplevel_info = toplevel_info;
	}
}
impl PanelItemHandler for PanelItemUI {
	fn commit_toplevel(&mut self, state: Option<ToplevelInfo>) {
		if self.toplevel_info.is_none() && state.is_some() {
			self.item
				.apply_surface_material(&SurfaceID::Toplevel, &self.model, 0)
				.unwrap();
		}
		self.update_toplevel_info(state);
	}

	fn set_cursor(&mut self, info: Option<CursorInfo>) {
		self.cursor.update_info(&info, &self.item);
	}

	fn recommend_toplevel_state(&mut self, state: RequestedState) {
		debug!(?state, "Recommend toplevel state");
		let new_states = match state {
			RequestedState::Maximize(true) => vec![State::Activated, State::Maximized],
			RequestedState::Fullscreen(true) => vec![State::Activated, State::Fullscreen],
			_ => vec![State::Activated],
		};
		self.item
			.configure_toplevel(None, &new_states, None)
			.unwrap();
	}

	fn show_window_menu(&mut self) {}

	fn new_popup(&mut self, uid: &str, data: Option<PopupInfo>) {
		dbg!(uid);
		dbg!(data);
		self.item
			.apply_surface_material(&SurfaceID::Popup(uid.to_string()), &self.model, 0)
			.unwrap();
	}
	fn reposition_popup(&mut self, uid: &str, data: Option<PositionerData>) {
		dbg!(uid);
		dbg!(data);
	}
	fn drop_popup(&mut self, uid: &str) {
		dbg!(uid);
		self.item
			.apply_surface_material(&SurfaceID::Toplevel, &self.model, 0)
			.unwrap();
	}
}
impl Drop for PanelItemUI {
	fn drop(&mut self) {
		println!("Panel item destroyed");
	}
}
