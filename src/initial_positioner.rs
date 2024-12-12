use asteroids::{custom::ElementTrait, ValidState};
use stardust_xr_fusion::{
	core::schemas::zbus::Connection,
	node::NodeError,
	spatial::{Spatial, SpatialAspect, SpatialRef, Transform},
};

#[derive(Debug, PartialEq)]
pub struct InitialPositioner(pub SpatialRef);
impl<State: ValidState> ElementTrait<State> for InitialPositioner {
	type Inner = Spatial;
	type Error = NodeError;

	fn create_inner(
		&self,
		parent_space: &SpatialRef,
		_dbus_conn: &Connection,
	) -> Result<Self::Inner, Self::Error> {
		let spatial = Spatial::create(parent_space, Transform::identity(), false)?;
		spatial.set_relative_transform(&self.0, Transform::identity())?;
		Ok(spatial)
	}

	fn update(&self, _old_decl: &Self, _state: &mut State, _inner: &mut Self::Inner) {}

	fn spatial_aspect(&self, inner: &Self::Inner) -> SpatialRef {
		inner.clone().as_spatial_ref()
	}
}
