use crate::{surface::Surface, toplevel::TOPLEVEL_THICKNESS};
use stardust_xr_fusion::{
	core::values::{color::rgba_linear, ResourceID},
	drawable::{MaterialParameter, Model, ModelPart, ModelPartAspect},
	fields::{Field, Shape},
	input::{InputDataType::Pointer, InputHandler},
	items::panel::{PanelItem, PanelItemAspect},
	node::{NodeError, NodeType},
	root::FrameInfo,
	spatial::{SpatialAspect, Transform},
};
use stardust_xr_molecules::{
	input_action::{InputQueue, InputQueueable, SimpleAction},
	Exposure,
};

pub struct CloseButton {
	item: PanelItem,
	model: Model,
	shell: ModelPart,
	exposure: Exposure,
	field: Field,
	input: InputQueue,
	distance_action: SimpleAction,
}
impl CloseButton {
	pub fn new(item: PanelItem, thickness: f32, surface: &Surface) -> Result<Self, NodeError> {
		let model = Model::create(
			&item,
			Transform::from_translation_scale(
				[surface.physical_size().x, -surface.physical_size().y, 0.0],
				[0.025, 0.025, thickness],
			),
			&ResourceID::new_namespaced("flatland", "close_button"),
		)?;
		let shell = model.part("Shell")?;
		let exposure = Exposure {
			exposure: 0.0,
			cooling: 5.0,
			max: 1.0,
		};

		// compensate for the server not being able to handle scaled fields
		let field = Field::create(
			&shell,
			Transform::none(),
			Shape::Box([1.5 * 0.025, 0.025, thickness].into()),
		)?;
		field.set_spatial_parent_in_place(&item)?;
		field.set_local_transform(Transform::from_scale([1.0; 3]))?;

		let input = InputHandler::create(&shell, Transform::none(), &field)?.queue()?;

		Ok(CloseButton {
			item,
			model,
			shell,
			exposure,
			field,
			input,
			distance_action: SimpleAction::default(),
		})
	}

	pub fn update(&mut self, frame_info: &FrameInfo) {
		self.distance_action.update(&self.input, &|data| {
			data.distance < 0.0
				&& match &data.input {
					Pointer(_) => data.datamap.with_data(|d| d.idx("select").as_f32() > 0.5),
					_ => true,
				}
		});
		let exposure: f32 = self
			.distance_action
			.currently_acting()
			.iter()
			.map(|d| d.distance.abs().powf(1.0 / 2.2))
			.sum();
		self.exposure.update(frame_info.delta);
		self.exposure
			.expose(exposure * 2.0 / TOPLEVEL_THICKNESS, frame_info.delta);
		self.exposure
			.expose_flash(self.distance_action.currently_acting().len() as f32 * 0.25);
		if self.exposure.exposure > 1.0 {
			let _ = self.item.close_toplevel();
		} else if self.exposure.exposure > 0.0 {
			let color = colorgrad::magma().at(self.exposure.exposure.into());
			let _ = self.shell.set_material_parameter(
				"emission_factor",
				MaterialParameter::Color(rgba_linear!(
					color.r as f32,
					color.g as f32,
					color.b as f32,
					color.a as f32
				)),
			);
		}
	}

	pub fn resize(&mut self, surface: &Surface) {
		self.model
			.set_relative_transform(
				surface.root(),
				Transform::from_translation([
					surface.physical_size().x,
					-surface.physical_size().y,
					0.0,
				]),
			)
			.unwrap();
		self.field
			.set_relative_transform(&self.shell, Transform::from_translation([0.0; 3]))
			.unwrap();
	}

	pub fn set_enabled(&mut self, enabled: bool) {
		self.model.set_enabled(enabled).unwrap();
		self.input.handler().set_enabled(enabled).unwrap();
	}
}
