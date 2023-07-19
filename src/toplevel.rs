use std::f32::consts::PI;

use crate::surface::{Surface, THICKNESS};
use glam::{vec3, Quat};
use rustc_hash::FxHashMap;
use stardust_xr_fusion::{
	client::FrameInfo,
	core::values::Transform,
	drawable::{Alignment, Text, TextStyle},
	items::panel::{PanelItem, PopupInfo, PositionerData, SurfaceID, ToplevelInfo},
	node::{NodeError, NodeType},
};
use stardust_xr_molecules::{GrabData, Grabbable};

pub struct Toplevel {
	_item: PanelItem,
	surface: Surface,
	grabbable: Grabbable,
	title: Text,
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

		let title_style = TextStyle {
			character_height: THICKNESS, // * 1.5,
			text_align: Alignment::XLeft | Alignment::YBottom,
			..Default::default()
		};
		let title = Text::create(
			&item,
			Transform::from_position_rotation(
				[
					surface.physical_size().x * 0.5,
					surface.physical_size().y * 0.5,
					-THICKNESS,
				],
				Quat::from_rotation_x(-PI * 0.5) * Quat::from_rotation_y(-PI * 0.5),
			),
			&info.title.unwrap_or_default(),
			title_style,
		)
		.unwrap();

		Ok(Toplevel {
			_item: item,
			surface,
			grabbable,
			title,
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
		self.grabbable.cancel_linear_velocity();
		self.grabbable.cancel_angular_velocity();
	}

	pub fn update_info(&mut self, info: ToplevelInfo) {
		self.surface.resize(info.size).unwrap();
		let app_name = info
			.app_id
			.clone()
			.map(|id| id.split('.').last().unwrap_or_default().to_string());
		let title = match (info.title.clone(), app_name) {
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
			.set_position(
				None,
				[
					self.surface.physical_size().x * 0.5,
					self.surface.physical_size().y * 0.5,
					-THICKNESS,
				],
			)
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
