use crate::{Flatland, grab_ball::GrabBallSettings};
use derive_setters::Setters;
use glam::{Mat4, Quat, Vec3, Vec3Swizzles, vec2, vec3};
use stardust_xr_asteroids::{
	ClientState, Context, CreateInnerInfo, CustomElement, FnWrapper, ValidState,
};
use stardust_xr_fusion::{
	Error,
	client::{Client, ClientHandler, FrameInfo},
	drawable::{MaterialParameter, Model, ModelExt as _, ModelPart},
	fields::{Field, FieldExt as _, Shape},
	spatial::{PartialTransform, Spatial, SpatialExt as _, SpatialRef, Transform},
	suis::InputDataType,
	tracked::{Tracked, TrackedExt},
	types::{Color, Posef, Resource, ResourceLoadError, Vec2F, rgba_linear},
};
use stardust_xr_molecules::{
	UIElement,
	input_action::{InputQueue, SingleAction},
	reparentable::Reparentable,
};
use std::{f32::consts::FRAC_PI_2, sync::Arc, time::Duration};
use tokio::{
	sync::{Notify, watch},
	task::AbortHandle,
};

const RESIZE_HANDLE_FLOATING: f32 = 0.025;

async fn pos(
	client: &Client<impl ClientHandler>,
	transform: &SpatialRef,
	relative_to: &SpatialRef,
) -> Vec3 {
	client
		.spatial_interface()
		.get_relative_transform(relative_to.clone(), transform.clone())
		.await
		.unwrap()
		.unwrap()
		.translation
		.into()
}
async fn mat(
	client: &Client<impl ClientHandler>,
	transform: &SpatialRef,
	relative_to: &SpatialRef,
) -> Mat4 {
	let transform = client
		.spatial_interface()
		.get_relative_transform(relative_to.clone(), transform.clone())
		.await
		.unwrap()
		.unwrap();
	Mat4::from_scale_rotation_translation(
		transform.scale.into(),
		transform.rotation.into(),
		transform.translation.into(),
	)
}

pub struct ResizeHandle {
	settings: GrabBallSettings,
	_model: Model,
	sphere: ModelPart,
	model_spatial: Spatial,
	_field: Field,
	_input_spatial: Spatial,
	input_spatial_ref: SpatialRef,
	input: InputQueue,
	grab_action: SingleAction,
	pointer_distance: f32,
	old_interact_point: Vec3,
	pub last_pos: Vec3,
}
impl ResizeHandle {
	pub async fn new(
		client: &Client<impl ClientHandler>,
		initial_parent: &SpatialRef,
		input_reference: &SpatialRef,
		settings: GrabBallSettings,
	) -> stardust_xr_fusion::Result<Self> {
		let (model_spatial, _) = Spatial::new(client, initial_parent, Transform::IDENTITY).await?;
		let model = Model::new(
			client,
			&model_spatial,
			Resource::Namespaced {
				namespace: Flatland::APP_ID.into(),
				path: "resize_handle".into(),
			},
		)
		.await?;
		let sphere = model
			.get_part("sphere")
			.await?
			.ok_or(ResourceLoadError::NotFound)?;
		sphere
			.set_material_parameter(
				"color",
				MaterialParameter::Color {
					value: rgba_linear!(0.75, 0.75, 0.75, 1.0),
				},
			)
			.await?;
		let (input_spatial, input_spatial_ref) =
			Spatial::new(client, input_reference, Transform::IDENTITY).await?;
		let (field, _) =
			Field::new(client, &model_spatial, Shape::Sphere { radius: 0.005 }).await?;
		let input = InputQueue::new(
			client,
			model_spatial.clone(),
			field.clone(),
			input_spatial_ref.clone(),
		)
		.await?;

		Ok(ResizeHandle {
			settings,

			_model: model,
			sphere,
			model_spatial,
			_field: field,
			_input_spatial: input_spatial,
			input_spatial_ref,
			input,
			grab_action: Default::default(),
			pointer_distance: 0.0,
			old_interact_point: Vec3::ZERO,
			last_pos: Vec3::ZERO,
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
			|input| input.distance() < (self.settings.radius + self.settings.padding),
			|input| match &input.input() {
				InputDataType::Hand { data: _ } => input.datamap_f32("pinch_strength") > 0.90,
				InputDataType::Pointer { data: _ } => input.datamap_f32("grab") > 0.90,
				_ => input.datamap_f32("grab") > 0.90,
			},
		);

		// if something just got close
		if !self.grab_action.hovering().added().is_empty()
			&& self.grab_action.hovering().added().len()
				== self.grab_action.hovering().current().len()
		{
			let sphere = self.sphere.clone();
			tokio::spawn(async move {
				sphere
					.set_material_parameter(
						"color",
						MaterialParameter::Color {
							value: rgba_linear!(1.0, 1.0, 1.0, 1.0),
						},
					)
					.await
					.unwrap();
			});
		}

		if self.grab_action.hovering().current().is_empty()
			&& !self.grab_action.hovering().removed().is_empty()
		{
			let sphere = self.sphere.clone();
			tokio::spawn(async move {
				sphere
					.set_material_parameter(
						"color",
						MaterialParameter::Color {
							value: rgba_linear!(0.5, 0.5, 0.5, 1.0),
						},
					)
					.await
					.unwrap();
			});
		}

		if self.grab_action.actor_started() {
			let sphere = self.sphere.clone();
			let value = self.settings.connector_color;
			tokio::spawn(async move {
				sphere
					.set_material_parameter("color", MaterialParameter::Color { value })
					.await
					.unwrap();
			});
		}
		if let Some(grab_point) = self.grab_point() {
			self.last_pos = grab_point;
			self.set_pos(&self.input_spatial_ref, grab_point);
		}
		if self.grab_action.actor_stopped() {
			let sphere = self.sphere.clone();
			tokio::spawn(async move {
				sphere
					.set_material_parameter(
						"color",
						MaterialParameter::Color {
							value: rgba_linear!(0.5, 0.5, 0.5, 1.0),
						},
					)
					.await
					.unwrap();
			});
		}
		true
	}
}
impl ResizeHandle {
	fn grab_point(&mut self) -> Option<Vec3> {
		let grabbing = self.grab_action.actor()?;
		match &grabbing.input() {
			InputDataType::Pointer { data: p } => {
				if self.grab_action.actor_started() {
					// Set initial pointer distance based on deepest point
					self.pointer_distance = p.deepest_point;
					self.old_interact_point = Vec3::from(p.pose.position)
						+ Vec3::from(p.direction()).normalize() * self.pointer_distance;
				}

				// Adjust pointer_distance based on scroll input
				let scroll_continuous = grabbing.datamap_vec2("scroll_continuous").y;
				let scroll_discrete = grabbing.datamap_vec2("scroll_discrete").y;
				self.pointer_distance += (scroll_continuous * 0.01) + (scroll_discrete * 0.05);

				// Calculate position at current distance along pointer ray
				let origin = Vec3::from(p.pose.position);
				let direction = Vec3::from(p.direction()).normalize();
				Some(origin + (direction * self.pointer_distance))
			}
			InputDataType::Hand { data: h } => Some(
				Vec3::from(h.thumb.tip.pose.position)
					.lerp(Vec3::from(h.index.tip.pose.position), 0.5),
			),
			InputDataType::Tip { data: t } => Some(t.pose.position.into()),
		}
	}
	pub fn set_pos(&self, relative_to: &SpatialRef, pos: Vec3) {
		let _ = self
			.model_spatial
			.set_relative_transform(relative_to.clone(), PartialTransform::from_translation(pos));
	}
	fn set_enabled(&mut self, enabled: bool) {
		let _ = self
			.model_spatial
			.set_local_transform(PartialTransform::from_scale([enabled as u8 as f32; 3]));
	}
}

pub struct ResizeHandlesInner {
	client_root: SpatialRef,
	content_parent: Spatial,
	content_parent_ref: SpatialRef,
	bottom: ResizeHandle,
	top: ResizeHandle,
	reparentable: watch::Sender<Option<Reparentable>>,
	reparentable_field: Field,
	parent: SpatialRef,

	hmd_pos: watch::Receiver<Vec3>,
	stage_transform: watch::Receiver<Mat4>,
	frame_tick: Arc<Notify>,
	_hmd_task: AbortHandle,
	_stage_task: AbortHandle,
	is_reparentable: bool,
	change_tx: watch::Sender<(Posef, Vec2F)>,
	change: watch::Receiver<(Posef, Vec2F)>,
	pub min_size: Option<Vec2F>,
	pub max_size: Option<Vec2F>,
}
impl ResizeHandlesInner {
	#[allow(clippy::too_many_arguments)]
	pub async fn new(
		client: &Arc<Client<impl ClientHandler>>,
		parent: SpatialRef,
		reparentable: bool,
		accent_color: Color,
		initial_pose: Posef,
		initial_size: Vec2F,
		min_size: Option<Vec2F>,
		max_size: Option<Vec2F>,
	) -> stardust_xr_fusion::Result<Self> {
		let settings = GrabBallSettings {
			radius: 0.005,
			padding: 0.02,
			connector_thickness: 0.0025,
			connector_color: accent_color,
		};

		let (content_parent, content_parent_ref) =
			Spatial::new(client, &parent, Transform::IDENTITY).await?;
		let bottom =
			ResizeHandle::new(client, &content_parent_ref, &parent, settings.clone()).await?;
		let top = ResizeHandle::new(client, &content_parent_ref, &parent, settings.clone()).await?;

		let (change_tx, change) = watch::channel((initial_pose, initial_size));
		let hmd = Tracked::hmd_spatial(client).await?;
		let stage = Tracked::stage_spatial(client).await?;
		let (hmd_tx, hmd_pos) = watch::channel(Vec3::ZERO);
		let frame_tick = Arc::new(Notify::new());
		let _hmd_task = {
			let client = client.clone();
			let hmd_parent = parent.clone();
			let frame_tick = frame_tick.clone();
			tokio::spawn(async move {
				// driven by the client's actual frame events rather than a fixed-rate
				// timer, since display refresh rate varies; also keeps this from
				// flooding the connection with requests (it used to send them as fast
				// as possible)
				loop {
					frame_tick.notified().await;
					let _ = hmd_tx.send(pos(&client, &hmd, &hmd_parent).await);
				}
			})
			.abort_handle()
		};
		let (stage_tx, stage_transform) = watch::channel(Mat4::IDENTITY);
		let _stage_task = {
			let client = client.clone();
			let parent = parent.clone();
			tokio::spawn(async move {
				// this practically never changes, so poll it infrequently rather than
				// flooding the connection with requests
				let mut ticker = tokio::time::interval(Duration::from_millis(250));
				loop {
					ticker.tick().await;
					let _ = stage_tx.send(mat(&client, &stage, &parent).await);
				}
			})
			.abort_handle()
		};
		let (reparentable_field, _) = Field::new(
			client,
			&content_parent,
			Shape::Box {
				size: [initial_size.x, initial_size.y, 0.01].into(),
			},
		)
		.await?;
		let _ = content_parent.set_local_transform(Transform {
			translation: initial_pose.position,
			rotation: initial_pose.orientation,
			scale: [1.0; 3].into(),
		});
		let stage = Tracked::stage_spatial(client).await?;

		let mut resize_handles = ResizeHandlesInner {
			client_root: stage,
			content_parent,
			content_parent_ref,
			bottom,
			top,
			reparentable: watch::channel(None).0,
			reparentable_field,

			parent,
			hmd_pos,
			frame_tick,
			_hmd_task,
			is_reparentable: reparentable,
			change_tx,
			change,
			min_size,
			max_size,
			stage_transform,
			_stage_task,
		};
		resize_handles.set_handle_positions(initial_size, initial_pose);
		resize_handles.make_reparentable(client);
		Ok(resize_handles)
	}
	pub fn handle_events(&mut self, client: &Arc<Client<impl ClientHandler>>) {
		let root = &self.client_root;
		self.bottom.handle_events();
		self.top.handle_events();
		if (self.top.grab_action.actor_started() && !self.bottom.grab_action.actor_acting())
			|| (self.bottom.grab_action.actor_started() && !self.top.grab_action.actor_acting())
		{
			let _ = self.top.model_spatial.set_parent_in_place(root.clone());
			let _ = self.bottom.model_spatial.set_parent_in_place(root.clone());
			self.reparentable.send_modify(|v| {
				v.take();
			});
		}
		if self.top.grab_action.actor_acting() || self.bottom.grab_action.actor_acting() {
			self.update_content_transform();
		}

		if (self.top.grab_action.actor_stopped() && !self.bottom.grab_action.actor_acting())
			|| (self.bottom.grab_action.actor_stopped() && !self.top.grab_action.actor_acting())
		{
			let _ = self
				.top
				.model_spatial
				.set_parent_in_place(self.content_parent_ref.clone());
			let _ = self
				.bottom
				.model_spatial
				.set_parent_in_place(self.content_parent_ref.clone());
			self.make_reparentable(client);
		}
	}
	fn make_reparentable(&mut self, client: &Arc<Client<impl ClientHandler>>) {
		if self.is_reparentable {
			let content_parent = self.content_parent.clone();
			let parent = self.parent.clone();
			let field = self.reparentable_field.clone();
			let client = client.clone();
			let watch = self.reparentable.clone();
			tokio::spawn(async move {
				let v = Reparentable::new(&client, content_parent, parent, field)
					.await
					.ok();
				_ = watch.send(v);
			});
		} else {
			self.reparentable.send_modify(|v| {
				v.take();
			});
		}
	}
	fn update_content_transform(&mut self) {
		let stage_to_parent = *self.stage_transform.borrow();
		let parent_to_stage = stage_to_parent.inverse();
		let hmd_pos = parent_to_stage.transform_point3(*self.hmd_pos.borrow());
		let mut corner1 = parent_to_stage.transform_point3(self.bottom.last_pos);
		let mut corner2 = parent_to_stage.transform_point3(self.top.last_pos);

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

		let min_size = self.min_size.unwrap_or([0.0; 2].into());
		let max_size = self.max_size.unwrap_or([4096.0; 2].into());
		let (stage_to_parent_scale, stage_to_parent_rot, _) =
			stage_to_parent.to_scale_rotation_translation();
		let mut size = vec2(
			// for some reason this doesn't work with transforming the corners back into
			// parent space?
			((corner1.x - corner2.x) * stage_to_parent_scale.x).abs()
				- (RESIZE_HANDLE_FLOATING * 2.0),
			stage_to_parent
				.transform_point3(corner1_2d.extend(0.0))
				.zy()
				.distance(
					stage_to_parent
						.transform_point3(corner2_2d.extend(0.0))
						.zy(),
				) - (RESIZE_HANDLE_FLOATING * 2.0),
		);
		size.x = size.x.max(min_size.x).min(max_size.x);
		size.y = size.y.max(min_size.y).min(max_size.y);

		let pose = Posef {
			position: stage_to_parent.transform_point3(center_point).into(),
			orientation: (stage_to_parent_rot * (y_rotation * x_rotation)).into(),
		};
		let _ = self.change_tx.send((pose, size.into()));
	}
	pub fn set_handle_positions(&mut self, panel_size: Vec2F, pose: Posef) {
		let offset = vec3(
			panel_size.x * 0.5 + RESIZE_HANDLE_FLOATING,
			panel_size.y * 0.5 + RESIZE_HANDLE_FLOATING,
			0.0,
		);
		if !self.top.grab_action.actor_acting() && !self.bottom.grab_action.actor_acting() {
			self.top.set_pos(&self.content_parent_ref, offset);
			self.bottom.set_pos(&self.content_parent_ref, -offset);
			let center: Vec3 = pose.position.into();
			let q = Quat::from_xyzw(
				pose.orientation.v.x,
				pose.orientation.v.y,
				pose.orientation.v.z,
				pose.orientation.s,
			);
			self.top.last_pos = center + q * offset;
			self.bottom.last_pos = center + q * (-offset);
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
	pub pose: Posef,
	pub size: Vec2F,
	pub min_size: Option<Vec2F>,
	pub max_size: Option<Vec2F>,
	pub on_change: FnWrapper<dyn Fn(&mut State, Posef, Vec2F) + Send + Sync>,
}
impl<State: ValidState> CustomElement<State> for ResizeHandles<State> {
	type Inner = ResizeHandlesInner;
	type Error = Error;

	async fn create_inner(
		&self,
		ctx: &Context,
		info: CreateInnerInfo,
	) -> Result<Self::Inner, Self::Error> {
		let v = ResizeHandlesInner::new(
			&ctx.stardust_client,
			info.parent_space.clone(),
			self.reparentable,
			ctx.accent_color.color(),
			self.pose,
			self.size,
			self.min_size,
			self.max_size,
		)
		.await?;
		info.child_space.set_parent(v.content_parent_ref.clone())?;

		Ok(v)
	}

	fn diff(&self, old: &Self, ctx: &Context, inner: &mut Self::Inner) {
		inner.min_size = self.min_size;
		inner.max_size = self.max_size;
		inner.bottom.settings.connector_color = ctx.accent_color.color();
		inner.top.settings.connector_color = ctx.accent_color.color();
		if self.pose != old.pose {
			let _ = inner.content_parent.set_local_transform(Transform {
				translation: self.pose.position,
				rotation: self.pose.orientation,
				scale: [1.0; 3].into(),
			});
		}
		inner.set_handle_positions(self.size, self.pose);
		if self.size != old.size {
			_ = inner.reparentable_field.set_shape(Shape::Box {
				size: [self.size.x, self.size.y, 0.01].into(),
			});
		}
	}

	fn frame(&self, ctx: &Context, _info: &FrameInfo, state: &mut State, inner: &mut Self::Inner) {
		inner.frame_tick.notify_one();
		inner.handle_events(&ctx.stardust_client);

		if inner.change.has_changed().is_ok_and(|t| t) {
			let (pose, size) = *inner.change.borrow_and_update();
			(self.on_change.0)(state, pose, size);
			_ = inner.reparentable_field.set_shape(Shape::Box {
				size: [size.x, size.y, 0.01].into(),
			});
		}
	}
}

#[tokio::test]
async fn test_resize_handles() {
	use serde::{Deserialize, Serialize};
	use stardust_xr_asteroids::{
		ClientState, Migrate, Reify, Transformable, client, elements::Lines,
	};
	use stardust_xr_fusion::{spatial::BoundingBox, types::QuatF};
	use stardust_xr_molecules::lines::bounding_box;

	// Simple test state
	#[derive(Debug, Serialize, Deserialize)]
	struct State {
		time: f32,
		#[serde(skip)]
		pose: Posef,
		size: Vec2F,
	}
	impl Default for State {
		fn default() -> Self {
			Self {
				time: 0.0,
				pose: Posef {
					position: [0.0; 3].into(),
					orientation: QuatF {
						v: [0.0; 3].into(),
						s: 1.0,
					},
				},
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
		fn reify(
			&self,
			_context: &Context,
			_tasks: impl stardust_xr_asteroids::Tasker<Self>,
		) -> impl stardust_xr_asteroids::Element<Self> {
			stardust_xr_asteroids::elements::Spatial::default()
				.rot(Quat::from_rotation_y(self.time / 10.0))
				.build()
				.child(
					ResizeHandles::<Self> {
						reparentable: true,
						pose: self.pose,
						size: self.size,
						min_size: None,
						max_size: None,
						on_change: FnWrapper(Box::new(|state, pose, size| {
							state.pose = pose;
							state.size = size;
						})),
					}
					.build()
					.child(
						Lines::new(bounding_box(BoundingBox {
							center: [0.0; 3].into(),
							extents: [self.size.x, self.size.y, 0.0].into(),
						}))
						.build(),
					),
				)
		}
	}

	client::run::<State>(&[&stardust_xr_asteroids::project_local_resources!("data")])
		.await
		.unwrap();
}
impl Drop for ResizeHandlesInner {
	fn drop(&mut self) {
		self._hmd_task.abort();
		self._stage_task.abort();
	}
}
