use color::rgba_linear;
use map_range::MapRange;
use rustc_hash::FxHashMap;
use stardust_xr_fusion::{
	core::values::ResourceID,
	drawable::{MaterialParameter, Model, ModelPart, ModelPartAspect},
	fields::{FieldAspect, UnknownField},
	items::{panel::PanelItem, ItemAcceptor},
	node::{NodeError, NodeType},
	spatial::{SpatialAspect, Transform},
};
use stardust_xr_molecules::input_action::SingleActorAction;
use tokio::task::JoinSet;

use crate::grab_ball::{GrabBallHead, GrabBallSettings};

const MAX_ACCEPT_DISTANCE: f32 = 0.05;
pub struct PanelShellTransfer {
	panel_item: PanelItem,
	model: Model,
	outside: ModelPart,
}
impl PanelShellTransfer {
	pub fn create(
		connect_root: &impl SpatialAspect,
		panel_item: PanelItem,
	) -> Result<Self, NodeError> {
		let model = Model::create(
			connect_root,
			Transform::identity(),
			&ResourceID::new_namespaced("flatland", "panel_shell"),
		)?;
		let outside = model.model_part("Outside")?;

		Ok(PanelShellTransfer {
			panel_item,
			model,
			outside,
		})
	}

	pub fn update_distances(
		&self,
		grab_action: &SingleActorAction<GrabBallSettings>,
		acceptors: &FxHashMap<String, (ItemAcceptor<PanelItem>, UnknownField)>,
	) {
		let mut fields: JoinSet<Result<(f32, ItemAcceptor<PanelItem>), NodeError>> = JoinSet::new();
		for (acceptor, field) in acceptors.values() {
			let model = self.model.alias();
			let acceptor = acceptor.alias();
			let field = field.alias();
			fields.spawn(async move {
				let distance = field.distance(&model, [0.0; 3]).await?;
				Ok((distance, acceptor))
			});
		}
		let panel_item = self.panel_item.alias();
		let outside = self.outside.alias();
		let released = grab_action.actor_stopped();
		tokio::spawn(async move {
			let mut closest_distance = f32::INFINITY;
			let mut closest_acceptor = None;
			while let Some(distance_pair) = fields.join_next().await {
				if let Ok(Ok((distance, acceptor))) = distance_pair {
					if distance < closest_distance {
						closest_distance = distance;
						closest_acceptor.replace(acceptor);
					}
				}
			}
			let Some(acceptor) = closest_acceptor else {
				let _ = outside.set_material_parameter(
					"color",
					MaterialParameter::Color(rgba_linear!(1.0, 1.0, 1.0, 1.0)),
				);
				return;
			};

			let gradient = colorgrad::magma();
			let color =
				gradient.at(closest_distance.map_range(0.25..MAX_ACCEPT_DISTANCE, 0.0..1.0) as f64);
			let _ = outside.set_material_parameter(
				"emission_factor",
				MaterialParameter::Color(rgba_linear!(
					color.r as f32,
					color.g as f32,
					color.b as f32,
					color.a as f32
				)),
			);
			if released && dbg!(closest_distance) < MAX_ACCEPT_DISTANCE {
				let _ = acceptor.capture(&panel_item);
			}
		});
	}
}
impl GrabBallHead for PanelShellTransfer {
	fn root(&self) -> &impl SpatialAspect {
		&self.model
	}

	fn set_enabled(&mut self, enabled: bool) {
		let _ = self.model.set_enabled(enabled);
	}

	fn update(&mut self, _grab_action: &SingleActorAction<GrabBallSettings>) {}
}
