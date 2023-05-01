use crate::toplevel::Toplevel;
use glam::Quat;

use stardust_xr_fusion::{
	client::FrameInfo,
	core::values::Transform,
	items::panel::{
		CursorInfo, PanelItem, PanelItemHandler, PanelItemInitData, PopupInfo, PositionerData,
		RequestedState, State, SurfaceID, ToplevelInfo,
	},
	node::NodeType,
};

pub struct PanelItemUI {
	pub item: PanelItem,
	// cursor: Cursor,
	// mouse: HandlerWrapper<PulseReceiver, Mouse>,
	toplevel: Option<Toplevel>,
}
impl PanelItemUI {
	pub fn create(init_data: PanelItemInitData, item: PanelItem) -> Self {
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
		// let mouse = Mouse::new(
		// 	&item,
		// 	Transform::default(),
		// 	&touch_plane.field(),
		// 	Some(item.alias()),
		// 	Weak::new(),
		// 	SurfaceID::Toplevel,
		// )
		// .unwrap();

		// let cursor = Cursor::new(&item, &init_data.cursor, &item);
		// cursor.update_info(&None, &item);

		let mut ui = PanelItemUI {
			item,
			toplevel: None,
		};

		ui.update_toplevel_info(init_data.toplevel);
		ui
	}

	pub fn update_toplevel_info(&mut self, info: Option<ToplevelInfo>) {
		let Some(info) = info else {
			self.toplevel.take();
			return;
		};

		let Some(toplevel) = &mut self.toplevel else {
			self.toplevel
				.replace(Toplevel::create(self.item.alias(), info).unwrap());
			return;
		};

		toplevel.update_info(info);
	}

	pub fn frame(&mut self, info: &FrameInfo) {
		let Some(toplevel) = &mut self.toplevel else {return};
		toplevel.frame(info);
	}

	// pub fn pointer_delta(&mut self, delta: mint::Vector2<f32>) {
	// 	debug!(?delta, mapped = self.mapped, "Pointer delta");
	// 	if self.mapped {
	// 		let pos = Vector2::from([
	// 			(self.cursor.pos.x + delta.x).clamp(0.0, self.size.x - 1.0),
	// 			(self.cursor.pos.y + delta.y).clamp(0.0, self.size.y - 1.0),
	// 		]);
	// 		self.set_pointer_pos(pos);
	// 	}
	// }

	// pub fn set_pointer_pos(&mut self, pos: mint::Vector2<f32>) {
	// 	self.cursor.pos = pos;
	// 	let _ = self.item.pointer_motion(&SurfaceID::Toplevel, pos);
	// 	self.cursor.update_position(self.size, pos);
	// }

	// pub fn update_toplevel_info(&mut self, toplevel_info: Option<ToplevelInfo>) {
	// 	debug!(?toplevel_info, "Update toplevel info");
	// 	self.mapped = toplevel_info.is_some();
	// 	if let Some(toplevel_info) = &toplevel_info {
	// 		self.size =
	// 			Vector2::from_slice(&[toplevel_info.size.x as f32, toplevel_info.size.y as f32]);
	// 		let size = glam::vec3(self.size.x / PPM, self.size.y / PPM, 0.01);
	// 		self.model.set_scale(None, size).unwrap();
	// 		self.touch_plane.set_size(size.xy()).unwrap();
	// 		self.touch_plane.x_range = 0.0..self.size.x;
	// 		self.touch_plane.y_range = 0.0..self.size.y;
	// 		// self.touch_plane.set_debug(Some(DebugSettings::default()));
	// 		self.mouse
	// 			.node()
	// 			.set_position(None, Vector3::from([0.01, size.y * -0.5, 0.0]))
	// 			.unwrap();
	// 		let app_name = toplevel_info
	// 			.app_id
	// 			.clone()
	// 			.map(|id| id.split('.').last().unwrap_or_default().to_string());
	// 		let title = match (toplevel_info.title.clone(), app_name) {
	// 			(Some(title), Some(app_name)) => {
	// 				if title == app_name {
	// 					title
	// 				} else {
	// 					format!("{title} - {app_name}")
	// 				}
	// 			}
	// 			(Some(title), None) => title,
	// 			(None, Some(app_name)) => app_name,
	// 			(None, None) => String::new(),
	// 		};
	// 		self.title.set_text(title).unwrap();
	// 		self.title
	// 			.set_position(None, [size.x / 2.0, (size.y / 2.0) - THICKNESS, -THICKNESS])
	// 			.unwrap();
	// 	}
	// 	self.toplevel_info = toplevel_info;
	// }
}
impl PanelItemHandler for PanelItemUI {
	fn commit_toplevel(&mut self, state: Option<ToplevelInfo>) {
		self.update_toplevel_info(state);
	}

	fn set_cursor(&mut self, _info: Option<CursorInfo>) {
		// self.cursor.update_info(&info, &self.item);
	}

	fn recommend_toplevel_state(&mut self, state: RequestedState) {
		let Some(toplevel) = &self.toplevel else {return};
		toplevel.recommend_state(state);
	}

	fn show_window_menu(&mut self) {}

	fn new_popup(&mut self, uid: &str, data: Option<PopupInfo>) {
		let Some(info) = data else {return};
		let Some(toplevel) = &mut self.toplevel else {return};
		let id = SurfaceID::Popup(uid.to_string());
		toplevel.new_popup(id, info);
	}
	fn reposition_popup(&mut self, uid: &str, data: Option<PositionerData>) {
		let Some(info) = data else {return};
		let Some(toplevel) = &mut self.toplevel else {return};
		let id = SurfaceID::Popup(uid.to_string());
		toplevel.reposition_popup(id, info);
	}
	fn drop_popup(&mut self, uid: &str) {
		let Some(toplevel) = &mut self.toplevel else {return};
		let id = SurfaceID::Popup(uid.to_string());
		toplevel.drop_popup(id);
	}
}
// impl Drop for PanelItemUI {
// 	fn drop(&mut self) {
// 		println!("Panel item destroyed");
// 	}
// }
