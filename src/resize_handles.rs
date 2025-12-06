use crate::grab_ball::GrabBallSettings;
use derive_setters::Setters;
use glam::{vec2, vec3, Mat4, Quat, Vec3, Vec3Swizzles};
use stardust_xr_asteroids::{Context, CreateInnerInfo, CustomElement, FnWrapper, ValidState};
use stardust_xr_fusion::{
	drawable::{MaterialParameter, Model, ModelPart, ModelPartAspect},
	fields::{Field, FieldAspect, Shape},
	input::{InputDataType, InputHandler},
	node::{NodeError, NodeResult, NodeType},
	objects::hmd,
	objects::zbus::Connection,
	root::FrameInfo,
	spatial::{Spatial, SpatialAspect, SpatialRef, SpatialRefAspect, Transform},
	values::ResourceID,
	values::{color::rgba_linear, Color, Vector2},
};
use stardust_xr_molecules::{
	dbus::AbortOnDrop,
	input_action::{InputQueue, InputQueueable, SingleAction},
	reparentable::Reparentable,
	UIElement,
};
use std::{
	f32::consts::FRAC_PI_2,
	path::{Path, PathBuf},
};
use tokio::sync::watch;

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

pub struct ResizeHandle {
	settings: GrabBallSettings,
	model: Model,
	sphere: ModelPart,
	_field: Field,
	input: InputQueue,
	grab_action: SingleAction,
	pointer_distance: f32,
	old_interact_point: Vec3,
}
impl ResizeHandle {
	pub fn create(
		initial_parent: &impl SpatialRefAspect,
		settings: GrabBallSettings,
	) -> NodeResult<Self> {
		let model = Model::create(
			initial_parent,
			Transform::identity(),
			&ResourceID::new_namespaced("flatland", "resize_handle"),
		)?;
		let sphere = model.part("sphere")?;
		sphere.set_material_parameter(
			"color",
			MaterialParameter::Color(rgba_linear!(0.75, 0.75, 0.75, 1.0)),
		)?;

		let field = Field::create(&model, Transform::identity(), Shape::Sphere(0.005))?;
		let client = initial_parent.client().clone();
		let root = client.get_root();
		let input = InputHandler::create(root, Transform::identity(), &field)?.queue()?;

		Ok(ResizeHandle {
			settings,

			model,
			sphere,
			_field: field,
			input,
			grab_action: Default::default(),
			pointer_distance: 0.0,
			old_interact_point: Vec3::ZERO,
		})
	}
}
impl UIElement for ResizeHandle {
	fn handle_events(&mut self) -> bool {
		if !self.input.handle_events() {
			return false;
		}
		self.grab_action.update(
			true,
			&self.input,
			|input| match &input.input {
				InputDataType::Pointer(_) => true,
				_ => input.distance < (self.settings.radius + self.settings.padding),
			},
			|input| {
				input.datamap.with_data(|datamap| match &input.input {
					InputDataType::Hand(_) => datamap.idx("pinch_strength").as_f32() > 0.90,
					InputDataType::Pointer(_) => datamap.idx("grab").as_f32() > 0.90,
					_ => datamap.idx("grab").as_f32() > 0.90,
				})
			},
		);

		// if something just got close
		if !self.grab_action.hovering().added().is_empty()
			&& self.grab_action.hovering().added().len()
				== self.grab_action.hovering().current().len()
		{
			let _ = self.sphere.set_material_parameter(
				"color",
				MaterialParameter::Color(rgba_linear!(1.0, 1.0, 1.0, 1.0)),
			);
		}

		if self.grab_action.hovering().current().is_empty()
			&& !self.grab_action.hovering().removed().is_empty()
		{
			let _ = self.sphere.set_material_parameter(
				"color",
				MaterialParameter::Color(rgba_linear!(0.5, 0.5, 0.5, 1.0)),
			);
		}

		if self.grab_action.actor_started() {
			let _ = self.sphere.set_material_parameter(
				"color",
				MaterialParameter::Color(self.settings.connector_color),
			);
		}
		if let Some(grab_point) = self.grab_point() {
			self.set_pos(self.input.handler(), grab_point);
		}
		if self.grab_action.actor_stopped() {
			let _ = self.sphere.set_material_parameter(
				"color",
				MaterialParameter::Color(rgba_linear!(0.5, 0.5, 0.5, 1.0)),
			);
		}
		true
	}
}
impl ResizeHandle {
	fn grab_point(&mut self) -> Option<Vec3> {
		let grabbing = self.grab_action.actor()?;
		match &grabbing.input {
			InputDataType::Pointer(p) => {
				if self.grab_action.actor_started() {
					// Set initial pointer distance based on deepest point
					self.pointer_distance =
						Vec3::from(p.origin).distance(Vec3::from(p.deepest_point));
					self.old_interact_point = Vec3::from(p.origin)
						+ Vec3::from(p.direction()).normalize() * self.pointer_distance;
				}

				// Adjust pointer_distance based on scroll input
				let scroll_continuous = grabbing
					.datamap
					.with_data(|d| d.idx("scroll_continuous").as_vector().idx(1).as_f32());
				let scroll_discrete = grabbing
					.datamap
					.with_data(|d| d.idx("scroll_discrete").as_vector().idx(1).as_f32());
				self.pointer_distance += (scroll_continuous * 0.01) + (scroll_discrete * 0.05);

				// Calculate position at current distance along pointer ray
				let origin = Vec3::from(p.origin);
				let direction = Vec3::from(p.direction()).normalize();
				Some(origin + (direction * self.pointer_distance))
			}
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

pub struct ResizeHandlesInner {
	content_parent: Spatial,
	bottom: ResizeHandle,
	top: ResizeHandle,
	reparentable: Option<Reparentable>,
	reparentable_field: Field,
	_field_update_task: AbortOnDrop,
	path: PathBuf,
	connection: Connection,
	parent: SpatialRef,

	hmd: watch::Receiver<Option<SpatialRef>>,
	is_reparentable: bool,
	size_tx: watch::Sender<Vector2<f32>>,
	size: watch::Receiver<Vector2<f32>>,
	pub min_size: Option<Vector2<f32>>,
	pub max_size: Option<Vector2<f32>>,
}
impl ResizeHandlesInner {
	#[allow(clippy::too_many_arguments)]
	pub fn create(
		parent: SpatialRef,
		connection: Connection,
		path: impl AsRef<Path>,
		zoneable: bool,
		accent_color: Color,
		initial_size: Vector2<f32>,
		min_size: Option<Vector2<f32>>,
		max_size: Option<Vector2<f32>>,
	) -> NodeResult<Self> {
		let settings = GrabBallSettings {
			radius: 0.005,
			padding: 0.02,
			connector_thickness: 0.0025,
			connector_color: accent_color,
		};

		let content_parent = Spatial::create(&parent, Transform::identity())?;
		let bottom = ResizeHandle::create(&content_parent, settings.clone())?;
		let top = ResizeHandle::create(&content_parent, settings.clone())?;

		let (size_tx, size) = watch::channel(initial_size);
		let (hmd_tx, hmd_rx) = watch::channel(None);
		tokio::task::spawn({
			let client = content_parent.client().clone();
			async move {
				if let Some(hmd) = hmd(&client).await {
					let _ = hmd_tx.send(Some(hmd));
				}
			}
		});
		let reparentable_field = Field::create(
			&content_parent,
			Transform::none(),
			Shape::Box([initial_size.x, initial_size.y, 0.01].into()),
		)?;
		let _field_update_task = AbortOnDrop(
			tokio::task::spawn({
				let mut size = size.clone();
				let field = reparentable_field.clone();
				async move {
					while size.changed().await.is_ok() {
						let size = size.borrow();
						_ = field.set_shape(Shape::Box([size.x, size.y, 0.01].into()));
					}
				}
			})
			.abort_handle(),
		);
		let mut resize_handles = ResizeHandlesInner {
			content_parent,
			bottom,
			top,
			parent,
			reparentable: None,
			reparentable_field,
			_field_update_task,
			path: path.as_ref().to_path_buf(),
			connection,

			hmd: hmd_rx,
			is_reparentable: zoneable,
			size_tx,
			size,
			min_size,
			max_size,
		};
		resize_handles.set_handle_positions(initial_size);
		resize_handles.make_reparentable();
		Ok(resize_handles)
	}
	pub fn handle_events(&mut self) {
		let client = self.content_parent.client().clone();
		let root = client.get_root();
		self.bottom.handle_events();
		self.top.handle_events();
		if (self.top.grab_action.actor_started() && !self.bottom.grab_action.actor_acting())
			|| (self.bottom.grab_action.actor_started() && !self.top.grab_action.actor_acting())
		{
			let _ = self.top.model.set_spatial_parent_in_place(root);
			let _ = self.bottom.model.set_spatial_parent_in_place(root);
			let _ = self.reparentable.take();
		}
		if self.top.grab_action.actor_acting() || self.bottom.grab_action.actor_acting() {
			self.update_content_transform();
		}

		if (self.top.grab_action.actor_stopped() && !self.bottom.grab_action.actor_acting())
			|| (self.bottom.grab_action.actor_stopped() && !self.top.grab_action.actor_acting())
		{
			let _ = self
				.top
				.model
				.set_spatial_parent_in_place(&self.content_parent);
			let _ = self
				.bottom
				.model
				.set_spatial_parent_in_place(&self.content_parent);
			self.make_reparentable();
		}
	}
	fn make_reparentable(&mut self) {
		self.reparentable = self
			.is_reparentable
			.then(|| {
				Reparentable::create(
					self.connection.clone(),
					&self.path,
					self.parent.clone(),
					self.content_parent.clone(),
					Some(self.reparentable_field.clone()),
				)
				.ok()
			})
			.flatten();
	}
	fn update_content_transform(&self) {
		let client = self.content_parent.client().clone();
		let content_parent = self.content_parent.clone();
		let corner1 = self.bottom.model.clone();
		let corner2 = self.top.model.clone();

		let size_tx = self.size_tx.clone();
		let min_size = self.min_size.unwrap_or([0.0; 2].into());
		let max_size = self.max_size.unwrap_or([4096.0; 2].into());

		let hmd = self.hmd.clone();

		tokio::task::spawn(async move {
			let Some(hmd) = hmd.borrow().clone() else {
				return;
			};
			let root = client.get_root();
			let (hmd_pos, mut corner1, mut corner2) =
				tokio::join!(pos(&hmd, root), pos(&corner1, root), pos(&corner2, root));
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
			size.x = size.x.max(min_size.x).min(max_size.x);
			size.y = size.y.max(min_size.y).min(max_size.y);

			let _ = content_parent.set_relative_transform(
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
		if !self.top.grab_action.actor_acting() && !self.bottom.grab_action.actor_acting() {
			self.top.set_pos(&self.content_parent, offset);
			self.bottom.set_pos(&self.content_parent, -offset);
		}
	}
	pub fn set_enabled(&mut self, enabled: bool) {
		self.top.set_enabled(enabled);
		self.bottom.set_enabled(enabled);
	}
}

#[derive_where::derive_where(Debug, PartialEq)]
#[derive(Setters)]
#[setters(into, strip_option)]
#[allow(clippy::type_complexity)]
pub struct ResizeHandles<State: ValidState> {
	pub reparentable: bool,
	pub current_size: Vector2<f32>,
	pub min_size: Option<Vector2<f32>>,
	pub max_size: Option<Vector2<f32>>,
	pub on_size_changed: FnWrapper<dyn Fn(&mut State, Vector2<f32>) + Send + Sync>,
}
impl<State: ValidState> CustomElement<State> for ResizeHandles<State> {
	type Inner = ResizeHandlesInner;
	type Resource = ();
	type Error = NodeError;

	fn create_inner(
		&self,
		context: &Context,
		info: CreateInnerInfo,
		_resource: &mut Self::Resource,
	) -> Result<Self::Inner, Self::Error> {
		ResizeHandlesInner::create(
			info.parent_space.clone(),
			context.dbus_connection.clone(),
			info.element_path,
			self.reparentable,
			context.accent_color.color(),
			self.current_size,
			self.min_size,
			self.max_size,
		)
	}

	fn diff(&self, old: &Self, inner: &mut Self::Inner, _resource: &mut Self::Resource) {
		inner.min_size = self.min_size;
		inner.max_size = self.max_size;
		if self.current_size != old.current_size {
			inner.set_handle_positions(self.current_size);
		}
	}

	fn frame(
		&self,
		_context: &Context,
		_info: &FrameInfo,
		state: &mut State,
		inner: &mut Self::Inner,
	) {
		inner.handle_events();

		if inner.size.has_changed().is_ok_and(|t| t) {
			(self.on_size_changed.0)(state, *inner.size.borrow_and_update());
		}
	}

	fn spatial_aspect(&self, inner: &Self::Inner) -> SpatialRef {
		inner.content_parent.clone().as_spatial_ref()
	}
}

#[tokio::test]
async fn test_resize_handles() {
	use serde::{Deserialize, Serialize};
	use stardust_xr_asteroids::{client, ClientState, Migrate, Reify, Transformable};

	// Simple test state
	#[derive(Debug, Serialize, Deserialize)]
	struct State {
		time: f32,
		size: Vector2<f32>,
	}
	impl Default for State {
		fn default() -> Self {
			Self {
				time: 0.0,
				size: [0.3, 0.3].into(),
			}
		}
	}
	impl Migrate for State {
		type Old = Self;
	}
	impl ClientState for State {
		const APP_ID: &'static str = "org.stardustxr.flatland.ResizeHandles";
	}
	impl Reify for State {
		fn reify(&self) -> impl stardust_xr_asteroids::Element<Self> {
			stardust_xr_asteroids::elements::Spatial::default()
				.rot(Quat::from_rotation_y(self.time / 10.0))
				.build()
				.child(
					ResizeHandles::<Self> {
						reparentable: true,
						current_size: self.size,
						min_size: None,
						max_size: None,
						on_size_changed: FnWrapper(Box::new(|state, new_size| {
							state.size = new_size;
						})),
					}
					.build()
					.child(
						stardust_xr_asteroids::elements::Text::new("uwu")
							.character_height(0.05)
							.build(),
					),
				)
		}
	}

	client::run::<State>(&[]).await;
}
