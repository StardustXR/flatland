use color::rgba_linear;
use map_range::MapRange;
use rustc_hash::FxHashMap;
use stardust_xr_fusion::{
	core::values::Transform,
	drawable::{MaterialParameter, Model, ModelPart, ResourceID},
	fields::UnknownField,
	items::{panel::PanelItem, ItemAcceptor},
	node::{NodeError, NodeType},
	spatial::Spatial,
};
use stardust_xr_molecules::input_action::SingleActorAction;

use crate::grab_ball::{GrabBallHead, GrabBallSettings};

const MAX_ACCEPT_DISTANCE: f32 = 0.05;
pub struct PanelShellTransfer {
	panel_item: PanelItem,
	model: Model,
	outside: ModelPart,
}
impl PanelShellTransfer {
	pub fn create(connect_root: &Spatial, panel_item: PanelItem) -> Result<Self, NodeError> {
		let model = Model::create(
			&connect_root,
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
		let fields: Vec<_> = acceptors.values().map(|(_, f)| f.alias()).collect();
		let Ok(future) = self.model.field_distance([0.0; 3], fields) else {
			return;
		};
		let panel_item = self.panel_item.alias();
		let item_acceptors: FxHashMap<String, ItemAcceptor<PanelItem>> = acceptors
			.keys()
			.filter_map(|k| Some((k.clone(), acceptors.get(k)?.0.alias())))
			.collect();
		let outside = self.outside.alias();
		let released = grab_action.actor_stopped();
		tokio::spawn(async move {
			let Ok(distances) = future.await else { return };
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
				let _ = outside.set_material_parameter(
					"color",
					MaterialParameter::Color(rgba_linear!(1.0, 1.0, 1.0, 1.0)),
				);
				return;
			};

			let gradient = colorgrad::magma();
			let color = gradient.at(distance.map_range(0.25..MAX_ACCEPT_DISTANCE, 0.0..1.0) as f64);
			let _ = outside.set_material_parameter(
				"emission_factor",
				MaterialParameter::Color(rgba_linear!(
					color.r as f32,
					color.g as f32,
					color.b as f32,
					color.a as f32
				)),
			);
			if released && dbg!(distance) < MAX_ACCEPT_DISTANCE {
				let Some(acceptor) = item_acceptors.get(uid) else {
					return;
				};
				let _ = acceptor.capture(&panel_item);
			}
		});
	}
}
impl GrabBallHead for PanelShellTransfer {
	fn root(&self) -> &Spatial {
		&self.model
	}

	fn set_enabled(&mut self, enabled: bool) {
		let _ = self.model.set_enabled(enabled);
	}

	fn update(&mut self, _grab_action: &SingleActorAction<GrabBallSettings>) {}
}
