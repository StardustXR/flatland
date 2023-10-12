use std::f32::consts::PI;

use crate::{panel_shell_grab_ball::PanelShellGrabBall, surface::Surface};
use glam::{vec3, Quat, Vec3};
use rustc_hash::FxHashMap;
use stardust_xr_fusion::{
	client::{Client, FrameInfo},
	core::values::Transform,
	drawable::{Alignment, Text, TextStyle},
	fields::UnknownField,
	items::{
		panel::{ChildInfo, Geometry, PanelItem, PanelItemHandler, PanelItemInitData, SurfaceID},
		ItemAcceptor,
	},
	node::{NodeError, NodeType},
	spatial::Spatial,
};
use stardust_xr_molecules::{Grabbable, GrabbableSettings, PointerMode};

pub const TOPLEVEL_THICKNESS: f32 = 0.015;
pub const CHILD_THICKNESS: f32 = 0.005;

pub struct Toplevel {
	_item: PanelItem,
	surface: Surface,
	grabbable: Grabbable,
	title_text: Text,
	title: Option<String>,
	app_id: Option<String>,
	children: FxHashMap<String, Surface>,
	panel_shell_grab_ball: PanelShellGrabBall,
}
impl Toplevel {
	pub fn create(item: PanelItem, data: PanelItemInitData) -> Result<Self, NodeError> {
		let client = item.client()?;
		Self::initial_position_item(&client, &item)?;

		let surface = Surface::create(
			&item,
			Transform::none(),
			item.alias(),
			SurfaceID::Toplevel,
			data.toplevel.size,
			TOPLEVEL_THICKNESS,
		)?;
		surface.root().set_position(
			None,
			vec3(surface.physical_size().x, -surface.physical_size().y, 0.0) * -0.5,
		)?;
		let grabbable = Grabbable::create(
			item.node().client()?.get_root(),
			Transform::none(),
			&surface.field(),
			GrabbableSettings {
				linear_momentum: None,
				angular_momentum: None,
				magnet: true,
				pointer_mode: PointerMode::Move,
				max_distance: 0.0254,
				..Default::default()
			},
		)?;
		item.set_spatial_parent_in_place(grabbable.content_parent())?;
		item.auto_size_toplevel()?;

		let title_style = TextStyle {
			character_height: CHILD_THICKNESS, // * 1.5,
			text_align: Alignment::XLeft | Alignment::YBottom,
			..Default::default()
		};
		let title_text = Text::create(
			&item,
			Transform::from_position_rotation(
				[
					surface.physical_size().x * 0.5,
					surface.physical_size().y * 0.5,
					-CHILD_THICKNESS,
				],
				Quat::from_rotation_x(-PI * 0.5) * Quat::from_rotation_y(-PI * 0.5),
			),
			&data.toplevel.title.clone().unwrap_or_default(),
			title_style,
		)
		.unwrap();

		let panel_shell_grab_ball_anchor = Spatial::create(
			&item,
			Transform::from_position([0.0, -surface.physical_size().y * 0.5, 0.0]),
			false,
		)
		.unwrap();
		let panel_shell_grab_ball = PanelShellGrabBall::create(
			panel_shell_grab_ball_anchor,
			[0.0, -0.02, 0.0],
			item.alias(),
		)
		.unwrap();

		Ok(Toplevel {
			_item: item,
			surface,
			grabbable,
			title_text,
			title: data.toplevel.title.clone(),
			app_id: data.toplevel.app_id.clone(),
			children: FxHashMap::default(),
			panel_shell_grab_ball,
		})
	}

	fn initial_position_item(client: &Client, item: &Spatial) -> Result<(), NodeError> {
		let hmd_alias = client.get_hmd().alias();
		let item_alias = item.alias();
		let future = item.get_position_rotation_scale(client.get_root())?;
		tokio::spawn(async move {
			let Ok((position, _, _)) = future.await else {return};
			let position = Vec3::from(position);
			// if the distance between the panel item and the client origin is basically nothing, it must be unpositioned
			if position.length_squared() < 0.01 {
				// so we want to position it in front of the user
				let _ = item_alias.set_transform(
					Some(&hmd_alias),
					Transform::from_position_rotation(vec3(0.0, 0.0, -0.25), Quat::IDENTITY),
				);
				return;
			}
			// otherwise make the panel look at the user

			// let _ = item_alias
			// .set_transform(Some(&hmd_alias), Transform::from_rotation(Quat::IDENTITY));
		});
		Ok(())
	}

	pub fn update(
		&mut self,
		info: &FrameInfo,
		acceptors: &FxHashMap<String, (ItemAcceptor<PanelItem>, UnknownField)>,
	) {
		self.grabbable.update(info).unwrap();
		self.panel_shell_grab_ball.update(acceptors);
		if !self.grabbable.grab_action().actor_acting() {
			self.surface.update();
			for popup in self.children.values_mut() {
				popup.update();
			}
		}
	}

	pub fn update_title(&mut self) {
		let app_name = self
			.app_id
			.as_ref()
			.map(|id| id.split('.').last().unwrap_or_default());
		let title = match (&self.app_id, app_name) {
			(Some(title), Some(app_name)) => {
				if title == app_name {
					title.to_string()
				} else {
					format!("{title} - {app_name}")
				}
			}
			(Some(title), None) => title.to_string(),
			(None, Some(app_name)) => app_name.to_string(),
			(None, None) => String::new(),
		};

		self.title_text.set_text(title).unwrap();
	}

	pub fn set_enabled(&mut self, enabled: bool) {
		let _ = self.grabbable.set_enabled(enabled);
		let _ = self.panel_shell_grab_ball.set_enabled(enabled);
		for child in self.children.values_mut() {
			child.set_enabled(enabled);
		}
		self.surface.set_enabled(enabled);
		let _ = self.title_text.set_enabled(enabled);
	}
}

impl PanelItemHandler for Toplevel {
	fn toplevel_title_changed(&mut self, title: &str) {
		self.title.replace(title.to_string());
		self.update_title();
	}
	fn toplevel_app_id_changed(&mut self, app_id: &str) {
		self.app_id.replace(app_id.to_string());
		self.update_title();
	}

	fn toplevel_size_changed(&mut self, size: mint::Vector2<u32>) {
		self.surface.resize(size).unwrap();
		self.title_text
			.set_position(
				None,
				[
					self.surface.physical_size().x * 0.5,
					self.surface.physical_size().y * 0.5,
					-CHILD_THICKNESS,
				],
			)
			.unwrap();
		self.panel_shell_grab_ball
			.connect_root()
			.set_position(None, [0.0, -self.surface.physical_size().y * 0.5, 0.0])
			.unwrap();
	}

	fn new_child(&mut self, uid: &str, info: ChildInfo) {
		let parent = match &info.parent {
			SurfaceID::Cursor => return,
			SurfaceID::Toplevel => &self.surface,
			SurfaceID::Child(parent_uid) => {
				if let Some(child) = self.children.get(parent_uid) {
					child
				} else {
					return;
				}
			}
		};
		let surface =
			Surface::new_child(parent, uid.to_string(), &info.geometry, CHILD_THICKNESS).unwrap();
		self.children.insert(uid.to_string(), surface);
		let _ = self.surface.touch_plane.set_enabled(false);
	}
	fn reposition_child(&mut self, uid: &str, geometry: Geometry) {
		let Some(child) = self.children.get_mut(uid) else {return};
		child.set_offset(geometry.origin).unwrap();
		child.resize(geometry.size).unwrap();
	}
	fn drop_child(&mut self, uid: &str) {
		self.children.remove(uid);
		if self.children.is_empty() {
			let _ = self.surface.touch_plane.set_enabled(true);
		}
	}
}
