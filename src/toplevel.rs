use crate::{
	close_button::CloseButton,
	grab_ball::{GrabBall, GrabBallSettings},
	panel_shell_transfer::PanelShellTransfer,
	surface::Surface,
};
use color::rgba_linear;
use glam::{vec3, Quat, Vec3};
use rustc_hash::FxHashMap;
use stardust_xr_fusion::{
	client::{Client, FrameInfo},
	drawable::{Text, TextAspect, TextBounds, TextFit, TextStyle, XAlign, YAlign},
	fields::UnknownField,
	items::{
		panel::{ChildInfo, Geometry, PanelItem, PanelItemHandler, PanelItemInitData, SurfaceID},
		ItemAcceptor,
	},
	node::{NodeError, NodeType},
	spatial::{Spatial, SpatialAspect, Transform},
};
use stardust_xr_molecules::{Grabbable, GrabbableSettings, PointerMode};
use std::{f32::consts::PI, sync::Arc};
use tokio::task::JoinHandle;

fn look_direction(direction: Vec3) -> Quat {
	let pitch = direction.y.asin();
	let yaw = direction.z.atan2(direction.x);
	Quat::from_rotation_y(-yaw - PI / 2.0) * Quat::from_rotation_x(pitch)
}

pub const GRAB_FIELD_PADDING: f32 = 0.01;
pub const TOPLEVEL_THICKNESS: f32 = 0.01;
pub const CHILD_THICKNESS: f32 = 0.005;

pub struct Toplevel {
	_item: PanelItem,
	surface: Surface,
	grabbable: Grabbable,
	title_text: Text,
	title: Option<String>,
	app_id: Option<String>,
	children: FxHashMap<String, Surface>,
	panel_shell_grab_ball: GrabBall<PanelShellTransfer>,
	close_button: CloseButton,
}
impl Toplevel {
	pub fn create(item: PanelItem, data: PanelItemInitData) -> Result<Self, NodeError> {
		let client = item.client()?;

		let surface = Surface::create(
			&item,
			Transform::none(),
			item.alias(),
			SurfaceID::Toplevel,
			data.toplevel.size,
			TOPLEVEL_THICKNESS,
		)?;
		surface
			.root()
			.set_local_transform(Transform::from_translation(
				vec3(surface.physical_size().x, -surface.physical_size().y, 0.0) * -0.5,
			))?;
		let grabbable = Grabbable::create(
			client.get_root(),
			Transform::none(),
			&surface.field(),
			GrabbableSettings {
				linear_momentum: None,
				angular_momentum: None,
				magnet: false,
				pointer_mode: PointerMode::Align,
				max_distance: 0.05,
				..Default::default()
			},
		)?;
		grabbable
			.content_parent()
			.set_relative_transform(&item, Transform::identity())?;
		item.set_spatial_parent(grabbable.content_parent())?;
		item.set_local_transform(Transform::identity())?;
		item.auto_size_toplevel()?;

		Self::initial_position_item(client.clone(), grabbable.content_parent().alias())?;

		let title_style = TextStyle {
			character_height: CHILD_THICKNESS, // * 1.5,
			text_align_x: XAlign::Left,
			text_align_y: YAlign::Bottom,
			bounds: Some(TextBounds {
				bounds: [surface.physical_size().y, CHILD_THICKNESS].into(),
				fit: TextFit::Squeeze,
				anchor_align_x: XAlign::Left,
				anchor_align_y: YAlign::Bottom,
			}),
			..Default::default()
		};
		let title_text = Text::create(
			&item,
			Transform::from_translation_rotation(
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
			Transform::from_translation([0.0, -surface.physical_size().y * 0.5, 0.0]),
			false,
		)
		.unwrap();
		let panel_shell_transfer =
			PanelShellTransfer::create(surface.root(), item.alias()).unwrap();
		let panel_shell_grab_ball = GrabBall::create(
			panel_shell_grab_ball_anchor,
			[0.0, -0.02, 0.0],
			panel_shell_transfer,
			GrabBallSettings {
				radius: 0.01252,
				connector_thickness: 0.0025,
				connector_color: rgba_linear!(0.0, 1.0, 0.5, 1.0),
			},
		)
		.unwrap();
		let close_button = CloseButton::new(item.alias(), TOPLEVEL_THICKNESS, &surface)?;

		Ok(Toplevel {
			_item: item,
			surface,
			grabbable,
			title_text,
			title: data.toplevel.title.clone(),
			app_id: data.toplevel.app_id.clone(),
			children: FxHashMap::default(),
			panel_shell_grab_ball,
			close_button,
		})
	}

	fn initial_position_item(
		client: Arc<Client>,
		grabbable_content_parent: Spatial,
	) -> Result<(), NodeError> {
		let _: JoinHandle<Result<(), NodeError>> = tokio::spawn(async move {
			let distance_future = grabbable_content_parent.get_transform(client.get_root());
			let hmd_future = client.get_hmd().get_transform(client.get_root());

			let Transform {
				translation: item_translation,
				..
			} = distance_future.await?;
			// if the distance between the panel item and the client origin is basically nothing, it must be unpositioned
			if Vec3::from(item_translation.unwrap()).length_squared() < 0.001 {
				// so we want to position it in front of the user
				let _ = grabbable_content_parent.set_relative_transform(
					client.get_hmd(),
					Transform::from_translation_rotation(vec3(0.0, 0.0, -0.25), Quat::IDENTITY),
				);
				return Ok(());
			}

			// otherwise make the panel look at the user
			let Transform {
				translation: hmd_translation,
				..
			} = hmd_future.await?;
			let look_rotation = look_direction(
				(Vec3::from(item_translation.unwrap()) - Vec3::from(hmd_translation.unwrap()))
					.normalize(),
			);
			let _ = grabbable_content_parent
				.set_relative_transform(client.get_root(), Transform::from_rotation(look_rotation));
			Ok(())
		});
		Ok(())
	}

	pub fn update(
		&mut self,
		info: &FrameInfo,
		acceptors: &FxHashMap<String, (ItemAcceptor<PanelItem>, UnknownField)>,
	) {
		self.grabbable.update(info).unwrap();
		if !self.grabbable.grab_action().actor_acting() {
			self.surface.update();
			for popup in self.children.values_mut() {
				popup.update();
			}
		}
		self.close_button.update(info);
		self.panel_shell_grab_ball.update();
		self.panel_shell_grab_ball
			.head
			.update_distances(self.panel_shell_grab_ball.grab_action(), acceptors);
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

		self.title_text.set_text(&title).unwrap();
	}

	pub fn set_enabled(&mut self, enabled: bool) {
		let _ = self.grabbable.set_enabled(enabled);
		let _ = self.panel_shell_grab_ball.set_enabled(enabled);
		for child in self.children.values_mut() {
			child.set_enabled(enabled);
		}
		self.surface.set_enabled(enabled);
		let _ = self.title_text.set_enabled(enabled);
		let _ = self.close_button.set_enabled(enabled);
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
			.set_local_transform(Transform::from_translation([
				self.surface.physical_size().x * 0.5,
				self.surface.physical_size().y * 0.5,
				-CHILD_THICKNESS,
			]))
			.unwrap();
		self.panel_shell_grab_ball
			.connect_root()
			.set_local_transform(Transform::from_translation([
				0.0,
				-self.surface.physical_size().y * 0.5,
				0.0,
			]))
			.unwrap();
		self.close_button.resize(&self.surface);
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
		let _ = self.surface.hover_plane.set_enabled(false);
		let _ = self.surface.touch_plane.set_enabled(false);
	}
	fn reposition_child(&mut self, uid: &str, geometry: Geometry) {
		let Some(child) = self.children.get_mut(uid) else {
			return;
		};
		child.set_offset(geometry.origin).unwrap();
		child.resize(geometry.size).unwrap();
	}
	fn drop_child(&mut self, uid: &str) {
		self.children.remove(uid);
		if self.children.is_empty() {
			let _ = self.surface.hover_plane.set_enabled(true);
			let _ = self.surface.touch_plane.set_enabled(true);
		}
	}
}
