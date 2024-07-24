use glam::Vec3;
use stardust_xr_fusion::{
	core::values::{
		color::{color_space::LinearRgb, rgba_linear, Rgba},
		Vector3,
	},
	drawable::{Lines, LinesAspect},
	fields::{Field, Shape},
	input::{InputDataType, InputHandler},
	node::{NodeError, NodeType},
	spatial::{Spatial, SpatialAspect, Transform},
};
use stardust_xr_molecules::{
	input_action::{InputQueue, InputQueueable, SingleAction},
	lines::{line_from_points, LineExt},
};

pub trait GrabBallHead {
	fn root(&self) -> &impl SpatialAspect;
	fn set_enabled(&mut self, enabled: bool);
	fn update(&mut self, grab_action: &SingleAction, pos: Vec3);
}

#[derive(Debug, Clone)]
pub struct GrabBallSettings {
	pub radius: f32,
	pub padding: f32,
	pub connector_thickness: f32,
	pub connector_color: Rgba<f32, LinearRgb>,
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
	pub fn create(
		connect_root: Spatial,
		offset: impl Into<Vector3<f32>>,
		head: H,
		settings: GrabBallSettings,
	) -> Result<Self, NodeError> {
		let offset = Vec3::from(offset.into());
		head.root().set_spatial_parent(&connect_root)?;
		head.root()
			.set_local_transform(Transform::from_translation(offset))?;

		let connector = Lines::create(&connect_root, Transform::none(), &[])?;
		let _field = Field::create(
			head.root(),
			Transform::identity(),
			Shape::Sphere(settings.radius),
		)?;
		let input_handler =
			InputHandler::create(&connect_root, Transform::none(), &_field)?.queue()?;

		let grab_action = SingleAction::default();

		Ok(GrabBall {
			connect_root,
			head,
			connector,
			offset,
			_field,
			settings,
			input: input_handler,
			grab_action,
			pos: offset,
		})
	}

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

		if self.grab_action.actor_stopped() {
			self.pos = self.offset;
			let _ = self.head.root().set_relative_transform(
				&self.connect_root,
				Transform::from_translation(self.offset),
			);
		}
		if let Some(grab_point) = self.grab_point() {
			self.pos = grab_point;
			let _ = self
				.head
				.root()
				.set_relative_transform(&self.connect_root, Transform::from_translation(self.pos));
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
				&self.connect_root,
				Transform::from_translation(self.offset),
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

	pub fn update_line(&self) {
		let point = self.grab_point().unwrap_or(self.offset);
		let line_end = point.normalize_or_zero() * (point.length() - self.settings.radius);
		let line = line_from_points(vec![[0.0; 3].into(), line_end])
			.color(self.settings.connector_color)
			.thickness(self.settings.connector_thickness);
		let _ = self.connector.set_lines(&[line]);
	}

	pub fn set_enabled(&mut self, enabled: bool) {
		let _ = self.input.handler().set_enabled(enabled);
		let _ = self.connector.set_enabled(enabled);
		self.head.set_enabled(enabled);
	}

	pub fn connect_root(&self) -> &Spatial {
		&self.connect_root
	}

	pub fn grab_action(&self) -> &SingleAction {
		&self.grab_action
	}
}
