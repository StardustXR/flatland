use glam::Vec3;
use stardust_xr_fusion::{
	core::values::{
		color::{color_space::LinearRgb, rgba_linear, Rgba},
		Vector3,
	},
	drawable::{Line, LinePoint, Lines, LinesAspect},
	fields::{Field, Shape},
	input::{InputDataType, InputHandler},
	node::{NodeError, NodeType},
	spatial::{Spatial, SpatialAspect, Transform},
};
use stardust_xr_molecules::input_action::{InputQueue, InputQueueable, SingleActorAction};

pub trait GrabBallHead {
	fn root(&self) -> &impl SpatialAspect;
	fn set_enabled(&mut self, enabled: bool);
	fn update(&mut self, grab_action: &SingleActorAction);
}

#[derive(Debug, Clone)]
pub struct GrabBallSettings {
	pub radius: f32,
	pub connector_thickness: f32,
	pub connector_color: Rgba<f32, LinearRgb>,
}
impl Default for GrabBallSettings {
	fn default() -> Self {
		Self {
			radius: 0.02,
			connector_thickness: 0.0025,
			connector_color: rgba_linear!(1.0, 1.0, 1.0, 1.0),
		}
	}
}

pub struct GrabBall<H: GrabBallHead> {
	connect_root: Spatial,
	pub head: H,
	connector: Lines,
	connector_line: Line,
	offset: Vec3,
	_field: Field,
	settings: GrabBallSettings,
	input: InputQueue,
	grab_action: SingleActorAction,
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
		let connector_line = Line {
			points: vec![
				LinePoint {
					point: [0.0; 3].into(),
					thickness: settings.connector_thickness,
					color: settings.connector_color,
				},
				LinePoint {
					point: (offset.normalize_or_zero() * (offset.length() - settings.radius))
						.into(),
					thickness: settings.connector_thickness,
					color: settings.connector_color,
				},
			],
			cyclic: false,
		};
		let connector = Lines::create(
			&connect_root,
			Transform::none(),
			&vec![connector_line.clone()],
		)?;
		let _field = Field::create(
			head.root(),
			Transform::identity(),
			Shape::Sphere(settings.radius),
		)?;
		let input_handler =
			InputHandler::create(&connect_root, Transform::none(), &_field)?.queue()?;

		let grab_action = SingleActorAction::default();

		Ok(GrabBall {
			connect_root,
			head,
			connector_line,
			connector,
			offset,
			_field,
			settings,
			input: input_handler,
			grab_action,
		})
	}

	pub fn update(&mut self) {
		self.grab_action.update(
			true,
			&self.input,
			|input| input.distance < self.settings.radius,
			|input| {
				input.datamap.with_data(|datamap| match &input.input {
					InputDataType::Hand(_) => datamap.idx("pinch_strength").as_f32() > 0.90,
					_ => datamap.idx("grab").as_f32() > 0.90,
				})
			},
		);

		if self.grab_action.actor_stopped() {
			let _ = self.head.root().set_relative_transform(
				&self.connect_root,
				Transform::from_translation(self.offset),
			);
			self.connector_line.points[1].point = (self.offset.normalize_or_zero()
				* (self.offset.length() - self.settings.radius))
				.into();
			let _ = self.connector.set_lines(&[self.connector_line.clone()]);
			return;
		}
		let Some(grabbing) = self.grab_action.actor() else {
			return;
		};
		let grab_point = match &grabbing.input {
			InputDataType::Pointer(_) => return,
			InputDataType::Hand(h) => {
				Vec3::from(h.thumb.tip.position).lerp(Vec3::from(h.index.tip.position), 0.5)
			}
			InputDataType::Tip(t) => t.origin.into(),
		};
		let _ = self
			.head
			.root()
			.set_relative_transform(&self.connect_root, Transform::from_translation(grab_point));
		let line_end =
			grab_point.normalize_or_zero() * (grab_point.length() - self.settings.radius);
		self.connector_line.points[1].point = line_end.into();
		let _ = self.connector.set_lines(&vec![self.connector_line.clone()]);
		self.head.update(&self.grab_action);
	}

	pub fn set_enabled(&mut self, enabled: bool) {
		let _ = self.input.handler().set_enabled(enabled);
		let _ = self.connector.set_enabled(enabled);
		self.head.set_enabled(enabled);
	}

	pub fn connect_root(&self) -> &Spatial {
		&self.connect_root
	}

	pub fn grab_action(&self) -> &SingleActorAction {
		&self.grab_action
	}
}
