// use crate::toplevel::TOPLEVEL_THICKNESS;
use asteroids::{
	custom::{ElementTrait, FnWrapper, Transformable},
	ValidState,
};
use derive_setters::Setters;
use glam::Quat;
use stardust_xr_fusion::{
	core::{
		schemas::zbus::Connection,
		values::{color::rgba_linear, ResourceID},
	},
	drawable::{MaterialParameter, Model, ModelPart, ModelPartAspect},
	fields::{Field, FieldAspect, Shape},
	input::{InputDataType::Pointer, InputHandler},
	node::{NodeError, NodeType},
	root::FrameInfo,
	spatial::{Spatial, SpatialAspect, SpatialRef, SpatialRefAspect, Transform},
};
use stardust_xr_molecules::{
	input_action::{InputQueue, InputQueueable, SimpleAction},
	Exposure,
};

#[derive_where::derive_where(Debug, PartialEq)]
#[derive(Setters)]
#[setters(into, strip_option)]
pub struct ExposureButton<State: ValidState> {
	pub transform: Transform,
	pub thickness: f32,
	pub on_click: FnWrapper<dyn Fn(&mut State) + Send + Sync>,
}
impl<State: ValidState> ElementTrait<State> for ExposureButton<State> {
	type Inner = ExposureButtonInner;
	type Resource = ();
	type Error = NodeError;

	fn create_inner(
		&self,
		spatial_parent: &SpatialRef,
		_dbus_conn: &Connection,
		_resource: &mut Self::Resource,
	) -> Result<Self::Inner, Self::Error> {
		ExposureButtonInner::new(spatial_parent, self.transform, self.thickness)
	}
	fn frame(&self, info: &FrameInfo, _state: &mut State, inner: &mut Self::Inner) {
		inner.frame(info);
	}
	fn update(
		&self,
		old: &Self,
		state: &mut State,
		inner: &mut Self::Inner,
		_resource: &mut Self::Resource,
	) {
		self.apply_transform(old, &inner.root);
		if inner.exposure.exposure > 1.0 {
			(self.on_click.0)(state);
		}
		if self.thickness != old.thickness {
			let _ = inner
				.field
				.set_shape(Shape::Box([1.5 * 0.025, 0.025, self.thickness].into()));
			let _ = inner.model.set_local_transform(Transform::from_scale([
				0.025,
				0.025,
				self.thickness,
			]));
		}
	}

	fn spatial_aspect(&self, inner: &Self::Inner) -> SpatialRef {
		inner.model.clone().as_spatial().as_spatial_ref()
	}
}
impl<State: ValidState> Transformable for ExposureButton<State> {
	fn transform(&self) -> &Transform {
		&self.transform
	}
	fn transform_mut(&mut self) -> &mut Transform {
		&mut self.transform
	}
}

pub struct ExposureButtonInner {
	root: Spatial,
	model: Model,
	shell: ModelPart,
	exposure: Exposure,
	field: Field,
	input: InputQueue,
	distance_action: SimpleAction,
}
impl ExposureButtonInner {
	pub fn new(
		parent: &impl SpatialRefAspect,
		transform: Transform,
		thickness: f32,
	) -> Result<Self, NodeError> {
		let root = Spatial::create(parent, transform, false)?;
		let model = Model::create(
			&root,
			Transform::from_scale([0.025, 0.025, thickness]),
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
			&root,
			Transform::none(),
			Shape::Box([1.5 * 0.025, 0.025, thickness].into()),
		)?;
		field.set_relative_transform(
			&shell,
			Transform::from_translation_rotation([0.0; 3], Quat::IDENTITY),
		)?;

		let input = InputHandler::create(&shell, Transform::none(), &field)?.queue()?;

		Ok(ExposureButtonInner {
			root,
			model,
			shell,
			exposure,
			field,
			input,
			distance_action: SimpleAction::default(),
		})
	}

	pub fn frame(&mut self, frame_info: &FrameInfo) -> bool {
		self.input.handle_events();
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
		self.exposure.expose(exposure * 2.0, frame_info.delta);
		self.exposure
			.expose_flash(self.distance_action.currently_acting().len() as f32 * 0.25);
		if self.exposure.exposure > 1.0 {
			true
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
			false
		} else {
			false
		}
	}

	pub fn set_enabled(&mut self, enabled: bool) {
		self.model.set_enabled(enabled).unwrap();
		self.input.handler().set_enabled(enabled).unwrap();
	}
}
