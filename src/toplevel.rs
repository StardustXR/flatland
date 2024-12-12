use crate::{
	close_button::ExposureButtonInner,
	grab_ball::{GrabBall, GrabBallSettings},
	panel_shell_transfer::PanelShellTransfer,
	resize_handles::ResizeHandlesInner,
};
use glam::{vec3, Quat};
use rustc_hash::FxHashMap;
use stardust_xr_fusion::{
	core::values::color::rgba_linear,
	drawable::{Text, TextAspect, TextBounds, TextFit, TextStyle, XAlign, YAlign},
	fields::Field,
	items::panel::{
		PanelItem, PanelItemAcceptor, PanelItemAspect, PanelItemEvent, PanelItemInitData, SurfaceId,
	},
	node::{NodeError, NodeType},
	root::FrameInfo,
	spatial::{Spatial, SpatialAspect, Transform},
	values::Color,
};
use std::f32::consts::PI;

pub const GRAB_FIELD_PADDING: f32 = 0.01;
pub const TOPLEVEL_THICKNESS: f32 = 0.01;
pub const CHILD_THICKNESS: f32 = 0.005;

pub struct ToplevelInner {
	item: PanelItem,
	surface: Surface,
	children: FxHashMap<u64, Surface>,

	title_text: Text,
	title: Option<String>,
	app_id: Option<String>,

	resize_handles: ResizeHandlesInner,
	panel_shell_grab_ball: GrabBall<PanelShellTransfer>,
	close_button: ExposureButtonInner,
}
impl ToplevelInner {
	pub fn create(
		accent_color: Color,
		item: PanelItem,
		data: PanelItemInitData,
	) -> Result<Self, NodeError> {
		let surface = Surface::create(
			&item.clone(),
			Transform::identity(),
			item.clone(),
			SurfaceId::Toplevel(()),
			data.toplevel.size,
			TOPLEVEL_THICKNESS,
			true,
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

		let resize_handles = ResizeHandlesInner::create(
			item.clone().as_item().as_spatial().as_spatial_ref(),
			accent_color,
			surface.physical_size().into(),
			data.toplevel.min_size,
			data.toplevel.max_size,
		)
		.unwrap();

		let panel_shell_grab_ball_anchor = Spatial::create(
			&item,
			Transform::from_translation([0.0, -surface.physical_size().y * 0.5, 0.0]),
			false,
		)
		.unwrap();
		let panel_shell_transfer =
			PanelShellTransfer::create(surface.root(), item.clone()).unwrap();
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
		let close_button =
			ExposureButtonInner::new(&item, Transform::identity(), TOPLEVEL_THICKNESS)?;

		Ok(ToplevelInner {
			item,
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

	pub fn frame(&mut self, info: &FrameInfo) {
		self.close_button.frame(info);
	}

	pub fn handle_events(&mut self, acceptors: &FxHashMap<u64, (PanelItemAcceptor, Field)>) {
		self.surface.handle_events();
		for popup in self.children.values_mut() {
			popup.handle_events();
		}
		self.panel_shell_grab_ball.update();
		self.panel_shell_grab_ball
			.head
			.update_distances(self.panel_shell_grab_ball.grab_action(), acceptors);
		self.resize_handles.handle_events();
		self.handle_item_events();
	}

	pub fn handle_item_events(&mut self) {
		while let Some(panel_event) = self.item.recv_panel_item_event() {
			match panel_event {
				PanelItemEvent::ToplevelTitleChanged { title } => {
					self.title.replace(title);
					self.update_title();
				}
				PanelItemEvent::ToplevelAppIdChanged { app_id } => {
					self.app_id.replace(app_id);
					self.update_title();
				}
				PanelItemEvent::ToplevelSizeChanged { size } => {
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
					// self.close_button.resize(&self.surface);
					self.resize_handles
						.set_handle_positions(self.surface.physical_size().into());
				}
				PanelItemEvent::CreateChild { uid, info } => {
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
					let surface = Surface::new_child(
						parent,
						uid,
						&info.geometry,
						CHILD_THICKNESS,
						info.receives_input,
					)
					.unwrap();
					self.children.insert(uid, surface);
					if let Some(input) = &mut self.surface.input {
						input.set_enabled(false);
					}
				}
				PanelItemEvent::RepositionChild { uid, geometry } => {
					let Some(child) = self.children.get_mut(&uid) else {
						return;
					};
					child.set_offset(geometry.origin).unwrap();
					child.resize(geometry.size).unwrap();
				}
				PanelItemEvent::DestroyChild { uid } => {
					self.children.remove(&uid);
					if !self.children.values().any(|child| child.input.is_some()) {
						if let Some(input) = &mut self.surface.input {
							input.set_enabled(true);
						}
					}
				}
				_ => (),
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
