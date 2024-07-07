use crate::{
	grab_ball::GrabBallSettings,
	surface::{Surface, PPM},
};
use glam::{vec2, vec3, Mat4, Quat, Vec2, Vec3, Vec3Swizzles};
use stardust_xr_fusion::{
	core::values::ResourceID,
	drawable::{MaterialParameter, Model, ModelPart, ModelPartAspect},
	fields::{Field, Shape},
	input::{InputDataType, InputHandler},
	items::panel::{PanelItem, PanelItemAspect, ToplevelInfo},
	node::{NodeResult, NodeType},
	root::Root,
	spatial::{SpatialAspect, SpatialRef, SpatialRefAspect, Transform},
	values::{color::rgba_linear, Vector2},
};
use stardust_xr_molecules::input_action::{InputQueue, InputQueueable, SingleAction};
use std::f32::consts::FRAC_PI_2;

const RESIZE_HANDLE_FLOATING: f32 = 0.025;

async fn pos(transform: &impl SpatialRefAspect, relative_to: &impl SpatialRefAspect) -> Vec3 {
	transform
		.get_transform(relative_to)
		.await
		.unwrap()
		.translation
		.map(Into::into)
		.unwrap_or_default()
}

pub struct ResizeHandles {
	root: Root,
	hmd: SpatialRef,
	item: PanelItem,
	bottom_left: ResizeHandle,
	top_right: ResizeHandle,

	min_size: Option<Vector2<f32>>,
	max_size: Option<Vector2<f32>>,
}
impl ResizeHandles {
	pub fn create(
		hmd: SpatialRef,
		item: &PanelItem,
		surface: &Surface,
		toplevel_data: &ToplevelInfo,
	) -> NodeResult<Self> {
		let settings = GrabBallSettings {
			radius: 0.005,
			padding: 0.02,
			connector_thickness: 0.0025,
			connector_color: rgba_linear!(0.0, 1.0, 0.5, 1.0),
		};

		let root = hmd.client()?.get_root().alias();
		let bottom_left = ResizeHandle::create(&root, settings.clone())?;
		let top_right = ResizeHandle::create(&root, settings.clone())?;

		let _ = top_right.model.set_spatial_parent_in_place(item);
		let _ = bottom_left.model.set_spatial_parent_in_place(item);
		let _ = item.set_zoneable(true);

		let mut resize_handles = ResizeHandles {
			root,
			hmd,
			item: item.alias(),
			bottom_left,
			top_right,

			min_size: toplevel_data.min_size,
			max_size: toplevel_data.max_size,
		};
		resize_handles.set_handle_positions(surface.physical_size());
		Ok(resize_handles)
	}
	pub fn update(&mut self) {
		self.bottom_left.update();
		self.top_right.update();
		if (self.top_right.grab_action.actor_started()
			&& !self.bottom_left.grab_action.actor_acting())
			|| (self.bottom_left.grab_action.actor_started()
				&& !self.top_right.grab_action.actor_acting())
		{
			let _ = self.top_right.model.set_spatial_parent_in_place(&self.root);
			let _ = self
				.bottom_left
				.model
				.set_spatial_parent_in_place(&self.root);
			let _ = self.item.set_zoneable(false);
		}
		if self.top_right.grab_action.actor_acting() || self.bottom_left.grab_action.actor_acting()
		{
			self.update_panel_transform();
		}

		if (self.top_right.grab_action.actor_stopped()
			&& !self.bottom_left.grab_action.actor_acting())
			|| (self.bottom_left.grab_action.actor_stopped()
				&& !self.top_right.grab_action.actor_acting())
		{
			let _ = self.top_right.model.set_spatial_parent_in_place(&self.item);
			let _ = self
				.bottom_left
				.model
				.set_spatial_parent_in_place(&self.item);
			let _ = self.item.set_zoneable(true);
		}
	}
	fn update_panel_transform(&self) {
		let root = self.root.alias();
		let hmd = self.hmd.alias();
		let item = self.item.alias();
		let corner1 = self.bottom_left.model.alias();
		let corner2 = self.top_right.model.alias();

		let min_size = self.min_size.unwrap_or([0.0; 2].into());
		let max_size = self.max_size.unwrap_or([4096.0; 2].into());

		tokio::task::spawn(async move {
			let hmd_pos = pos(&hmd, &root).await;
			let mut corner1 = pos(&corner1, &root).await;
			let mut corner2 = pos(&corner2, &root).await;
			let center_point = (corner1 + corner2) * 0.5;

			let center_hmd_relative = center_point - hmd_pos;
			let y_angle = center_hmd_relative.xz().to_angle() + FRAC_PI_2;
			let y_rotation = Quat::from_rotation_y(y_angle).inverse();

			let y_aligner = Mat4::from_translation(hmd_pos).inverse()
				* Mat4::from_rotation_y(y_angle)
				* Mat4::from_translation(hmd_pos);
			corner1 = y_aligner.transform_point3(corner1);
			corner2 = y_aligner.transform_point3(corner2);

			let corner1_2d = corner1.zy();
			let corner2_2d = corner2.zy();
			let x_angle = (corner1_2d - corner2_2d).to_angle() + FRAC_PI_2;
			let x_rotation = Quat::from_rotation_x(x_angle).inverse();

			let mut size = vec2(
				(corner1.x - corner2.x).abs() - (RESIZE_HANDLE_FLOATING * 2.0),
				corner1_2d.distance(corner2_2d) - (RESIZE_HANDLE_FLOATING * 2.0),
			) * PPM;
			size.x = size.x.max(min_size.x).min(max_size.x);
			size.y = size.y.max(min_size.y).min(max_size.y);

			let _ = item.set_relative_transform(
				&root,
				Transform::from_translation_rotation(center_point, y_rotation * x_rotation),
			);
			let _ = item.set_toplevel_size([size.x as u32, size.y as u32]);
		});
	}
	pub fn set_handle_positions(&mut self, panel_size: Vec2) {
		let offset = vec3(
			panel_size.x + RESIZE_HANDLE_FLOATING,
			panel_size.y + RESIZE_HANDLE_FLOATING,
			0.0,
		) * 0.5;
		if !self.top_right.grab_action.actor_acting()
			&& !self.bottom_left.grab_action.actor_acting()
		{
			self.top_right.set_pos(&self.item, offset);
			self.bottom_left.set_pos(&self.item, -offset);
		}
	}
	pub fn set_enabled(&mut self, enabled: bool) {
		let _ = self.top_right.set_enabled(enabled);
		let _ = self.bottom_left.set_enabled(enabled);
	}
}

pub struct ResizeHandle {
	settings: GrabBallSettings,

	model: Model,
	sphere: ModelPart,
	_field: Field,
	input: InputQueue,
	grab_action: SingleAction,
}
impl ResizeHandle {
	pub fn create(root: &impl SpatialRefAspect, settings: GrabBallSettings) -> NodeResult<Self> {
		let model = Model::create(
			root,
			Transform::identity(),
			&ResourceID::new_namespaced("flatland", "resize_handle"),
		)?;
		let sphere = model.part("sphere")?;
		sphere.set_material_parameter(
			"color",
			MaterialParameter::Color(rgba_linear!(0.75, 0.75, 0.75, 1.0)),
		)?;

		let field = Field::create(&model, Transform::identity(), Shape::Sphere(0.005))?;
		let input = InputHandler::create(root, Transform::identity(), &field)?.queue()?;

		model.set_spatial_parent(input.handler())?;

		Ok(ResizeHandle {
			settings,

			model,
			sphere,
			_field: field,
			input,
			grab_action: Default::default(),
		})
	}
}
impl ResizeHandle {
	pub fn update(&mut self) {
		self.grab_action.update(
			true,
			&self.input,
			|input| match &input.input {
				InputDataType::Pointer(_) => false,
				_ => input.distance < (self.settings.radius + self.settings.padding),
			},
			|input| {
				input.datamap.with_data(|datamap| match &input.input {
					InputDataType::Hand(_) => datamap.idx("pinch_strength").as_f32() > 0.90,
					_ => datamap.idx("grab").as_f32() > 0.90,
				})
			},
		);

		// if something just got close
		if self.grab_action.hovering().added().len() > 0
			&& self.grab_action.hovering().added().len()
				== self.grab_action.hovering().current().len()
		{
			let _ = self.sphere.set_material_parameter(
				"color",
				MaterialParameter::Color(rgba_linear!(1.0, 1.0, 1.0, 1.0)),
			);
		}

		if self.grab_action.hovering().current().len() == 0
			&& self.grab_action.hovering().removed().len() > 0
		{
			let _ = self.sphere.set_material_parameter(
				"color",
				MaterialParameter::Color(rgba_linear!(0.5, 0.5, 0.5, 1.0)),
			);
		}

		if self.grab_action.actor_started() {
			let _ = self.sphere.set_material_parameter(
				"color",
				MaterialParameter::Color(rgba_linear!(0.0, 1.0, 0.25, 1.0)),
			);
		}
		if let Some(grab_point) = self.grab_point() {
			self.set_pos(&self.input.handler().alias(), grab_point);
		}
		if self.grab_action.actor_stopped() {
			let _ = self.sphere.set_material_parameter(
				"color",
				MaterialParameter::Color(rgba_linear!(0.5, 0.5, 0.5, 1.0)),
			);
		}
	}
	fn grab_point(&self) -> Option<Vec3> {
		let grabbing = self.grab_action.actor()?;
		match &grabbing.input {
			InputDataType::Pointer(_) => None,
			InputDataType::Hand(h) => {
				Some(Vec3::from(h.thumb.tip.position).lerp(Vec3::from(h.index.tip.position), 0.5))
			}
			InputDataType::Tip(t) => Some(t.origin.into()),
		}
	}
	pub fn set_pos(&self, relative_to: &impl SpatialRefAspect, pos: Vec3) {
		let _ = self
			.model
			.set_relative_transform(relative_to, Transform::from_translation(pos));
	}
	fn set_enabled(&mut self, enabled: bool) {
		let _ = self.model.set_enabled(enabled);
	}
}
