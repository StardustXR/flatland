use asteroids::{Context, CreateInnerInfo, CustomElement, ValidState};
use glam::{vec3, Quat, Vec3};
use stardust_xr_fusion::{
	node::{NodeError, NodeResult, NodeType},
	objects::hmd,
	spatial::{Spatial, SpatialAspect, SpatialRef, SpatialRefAspect, Transform},
};
use std::f32::consts::PI;

fn look_direction(direction: Vec3) -> Quat {
	let pitch = direction.y.asin();
	let yaw = direction.z.atan2(direction.x);
	Quat::from_rotation_y(-yaw - PI / 2.0) * Quat::from_rotation_x(pitch)
}

async fn initial_placement(spatial_root: Spatial) -> NodeResult<()> {
	let client = spatial_root.client()?;
	let Some(hmd) = hmd(&client).await else {
		return Err(NodeError::NotAliased);
	};
	let root = client.get_root();

	let (
		Ok(Transform {
			translation: item_translation,
			..
		}),
		Ok(Transform {
			translation: hmd_translation,
			..
		}),
	) = tokio::join!(spatial_root.get_transform(root), hmd.get_transform(root))
	else {
		return Err(NodeError::NotAliased);
	};

	// if the distance between the panel item and the client origin is basically nothing, it must be unpositioned
	if Vec3::from(item_translation.unwrap()).length_squared() < 0.001 {
		println!("launched without a sense of space");
		// so we want to position it in front of the user
		let _ = spatial_root.set_relative_transform(
			&hmd,
			Transform::from_translation_rotation(vec3(0.0, 0.0, -0.25), Quat::IDENTITY),
		);
		return Ok(());
	}

	// otherwise make the panel look at the user
	let look_rotation = look_direction(
		(Vec3::from(item_translation.unwrap()) - Vec3::from(hmd_translation.unwrap())).normalize(),
	);
	let _ = spatial_root.set_relative_transform(root, Transform::from_rotation(look_rotation));

	Ok(())
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct InitialPanelPlacement;
impl<State: ValidState> CustomElement<State> for InitialPanelPlacement {
	type Inner = Spatial;
	type Resource = ();
	type Error = NodeError;

	fn create_inner(
		&self,
		_context: &Context,
		info: CreateInnerInfo,
		_resource: &mut Self::Resource,
	) -> Result<Self::Inner, Self::Error> {
		let spatial = Spatial::create(info.parent_space, Transform::identity(), false)?;
		tokio::task::spawn(initial_placement(spatial.clone()));
		Ok(spatial)
	}

	fn diff(&self, _old_self: &Self, _inner: &mut Self::Inner, _resource: &mut Self::Resource) {}

	fn spatial_aspect(&self, inner: &Self::Inner) -> SpatialRef {
		inner.clone().as_spatial_ref()
	}
}
