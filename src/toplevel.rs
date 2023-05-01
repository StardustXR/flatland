use crate::surface::Surface;
use glam::vec3;
use rustc_hash::FxHashMap;
use stardust_xr_fusion::{
	client::FrameInfo,
	core::values::Transform,
	items::panel::{
		PanelItem, PopupInfo, PositionerData, RequestedState, State, SurfaceID, ToplevelInfo,
	},
	node::{NodeError, NodeType},
};
use stardust_xr_molecules::{GrabData, Grabbable};
use tracing::debug;

pub struct Toplevel {
	item: PanelItem,
	surface: Surface,
	grabbable: Grabbable,
	// title: Text,
	popups: FxHashMap<SurfaceID, Surface>,
}
impl Toplevel {
	pub fn create(item: PanelItem, info: ToplevelInfo) -> Result<Self, NodeError> {
		let surface = Surface::create(
			&item,
			Transform::none(),
			item.alias(),
			SurfaceID::Toplevel,
			info.size,
		)?;
		surface.root().set_position(
			None,
			vec3(surface.physical_size().x, -surface.physical_size().y, 0.0) * -0.5,
		)?;
		let grabbable = Grabbable::create(
			item.node().client()?.get_root(),
			Transform::default(),
			&surface.field(),
			GrabData::default(),
		)?;
		item.set_spatial_parent_in_place(grabbable.content_parent())?;

		// let title_style = TextStyle {
		// 	character_height: THICKNESS * 1.5,
		// 	text_align: Alignment::XLeft | Alignment::YCenter,
		// 	..Default::default()
		// };
		// let title = Text::create(
		// 	&item,
		// 	Transform::from_position_rotation(
		// 		vec3(
		// 			surface.physical_size().x,
		// 			surface.physical_size().y,
		// 			THICKNESS,
		// 		) * 0.5,
		// 		Quat::from_rotation_x(-PI * 0.5) * Quat::from_rotation_y(-PI * 0.5),
		// 	),
		// 	"",
		// 	title_style,
		// )
		// .unwrap();

		Ok(Toplevel {
			item,
			surface,
			grabbable,
			// title,
			popups: FxHashMap::default(),
		})
	}

	pub fn frame(&mut self, info: &FrameInfo) {
		self.grabbable.update(info).unwrap();
		if !self.grabbable.grab_action().actor_acting() {
			self.surface.update();
			for popup in self.popups.values_mut() {
				popup.update();
			}
		}
	}

	pub fn update_info(&mut self, info: ToplevelInfo) {
		self.surface.resize(info.size).unwrap();
		// self.title
		// 	.set_position(
		// 		None,
		// 		vec3(
		// 			self.surface.physical_size().x,
		// 			self.surface.physical_size().y,
		// 			THICKNESS,
		// 		) * 0.5,
		// 	)
		// 	.unwrap();
	}

	pub fn recommend_state(&self, state: RequestedState) {
		debug!(?state, "Recommend toplevel state");
		let new_states = match state {
			RequestedState::Maximize(true) => vec![
				State::Activated,
				State::Maximized,
				State::TiledLeft,
				State::TiledRight,
				State::TiledTop,
				State::TiledBottom,
			],
			RequestedState::Fullscreen(true) => vec![
				State::Activated,
				State::Fullscreen,
				State::TiledLeft,
				State::TiledRight,
				State::TiledTop,
				State::TiledBottom,
			],
			_ => vec![
				State::Activated,
				State::TiledLeft,
				State::TiledRight,
				State::TiledTop,
				State::TiledBottom,
			],
		};
		self.item
			.configure_toplevel(None, &new_states, None)
			.unwrap();
	}

	pub fn new_popup(&mut self, id: SurfaceID, info: PopupInfo) {
		let parent = self.popups.get(&info.parent).unwrap_or(&self.surface);
		let surface = Surface::new_child(parent, id.clone(), &info.positioner_data).unwrap();
		self.popups.insert(id, surface);
	}
	pub fn reposition_popup(&mut self, id: SurfaceID, info: PositionerData) {
		let Some(popup) = self.popups.get_mut(&id) else {return};
		popup.resize(info.size).unwrap();
		popup.set_offset(info.anchor_rect_pos).unwrap();
	}
	pub fn drop_popup(&mut self, id: SurfaceID) {
		self.popups.remove(&id);
	}
}
