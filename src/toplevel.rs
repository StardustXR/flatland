use crate::{
	close_button::CloseButton,
	grab_ball::{GrabBall, GrabBallSettings},
	panel_shell_transfer::PanelShellTransfer,
	resize_handles::ResizeHandles,
	surface::Surface,
};
use glam::{vec3, Quat, Vec3};
use rustc_hash::FxHashMap;
use stardust_xr_fusion::{
	core::values::{color::rgba_linear, Vector2},
	drawable::{Text, TextAspect, TextBounds, TextFit, TextStyle, XAlign, YAlign},
	fields::Field,
	items::panel::{
		ChildInfo, Geometry, PanelItem, PanelItemAcceptor, PanelItemAspect, PanelItemHandler,
		PanelItemInitData, SurfaceId,
	},
	node::{NodeError, NodeResult, NodeType},
	root::FrameInfo,
	spatial::{Spatial, SpatialAspect, SpatialRef, SpatialRefAspect, Transform},
	values::Color,
};
use std::f32::consts::PI;

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
	children: FxHashMap<u64, Surface>,

	title_text: Text,
	title: Option<String>,
	app_id: Option<String>,

	resize_handles: ResizeHandles,
	panel_shell_grab_ball: GrabBall<PanelShellTransfer>,
	close_button: CloseButton,
}
impl Toplevel {
	pub fn create(
		accent_color: Color,
		hmd: SpatialRef,
		item: PanelItem,
		data: PanelItemInitData,
	) -> Result<Self, NodeError> {
		let surface = Surface::create(
			&item,
			Transform::identity(),
			item.alias(),
			SurfaceId::Toplevel(()),
			data.toplevel.size,
			TOPLEVEL_THICKNESS,
		)?;
		surface
			.root()
			.set_local_transform(Transform::from_translation(
				vec3(
					-surface.physical_size().x,
					surface.physical_size().y,
					TOPLEVEL_THICKNESS,
				) * 0.5,
			))?;
		tokio::task::spawn(Self::initial_position_item(hmd.alias(), item.alias()));
		item.auto_size_toplevel()?;

		let title_style = TextStyle {
			character_height: TOPLEVEL_THICKNESS, // * 1.5,
			text_align_x: XAlign::Left,
			text_align_y: YAlign::Bottom,
			bounds: Some(TextBounds {
				bounds: [surface.physical_size().y, TOPLEVEL_THICKNESS].into(),
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
					-TOPLEVEL_THICKNESS,
				],
				Quat::from_rotation_x(-PI * 0.5) * Quat::from_rotation_y(-PI * 0.5),
			),
			&data.toplevel.title.clone().unwrap_or_default(),
			title_style,
		)
		.unwrap();

		let resize_handles =
			ResizeHandles::create(accent_color, hmd, &item, &surface, &data.toplevel).unwrap();

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
				padding: 0.0,
				connector_thickness: 0.0025,
				connector_color: rgba_linear!(0.0, 1.0, 0.5, 1.0),
			},
		)
		.unwrap();
		let close_button = CloseButton::new(item.alias(), TOPLEVEL_THICKNESS, &surface)?;

		Ok(Toplevel {
			_item: item,
			surface,
			children: FxHashMap::default(),

			title_text,
			title: data.toplevel.title.clone(),
			app_id: data.toplevel.app_id.clone(),

			resize_handles,
			panel_shell_grab_ball,
			close_button,
		})
	}

	async fn initial_position_item(hmd: SpatialRef, item: PanelItem) -> NodeResult<()> {
		let root = hmd.client()?.get_root().alias();

		let Transform {
			translation: item_translation,
			..
		} = item.get_transform(&root).await?;
		// if the distance between the panel item and the client origin is basically nothing, it must be unpositioned
		if Vec3::from(item_translation.unwrap()).length_squared() < 0.001 {
			// so we want to position it in front of the user
			let _ = item.set_relative_transform(
				&hmd,
				Transform::from_translation_rotation(vec3(0.0, 0.0, -0.25), Quat::IDENTITY),
			);
			return Ok(());
		}

		// otherwise make the panel look at the user
		let Transform {
			translation: hmd_translation,
			..
		} = hmd.get_transform(&root).await?;
		let look_rotation = look_direction(
			(Vec3::from(item_translation.unwrap()) - Vec3::from(hmd_translation.unwrap()))
				.normalize(),
		);
		let _ = item.set_relative_transform(&root, Transform::from_rotation(look_rotation));

		Ok(())
	}

	pub fn update(
		&mut self,
		info: &FrameInfo,
		acceptors: &FxHashMap<u64, (PanelItemAcceptor, Field)>,
	) {
		self.surface.update();
		for popup in self.children.values_mut() {
			popup.update();
		}
		self.close_button.update(info);
		self.panel_shell_grab_ball.update();
		self.panel_shell_grab_ball
			.head
			.update_distances(self.panel_shell_grab_ball.grab_action(), acceptors);
		self.resize_handles.update();
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
		self.panel_shell_grab_ball.set_enabled(enabled);
		for child in self.children.values_mut() {
			child.set_enabled(enabled);
		}
		self.surface.set_enabled(enabled);
		let _ = self.title_text.set_enabled(enabled);
		self.close_button.set_enabled(enabled);
		self.resize_handles.set_enabled(enabled);
	}
}

impl PanelItemHandler for Toplevel {
	fn toplevel_title_changed(&mut self, title: String) {
		self.title.replace(title);
		self.update_title();
	}
	fn toplevel_app_id_changed(&mut self, app_id: String) {
		self.app_id.replace(app_id);
		self.update_title();
	}

	fn set_cursor(&mut self, _geometry: Geometry) {}
	fn hide_cursor(&mut self) {}

	fn toplevel_fullscreen_active(&mut self, _active: bool) {}
	fn toplevel_move_request(&mut self) {}
	fn toplevel_resize_request(&mut self, _up: bool, _down: bool, _left: bool, _right: bool) {}
	fn toplevel_parent_changed(&mut self, _parent_id: u64) {}
	fn toplevel_size_changed(&mut self, size: Vector2<u32>) {
		self.surface.resize(size).unwrap();
		self.surface
			.root()
			.set_local_transform(Transform::from_translation(
				vec3(
					-self.surface.physical_size().x,
					self.surface.physical_size().y,
					TOPLEVEL_THICKNESS,
				) * 0.5,
			))
			.unwrap();
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
		self.resize_handles
			.set_handle_positions(self.surface.physical_size());
	}

	fn create_child(&mut self, id: u64, info: ChildInfo) {
		let parent = match &info.parent {
			SurfaceId::Toplevel(_) => &self.surface,
			SurfaceId::Child(parent_uid) => {
				if let Some(child) = self.children.get(parent_uid) {
					child
				} else {
					return;
				}
			}
		};
		let surface = Surface::new_child(parent, id, &info.geometry, CHILD_THICKNESS).unwrap();
		self.children.insert(id, surface);
		let _ = self.surface.hover_plane.set_enabled(false);
		let _ = self.surface.touch_plane.set_enabled(false);
	}
	fn reposition_child(&mut self, id: u64, geometry: Geometry) {
		let Some(child) = self.children.get_mut(&id) else {
			return;
		};
		child.set_offset(geometry.origin).unwrap();
		child.resize(geometry.size).unwrap();
	}
	fn destroy_child(&mut self, id: u64) {
		self.children.remove(&id);
		if self.children.is_empty() {
			let _ = self.surface.hover_plane.set_enabled(true);
			let _ = self.surface.touch_plane.set_enabled(true);
		}
	}
}
