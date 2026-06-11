use glam::Vec3;
use stardust_xr_fusion::{
	Error,
	client::{Client, ClientHandler},
	drawable::{Lines, LinesExt as _},
	fields::{Field, FieldExt as _, Shape},
	spatial::{Spatial, SpatialRef, Transform},
	suis::InputDataType,
	types::{Color, Vec3F, rgba_linear},
};
use stardust_xr_molecules::{
	input_action::{InputQueue, SingleAction},
	lines::{LineExt, line_from_points},
};

pub trait GrabBallHead {
	fn root(&self) -> &Spatial;
	fn set_enabled(&mut self, enabled: bool);
	fn update(&mut self, grab_action: &SingleAction, pos: Vec3);
}

#[derive(Debug, Clone)]
pub struct GrabBallSettings {
	pub radius: f32,
	pub padding: f32,
	pub connector_thickness: f32,
	pub connector_color: Color,
}
impl Default for GrabBallSettings {
	fn default() -> Self {
		Self {
			radius: 0.02,
			padding: 0.05,
			connector_thickness: 0.0025,
			connector_color: rgba_linear!(1.0, 1.0, 1.0, 1.0),
		}
	}
}

pub struct GrabBall<H: GrabBallHead> {
	connect_root: Spatial,
	connect_root_ref: SpatialRef,
	pub head: H,
	connector: Lines,
	offset: Vec3,
	_field: Field,
	settings: GrabBallSettings,
	input: InputQueue,
	grab_action: SingleAction,
	pos: Vec3,
}
impl<H: GrabBallHead> GrabBall<H> {
	pub async fn create(
		client: &Client<impl ClientHandler>,
		connect_root: Spatial,
		offset: impl Into<Vec3F>,
		head: H,
		settings: GrabBallSettings,
	) -> Result<Self, Error> {
		let connect_root_ref = connect_root.spatial_ref().await?;
		let offset = Vec3::from(offset.into());
		head.root().set_parent(connect_root_ref.clone())?;
		head.root()
			.set_local_transform(Transform::from_translation(offset))?;

		let connector = Lines::create(client, &connect_root, Vec::new()).await?;
		let (field, _) = Field::create(
			client,
			head.root(),
			Shape::Sphere {
				radius: settings.radius,
			},
		)
		.await?;
		let input = InputQueue::new(
			client,
			connect_root.clone(),
			field.clone(),
			connect_root_ref.clone(),
		)
		.await?;

		let grab_action = SingleAction::default();

		Ok(GrabBall {
			connect_root,
			connect_root_ref,
			head,
			connector,
			offset,
			_field: field,
			settings,
			input,
			grab_action,
			pos: offset,
		})
	}

	pub fn update(&mut self) {
		self.grab_action.update(
			true,
			&self.input,
			|input| match &input.input() {
				InputDataType::Pointer { data: _ } => false,
				_ => input.distance() < (self.settings.radius + self.settings.padding),
			},
			|input| match &input.input() {
				InputDataType::Hand { data: _ } => input.datamap_f32("pinch_strength") > 0.90,
				_ => input.datamap_f32("grab") > 0.90,
			},
		);

		if self.grab_action.actor_stopped() {
			self.pos = self.offset;
			let _ = self.head.root().set_relative_transform(
				self.connect_root_ref.clone(),
				Transform::from_translation(self.offset),
			);
		}
		if let Some(grab_point) = self.grab_point() {
			self.pos = grab_point;
			let _ = self.head.root().set_relative_transform(
				self.connect_root_ref.clone(),
				Transform::from_translation(self.pos),
			);
		}
		self.head.update(&self.grab_action, self.pos);
		self.update_line();
	}
	pub fn pos(&self) -> &Vec3 {
		&self.pos
	}
	pub fn set_offset(&mut self, offset: impl Into<Vec3>) {
		self.offset = offset.into();
		if !self.grab_action.actor_acting() {
			self.pos = self.offset;
			let _ = self.head.root().set_relative_transform(
				self.connect_root_ref.clone(),
				Transform::from_translation(self.offset),
			);
		}
	}
	fn grab_point(&self) -> Option<Vec3> {
		let grabbing = self.grab_action.actor()?;
		match &grabbing.input() {
			InputDataType::Pointer { data: _ } => None,
			InputDataType::Hand { data: h } => Some(
				Vec3::from(h.thumb.tip.pose.position)
					.lerp(Vec3::from(h.index.tip.pose.position), 0.5),
			),
			InputDataType::Tip { data: t } => Some(t.pose.position.into()),
		}
	}

	pub fn update_line(&self) {
		let point = self.grab_point().unwrap_or(self.offset);
		let line_end = point.normalize_or_zero() * (point.length() - self.settings.radius);
		let line = line_from_points(vec![[0.0; 3].into(), line_end])
			.color(self.settings.connector_color)
			.thickness(self.settings.connector_thickness);
		let _ = self.connector.set_lines(&[line]);
	}

	pub fn connect_root(&self) -> &Spatial {
		&self.connect_root
	}

	pub fn grab_action(&self) -> &SingleAction {
		&self.grab_action
	}
}
