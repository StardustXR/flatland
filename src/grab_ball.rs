use color::{color_space::LinearRgb, rgba_linear, Rgba};
use glam::Vec3;
use mint::Vector3;
use stardust_xr_fusion::{
	core::values::Transform,
	drawable::{Line, LinePoint, Lines},
	fields::SphereField,
	input::{InputDataType, InputHandler},
	node::{NodeError, NodeType},
	spatial::Spatial,
	HandlerWrapper,
};
use stardust_xr_molecules::input_action::{BaseInputAction, InputActionHandler, SingleActorAction};
pub trait GrabBallHead {
	fn root(&self) -> &Spatial;
	fn set_enabled(&mut self, enabled: bool);
	fn update(&mut self, grab_action: &SingleActorAction<GrabBallSettings>);
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
	_field: SphereField,
	settings: GrabBallSettings,
	input_handler: HandlerWrapper<InputHandler, InputActionHandler<GrabBallSettings>>,
	condition_action: BaseInputAction<GrabBallSettings>,
	grab_action: SingleActorAction<GrabBallSettings>,
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
		head.root().set_position(None, offset)?;
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
		let _field = SphereField::create(head.root(), [0.0; 3], settings.radius)?;
		let input_handler = InputHandler::create(&connect_root, Transform::none(), &_field)?
			.wrap(InputActionHandler::new(settings.clone()))?;
		let condition_action = BaseInputAction::new(false, |input, data: &GrabBallSettings| {
			input.distance < data.radius
		});
		let grab_action = SingleActorAction::new(
			true,
			|input, _| {
				input.datamap.with_data(|datamap| match &input.input {
					InputDataType::Hand(_) => datamap.idx("pinch_strength").as_f32() > 0.90,
					_ => datamap.idx("grab").as_f32() > 0.90,
				})
			},
			false,
		);

		Ok(GrabBall {
			connect_root,
			head,
			connector_line,
			connector,
			offset,
			_field,
			settings,
			input_handler,
			condition_action,
			grab_action,
		})
	}

	pub fn update(&mut self) {
		self.input_handler
			.lock_wrapped()
			.update_actions([&mut self.condition_action, self.grab_action.base_mut()]);
		self.grab_action.update(Some(&mut self.condition_action));

		if self.grab_action.actor_stopped() {
			let _ = self
				.head
				.root()
				.set_position(Some(&self.connect_root), self.offset);
			self.connector_line.points[1].point = (self.offset.normalize_or_zero()
				* (self.offset.length() - self.settings.radius))
				.into();
			let _ = self
				.connector
				.update_lines(&vec![self.connector_line.clone()]);
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
			.set_position(Some(&self.connect_root), grab_point);
		let line_end =
			grab_point.normalize_or_zero() * (grab_point.length() - self.settings.radius);
		self.connector_line.points[1].point = line_end.into();
		let _ = self
			.connector
			.update_lines(&vec![self.connector_line.clone()]);
		self.head.update(&self.grab_action);
	}

	pub fn set_enabled(&mut self, enabled: bool) {
		let _ = self.input_handler.node().set_enabled(enabled);
		let _ = self.connector.set_enabled(enabled);
		self.head.set_enabled(enabled);
	}

	pub fn connect_root(&self) -> &Spatial {
		&self.connect_root
	}

	pub fn grab_action(&self) -> &SingleActorAction<GrabBallSettings> {
		&self.grab_action
	}
}
