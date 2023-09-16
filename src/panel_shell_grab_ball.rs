use color::rgba;
use glam::Vec3;
use map_range::MapRange;
use mint::Vector3;
use rustc_hash::FxHashMap;
use stardust_xr_fusion::{
	core::values::Transform,
	drawable::{LinePoint, Lines, MaterialParameter, Model, ModelPart, ResourceID},
	fields::{SphereField, UnknownField},
	input::{
		action::{BaseInputAction, InputAction, InputActionHandler},
		InputDataType, InputHandler,
	},
	items::{panel::PanelItem, ItemAcceptor},
	node::{NodeError, NodeType},
	spatial::Spatial,
	HandlerWrapper,
};
use stardust_xr_molecules::SingleActorAction;

const RADIUS: f32 = 0.01252;
const MAX_ACCEPT_DISTANCE: f32 = 0.05;
pub struct PanelShellGrabBall {
	connect_root: Spatial,
	panel_item: PanelItem,
	model: Model,
	outside: ModelPart,
	connector: Lines,
	connector_points: [LinePoint; 2],
	offset: Vec3,
	_field: SphereField,
	input_handler: HandlerWrapper<InputHandler, InputActionHandler<()>>,
	condition_action: BaseInputAction<()>,
	grab_action: SingleActorAction<()>,
}
impl PanelShellGrabBall {
	pub fn create(
		connect_root: Spatial,
		offset: impl Into<Vector3<f32>>,
		panel_item: PanelItem,
	) -> Result<Self, NodeError> {
		let offset = Vec3::from(offset.into());
		let model = Model::create(
			&connect_root,
			Transform::from_position(offset),
			&ResourceID::new_namespaced("flatland", "panel_shell"),
		)?;
		let outside = model.model_part("Outside")?;
		let connector_points = [
			LinePoint {
				point: [0.0; 3].into(),
				thickness: 0.0025,
				color: rgba!(0.0, 1.0, 0.5, 1.0),
			},
			LinePoint {
				point: (offset.normalize_or_zero() * (offset.length() - RADIUS)).into(),
				thickness: 0.0025,
				color: rgba!(0.0, 1.0, 0.5, 1.0),
			},
		];
		let connector = Lines::create(&connect_root, Transform::none(), &connector_points, false)?;
		let _field = SphereField::create(&model, [0.0; 3], RADIUS)?;
		let input_handler = InputHandler::create(&connect_root, Transform::none(), &_field)?
			.wrap(InputActionHandler::new(()))?;
		let condition_action =
			BaseInputAction::new(false, |input, _| input.distance < RADIUS * 2.0);
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

		Ok(PanelShellGrabBall {
			connect_root,
			panel_item,
			model,
			outside,
			connector_points,
			connector,
			offset,
			_field,
			input_handler,
			condition_action,
			grab_action,
		})
	}

	pub fn update(
		&mut self,
		acceptors: &FxHashMap<String, (ItemAcceptor<PanelItem>, UnknownField)>,
	) {
		self.input_handler.lock_wrapped().update_actions([
			self.condition_action.type_erase(),
			self.grab_action.type_erase(),
		]);
		self.grab_action.update(&mut self.condition_action);

		if self.grab_action.actor_stopped() {
			self.update_distances(acceptors);
			let _ = self
				.model
				.set_position(Some(&self.connect_root), self.offset);
			self.connector_points[1].point =
				(self.offset.normalize_or_zero() * (self.offset.length() - RADIUS)).into();
			let _ = self.connector.update_points(&self.connector_points);
			return;
		}
		let Some(grabbing) = self.grab_action.actor() else {return};
		let grab_point = match &grabbing.input {
			InputDataType::Pointer(_) => return,
			InputDataType::Hand(h) => {
				Vec3::from(h.thumb.tip.position).lerp(Vec3::from(h.index.tip.position), 0.5)
			}
			InputDataType::Tip(t) => t.origin.into(),
		};
		let _ = self
			.model
			.set_position(Some(&self.connect_root), grab_point);
		let line_end = grab_point.normalize_or_zero() * (grab_point.length() - RADIUS);
		self.connector_points[1].point = line_end.into();
		let _ = self.connector.update_points(&self.connector_points);

		self.update_distances(acceptors);
	}

	fn update_distances(
		&mut self,
		acceptors: &FxHashMap<String, (ItemAcceptor<PanelItem>, UnknownField)>,
	) {
		let fields: Vec<_> = acceptors.values().map(|(_, f)| f.alias()).collect();
		let Ok(future) = self.model.field_distance([0.0; 3], fields) else {return};
		let panel_item = self.panel_item.alias();
		let item_acceptors: FxHashMap<String, ItemAcceptor<PanelItem>> = acceptors
			.keys()
			.filter_map(|k| Some((k.clone(), acceptors.get(k)?.0.alias())))
			.collect();
		let outside = self.outside.alias();
		let released = self.grab_action.actor_stopped();
		tokio::spawn(async move {
			let Ok(distances) = future.await else {return};
			// dbg!(&distances);
			let closest_acceptor = item_acceptors
				.keys()
				.zip(distances.into_iter().flatten().map(f32::abs))
				.reduce(
					|(ak, av), (bk, bv)| {
						if av > bv {
							(bk, bv)
						} else {
							(ak, av)
						}
					},
				);
			let Some((uid, distance)) = closest_acceptor else {
				let _ = outside.set_material_parameter("color", MaterialParameter::Color([1.0; 4]));
				return;
			};

			let gradient = colorgrad::magma();
			let color = gradient.at(distance.map_range(0.25..MAX_ACCEPT_DISTANCE, 0.0..1.0) as f64);
			let _ = outside.set_material_parameter(
				"color",
				MaterialParameter::Color(color.to_array().map(|c| c as f32)),
			);
			if released && dbg!(distance) < MAX_ACCEPT_DISTANCE {
				let Some(acceptor) = item_acceptors.get(uid) else {return};
				let _ = acceptor.capture(&panel_item);
			}
		});
	}

	pub fn set_enabled(&mut self, enabled: bool) {
		let _ = self.input_handler.node().set_enabled(enabled);
		let _ = self.model.set_enabled(enabled);
		let _ = self.connector.set_enabled(enabled);
	}

	pub fn connect_root(&self) -> &Spatial {
		&self.connect_root
	}
}
