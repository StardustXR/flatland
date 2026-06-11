// use crate::toplevel::TOPLEVEL_THICKNESS;
use derive_setters::Setters;
use glam::{Quat, Vec3};
use stardust_xr_asteroids::{
	ClientState, Context, CreateInnerInfo, CustomElement, FnWrapper, Transformable, ValidState,
};
use stardust_xr_fusion::{
	Error,
	client::{Client, ClientHandler, FrameInfo},
	drawable::{MaterialParameter, Model, ModelExt as _, ModelPart},
	fields::{Field, FieldExt as _, Shape},
	spatial::{PartialTransform, Spatial, SpatialExt as _, Transform},
	suis::InputDataType,
	types::{Resource, ResourceLoadError, rgba_linear},
};
use stardust_xr_molecules::{
	Exposure,
	input_action::{InputQueue, SimpleAction},
};

use crate::ToplevelState;

#[derive_where::derive_where(Debug, PartialEq)]
#[derive(Setters)]
#[setters(into, strip_option)]
pub struct ExposureButton<State: ValidState> {
	pub transform: Transform,
	pub thickness: f32,
	pub gain: f32,
	pub on_click: FnWrapper<dyn Fn(&mut State) + Send + Sync>,
}
impl<State: ValidState> CustomElement<State> for ExposureButton<State> {
	type Inner = ExposureButtonInner;
	type Error = Error;

	async fn create_inner(
		&self,
		ctx: &Context,
		info: CreateInnerInfo,
	) -> Result<Self::Inner, Self::Error> {
		info.child_space.set_local_transform(self.transform.clone())?;
		ExposureButtonInner::new(&ctx.stardust_client, info.child_space, self.thickness).await
	}

	fn diff(&self, old: &Self, _context: &Context, inner: &mut Self::Inner) {
		self.apply_transform(old, &inner.root);

		if self.thickness != old.thickness {
			let _ = inner.field.set_shape(Shape::Box {
				size: [1.5 * 0.025, 0.025, self.thickness].into(),
			});
			let _ = inner
				.model_spatial
				.set_local_transform(PartialTransform::from_scale([0.025, 0.025, self.thickness]));
		}
	}

	fn frame(
		&self,
		_context: &Context,
		info: &FrameInfo,
		state: &mut State,
		inner: &mut Self::Inner,
	) {
		inner.frame(info, self.gain);
		if inner.exposure.exposure > 1.0 {
			(self.on_click.0)(state);
		}
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
	_model: Model,
	model_spatial: Spatial,
	shell: ModelPart,
	exposure: Exposure,
	field: Field,
	input: InputQueue,
	distance_action: SimpleAction,
}
impl ExposureButtonInner {
	pub async fn new(
		client: &Client<impl ClientHandler>,
		root: Spatial,
		thickness: f32,
	) -> Result<Self, Error> {
		let root_ref = root.spatial_ref().await?;
		let (model_spatial, _) = Spatial::create(
			client,
			&root_ref,
			Transform::from_scale([0.025, 0.025, thickness]),
		)
		.await?;
		let model = Model::create(
			&client,
			&model_spatial,
			Resource::Namespaced {
				namespace: ToplevelState::APP_ID.into(),
				path: "close_button".into(),
			},
		)
		.await?;
		let shell = model
			.get_part("Shell")
			.await?
			.ok_or(ResourceLoadError::NotFound)?;
		let shell_spatial = shell.get_spatial().await?;
		let shell_spatial_ref = shell_spatial.spatial_ref().await?;
		let exposure = Exposure {
			exposure: 0.0,
			cooling: 5.0,
			max: 1.0,
		};
		let (field_spatial, _) = Spatial::create(client, &root_ref, Transform::IDENTITY).await?;
		// compensate for the server not being able to handle scaled fields
		let (field, _) = Field::create(
			client,
			&field_spatial,
			Shape::Box {
				size: [1.5 * 0.025, 0.025, thickness].into(),
			},
		)
		.await?;
		field_spatial.set_relative_transform(
			shell_spatial_ref.clone(),
			PartialTransform::from_translation_rotation(Vec3::ZERO, Quat::IDENTITY),
		)?;

		let input = InputQueue::new(
			client,
			shell_spatial.clone(),
			field.clone(),
			shell_spatial_ref,
		)
		.await?;

		Ok(ExposureButtonInner {
			root,
			_model: model,
			model_spatial,
			shell,
			exposure,
			field,
			input,
			distance_action: SimpleAction::default(),
		})
	}

	pub fn frame(&mut self, frame_info: &FrameInfo, gain: f32) -> bool {
		self.input.handle_events();
		self.distance_action.update(&self.input, &|data| {
			data.distance() < 0.0
				&& match &data.input() {
					InputDataType::Pointer { data: _ } => data.datamap_f32("select") > 0.5,
					_ => true,
				}
		});
		let exposure: f32 = self
			.distance_action
			.currently_acting()
			.iter()
			.map(|d| d.distance().abs().powf(1.0 / 2.2))
			.sum();
		let last_exposure = self.exposure.exposure;
		self.exposure.update(frame_info.delta);
		self.exposure.expose(exposure * gain, frame_info.delta);
		self.exposure
			.expose_flash(self.distance_action.currently_acting().len() as f32 * 0.25);
		if self.exposure.exposure > 1.0 {
			true
		} else if self.exposure.exposure > 0.0 || last_exposure > 0.0 {
			let color = colorgrad::magma().at(self.exposure.exposure.into());
			let shell = self.shell.clone();
			tokio::spawn(async move {
				shell
					.set_material_parameter(
						"emission_factor",
						MaterialParameter::Color {
							value: rgba_linear!(
								color.r as f32,
								color.g as f32,
								color.b as f32,
								color.a as f32
							),
						},
					)
					.await
					.unwrap()
			});
			false
		} else {
			false
		}
	}
}
