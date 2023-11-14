use color::rgba_linear;
use glam::Vec2;
use stardust_xr_fusion::{
	client::FrameInfo,
	core::values::Transform,
	drawable::{MaterialParameter, Model, ModelPart, ResourceID},
	fields::BoxField,
	input::{InputDataType::Pointer, InputHandler},
	items::panel::PanelItem,
	node::{NodeError, NodeType},
	HandlerWrapper,
};
use stardust_xr_molecules::{
	input_action::{BaseInputAction, InputAction, InputActionHandler},
	Exposure,
};

use crate::{surface::Surface, toplevel::TOPLEVEL_THICKNESS};

pub struct CloseButton {
	item: PanelItem,
	model: Model,
	shell: ModelPart,
	exposure: Exposure,
	field: BoxField,
	handler: HandlerWrapper<InputHandler, InputActionHandler<()>>,
	distance_action: BaseInputAction<()>,
}
impl CloseButton {
	pub fn new(item: PanelItem, thickness: f32, panel_size: Vec2) -> Result<Self, NodeError> {
		let model = Model::create(
			&item,
			Transform::from_position_scale(
				[panel_size.x, -panel_size.y, 0.0],
				[0.025, 0.025, thickness],
			),
			&ResourceID::new_namespaced("flatland", "close_button"),
		)?;
		let shell = model.model_part("Shell")?;
		let exposure = Exposure {
			exposure: 0.0,
			cooling: 5.0,
			max: 1.0,
		};

		// compensate for the server not being able to handle scaled fields
		let field = BoxField::create(&shell, Transform::none(), [1.5, 1.0, 1.0])?;
		field.set_spatial_parent_in_place(&item)?;
		field.set_scale(None, [1.0; 3])?;
		field.set_size([1.5 * 0.025, 0.025, thickness])?;

		let handler = InputHandler::create(&shell, Transform::none(), &field)?
			.wrap(InputActionHandler::new(()))?;
		let distance_action = BaseInputAction::new(true, |data, _| {
			data.distance < 0.0
				&& match &data.input {
					Pointer(_) => data.datamap.with_data(|d| d.idx("select").as_f32() > 0.5),
					_ => true,
				}
		});

		Ok(CloseButton {
			item,
			model,
			shell,
			exposure,
			field,
			handler,
			distance_action,
		})
	}

	pub fn update(&mut self, frame_info: &FrameInfo) {
		self.handler
			.lock_wrapped()
			.update_actions([self.distance_action.type_erase()]);
		let exposure: f32 = self
			.distance_action
			.currently_acting
			.iter()
			.map(|d| d.distance.abs().powf(1.0 / 2.2))
			.sum();
		self.exposure.update(frame_info.delta as f32);
		self.exposure
			.expose(exposure * 2.0 / TOPLEVEL_THICKNESS, frame_info.delta as f32);
		self.exposure
			.expose_flash(self.distance_action.started_acting.len() as f32 * 0.25);
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
			.set_position(
				Some(surface.root()),
				[surface.physical_size().x, -surface.physical_size().y, 0.0],
			)
			.unwrap();
		self.field
			.set_position(Some(&self.shell), [0.0; 3])
			.unwrap();
	}

	pub fn set_enabled(&mut self, enabled: bool) {
		self.model.set_enabled(enabled).unwrap();
		self.handler.node().set_enabled(enabled).unwrap();
	}
}
