use crate::grab_ball::GrabBallSettings;
use asteroids::{custom::ElementTrait, ValidState};
use derive_setters::Setters;
use glam::{vec2, vec3, Mat4, Quat, Vec3, Vec3Swizzles};
use stardust_xr_fusion::{
	core::values::ResourceID,
	drawable::{MaterialParameter, Model, ModelPart, ModelPartAspect},
	fields::{Field, Shape},
	input::{InputDataType, InputHandler},
	node::{NodeError, NodeResult, NodeType},
	objects::hmd,
	spatial::{Spatial, SpatialAspect, SpatialRef, SpatialRefAspect, Transform},
	values::{color::rgba_linear, Color, Vector2},
};
use stardust_xr_molecules::{
	input_action::{InputQueue, InputQueueable, SingleAction},
	UIElement,
};
use std::f32::consts::{FRAC_PI_2, PI};
use tokio::sync::watch;
use tracing::info;

fn look_direction(direction: Vec3) -> Quat {
	let pitch = direction.y.asin();
	let yaw = direction.z.atan2(direction.x);
	Quat::from_rotation_y(-yaw - PI / 2.0) * Quat::from_rotation_x(pitch)
}

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
	content_parent: Spatial,
	bottom_left: ResizeHandle,
	top_right: ResizeHandle,

	size_tx: watch::Sender<Vector2<f32>>,
	size: watch::Receiver<Vector2<f32>>,
	pub min_size: Option<Vector2<f32>>,
	pub max_size: Option<Vector2<f32>>,
}
impl ResizeHandles {
	pub fn create(
		initial_pose: SpatialRef,
		accent_color: Color,
		initial_size: Vector2<f32>,
		min_size_px: Option<Vector2<f32>>,
		max_size_px: Option<Vector2<f32>>,
	) -> NodeResult<Self> {
		let settings = GrabBallSettings {
			radius: 0.005,
			padding: 0.02,
			connector_thickness: 0.0025,
			connector_color: accent_color,
		};

		let client = initial_pose.client().unwrap().clone();
		let root = client.get_root();
		let bottom_left = ResizeHandle::create(root, settings.clone())?;
		let top_right = ResizeHandle::create(root, settings.clone())?;

		let content_parent = Spatial::create(&initial_pose, Transform::identity(), true)?;
		let _ = top_right.model.set_spatial_parent(&content_parent);
		let _ = bottom_left.model.set_spatial_parent(&content_parent);
		content_parent.set_spatial_parent_in_place(root)?;
		tokio::task::spawn(Self::initial_position_item(content_parent.clone()));

		let (size_tx, size) = watch::channel(initial_size);
		let mut resize_handles = ResizeHandles {
			content_parent,
			bottom_left,
			top_right,

			size_tx,
			size,
			min_size: min_size_px,
			max_size: max_size_px,
		};
		resize_handles.set_handle_positions(initial_size);
		Ok(resize_handles)
	}
	async fn initial_position_item(spatial_root: Spatial) -> NodeResult<()> {
		let client = spatial_root.client()?;
		let Some(hmd) = hmd(&client).await else {
			return Err(NodeError::DoesNotExist);
		};
		let root = client.get_root();

		let Transform {
			translation: item_translation,
			..
		} = spatial_root.get_transform(root).await?;
		// if the distance between the panel item and the client origin is basically nothing, it must be unpositioned
		if Vec3::from(item_translation.unwrap()).length_squared() < 0.001 {
			// so we want to position it in front of the user
			let _ = spatial_root.set_relative_transform(
				&hmd,
				Transform::from_translation_rotation(vec3(0.0, 0.0, -0.25), Quat::IDENTITY),
			);
			return Ok(());
		}

		// otherwise make the panel look at the user
		let Transform {
			translation: hmd_translation,
			..
		} = hmd.get_transform(root).await?;
		let look_rotation = look_direction(
			(Vec3::from(item_translation.unwrap()) - Vec3::from(hmd_translation.unwrap()))
				.normalize(),
		);
		let _ = spatial_root.set_relative_transform(root, Transform::from_rotation(look_rotation));

		Ok(())
	}
	pub fn handle_events(&mut self) {
		let client = self.content_parent.client().unwrap().clone();
		let root = client.get_root();
		self.bottom_left.handle_events();
		self.top_right.handle_events();
		if (self.top_right.grab_action.actor_started()
			&& !self.bottom_left.grab_action.actor_acting())
			|| (self.bottom_left.grab_action.actor_started()
				&& !self.top_right.grab_action.actor_acting())
		{
			let _ = self.top_right.model.set_spatial_parent_in_place(root);
			let _ = self.bottom_left.model.set_spatial_parent_in_place(root);
			let _ = self.content_parent.set_zoneable(false);
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
			let _ = self
				.top_right
				.model
				.set_spatial_parent_in_place(&self.content_parent);
			let _ = self
				.bottom_left
				.model
				.set_spatial_parent_in_place(&self.content_parent);
			let _ = self.content_parent.set_zoneable(true);
		}
	}
	fn update_panel_transform(&self) {
		let client = self.content_parent.client().unwrap().clone();
		let item = self.content_parent.clone();
		let corner1 = self.bottom_left.model.clone();
		let corner2 = self.top_right.model.clone();

		let size_tx = self.size_tx.clone();
		let min_size_px = self.min_size.unwrap_or([0.0; 2].into());
		let max_size_px = self.max_size.unwrap_or([4096.0; 2].into());

		tokio::task::spawn(async move {
			let hmd = hmd(&client).await.unwrap();
			let root = client.get_root();
			let hmd_pos = pos(&hmd, root).await;
			let mut corner1 = pos(&corner1, root).await;
			let mut corner2 = pos(&corner2, root).await;
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
			);
			size.x = size.x.max(min_size_px.x).min(max_size_px.x);
			size.y = size.y.max(min_size_px.y).min(max_size_px.y);

			let _ = item.set_relative_transform(
				root,
				Transform::from_translation_rotation(center_point, y_rotation * x_rotation),
			);
			let _ = size_tx.send(size.into());
		});
	}
	pub fn set_handle_positions(&mut self, panel_size: Vector2<f32>) {
		let offset = vec3(
			panel_size.x + RESIZE_HANDLE_FLOATING,
			panel_size.y + RESIZE_HANDLE_FLOATING,
			0.0,
		) * 0.5;
		if !self.top_right.grab_action.actor_acting()
			&& !self.bottom_left.grab_action.actor_acting()
		{
			self.top_right.set_pos(&self.content_parent, offset);
			self.bottom_left.set_pos(&self.content_parent, -offset);
		}
	}
	pub fn set_enabled(&mut self, enabled: bool) {
		self.top_right.set_enabled(enabled);
		self.bottom_left.set_enabled(enabled);
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
impl UIElement for ResizeHandle {
	fn handle_events(&mut self) -> bool {
		if !self.input.handle_events() {
			info!("don't handle events");
			return false;
		}
		self.grab_action.update(
			false,
			&self.input,
			|input| match &input.input {
				InputDataType::Pointer(_) => false,
				_ => input.distance < (self.settings.radius + self.settings.padding),
			},
			|input| {
				input.datamap.with_data(|datamap| match &input.input {
					InputDataType::Hand(_) => {
						let w = datamap.idx("pinch_strength").as_f32() > 0.90;
						info!("{w}");
						w
					}
					_ => datamap.idx("grab").as_f32() > 0.90,
				})
			},
		);

		info!(":3 {}", self.grab_action.actor_acting());

		// if something just got close
		if !self.grab_action.hovering().added().is_empty()
			&& self.grab_action.hovering().added().len()
				== self.grab_action.hovering().current().len()
		{
			info!("started hover");
			let _ = self.sphere.set_material_parameter(
				"color",
				MaterialParameter::Color(rgba_linear!(1.0, 1.0, 1.0, 1.0)),
			);
		}

		if self.grab_action.hovering().current().is_empty()
			&& !self.grab_action.hovering().removed().is_empty()
		{
			info!("ended hover");
			let _ = self.sphere.set_material_parameter(
				"color",
				MaterialParameter::Color(rgba_linear!(0.5, 0.5, 0.5, 1.0)),
			);
		}

		if self.grab_action.actor_started() {
			info!("started grab");
			let _ = self.sphere.set_material_parameter(
				"color",
				MaterialParameter::Color(self.settings.connector_color),
			);
		}
		if let Some(grab_point) = self.grab_point() {
			self.set_pos(self.input.handler(), grab_point);
		}
		if self.grab_action.actor_stopped() {
			info!("ended grab");
			let _ = self.sphere.set_material_parameter(
				"color",
				MaterialParameter::Color(rgba_linear!(0.5, 0.5, 0.5, 1.0)),
			);
		}
		true
	}
}
impl ResizeHandle {
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
#[derive_where::derive_where(Debug, Clone, PartialEq)]
#[derive(Setters)]
#[setters(into, strip_option)]
pub struct ResizeHandlesElement<State: ValidState> {
	pub initial_position: SpatialRef,
	pub accent_color: Color,
	pub initial_size: Vector2<f32>,
	pub min_size: Option<Vector2<f32>>,
	pub max_size: Option<Vector2<f32>>,
	pub on_size_changed: Option<fn(&mut State, Vector2<f32>)>,
}
impl<State: ValidState> ElementTrait<State> for ResizeHandlesElement<State> {
	type Inner = ResizeHandles;
	type Error = NodeError;

	fn create_inner(&self, _spatial_parent: &SpatialRef) -> Result<Self::Inner, Self::Error> {
		ResizeHandles::create(
			self.initial_position.clone(),
			self.accent_color,
			self.initial_size,
			self.min_size,
			self.max_size,
		)
	}

	fn update(&self, _old: &Self, state: &mut State, inner: &mut Self::Inner) {
		inner.min_size = self.min_size;
		inner.max_size = self.max_size;
		inner.handle_events();

		if let Some(on_size_changed) = &self.on_size_changed {
			if inner.size.has_changed().is_ok_and(|t| t) {
				on_size_changed(state, *inner.size.borrow());
			}
		}
	}

	fn spatial_aspect(&self, inner: &Self::Inner) -> SpatialRef {
		inner.content_parent.clone().as_spatial_ref()
	}
}
