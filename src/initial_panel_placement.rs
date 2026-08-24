use glam::{Quat, Vec3, vec3};
use stardust_xr_asteroids::{Context, CreateInnerInfo, CustomElement, ValidState};
use stardust_xr_fusion::{
	Error, Result,
	client::{Client, ClientHandler},
	spatial::{Spatial, SpatialRefOpError, Transform},
	tracked::{Tracked, TrackedExt},
	types::CreateError,
};
use std::{env, f32::consts::PI};
use tracing::info;

fn look_direction(direction: Vec3) -> Quat {
	let pitch = direction.y.asin();
	let yaw = direction.z.atan2(direction.x);
	Quat::from_rotation_y(-yaw - PI / 2.0) * Quat::from_rotation_x(pitch)
}

async fn initial_placement(
	client: &Client<impl ClientHandler>,
	spatial_root: Spatial,
) -> Result<()> {
	let hmd = Tracked::hmd_spatial().await?;
	let root = client.root();

	let Transform {
		translation: item_translation,
		..
	} = spatial_root.get_relative_transform(root.clone()).await??;
	let Transform {
		translation: hmd_translation,
		..
	} = client
		.spatial_interface()
		.get_relative_transform(root.clone(), hmd.clone())
		.await?
		.map_err(|err| match err {
			SpatialRefOpError::RelativeToInvalid => CreateError::InvalidRef,
			SpatialRefOpError::SpatialRefInvalid => CreateError::InvalidRef,
		})?;

	// if the distance between the panel item and the client origin is basically nothing, it must be unpositioned
	if env::var_os("STARDUST_STARTUP_TOKEN").is_none_or(|v| v.is_empty()) {
		info!("launched without a sense of space");
		// so we want to position it in front of the user
		let _ = spatial_root.set_relative_transform(
			hmd,
			Transform::from_translation_rotation(vec3(0.0, 0.0, -0.25), Quat::IDENTITY),
		);
		return Ok(());
	}

	// otherwise make the panel look at the user
	let look_rotation =
		look_direction((Vec3::from(item_translation) - Vec3::from(hmd_translation)).normalize());
	let _ =
		spatial_root.set_relative_transform(root.clone(), Transform::from_rotation(look_rotation));

	Ok(())
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct InitialPanelPlacement;
impl<State: ValidState> CustomElement<State> for InitialPanelPlacement {
	type Inner = ();
	type Error = Error;

	async fn create_inner(&self, ctx: &Context, info: CreateInnerInfo) -> Result<Self::Inner> {
		initial_placement(&ctx.stardust_client, info.child_space).await
	}

	fn diff(&self, _old_self: &Self, _ctx: &Context, _inner: &mut Self::Inner) {}
}
