use ashpd::desktop::settings::Settings;
use asteroids::{
	custom::{ElementTrait, FnWrapper},
	ValidState,
};
use futures_util::StreamExt;
use stardust_xr_fusion::{
	core::schemas::zbus::Connection,
	node::NodeError,
	spatial::SpatialRef,
	values::{color::rgba_linear, Color},
};
use tokio::{sync::watch, task::AbortHandle};

fn accent_color_to_color(accent_color: ashpd::desktop::Color) -> Color {
	rgba_linear!(
		accent_color.red() as f32,
		accent_color.green() as f32,
		accent_color.blue() as f32,
		1.0
	)
}

pub struct AccentColorListenerResource {
	accent_color_loop: AbortHandle,
	accent_color: watch::Receiver<Color>,
}
impl Default for AccentColorListenerResource {
	fn default() -> Self {
		let (accent_color_sender, accent_color) = watch::channel(rgba_linear!(1.0, 1.0, 1.0, 1.0));
		let accent_color_loop = tokio::task::spawn(async move {
			let settings = Settings::new().await?;
			let initial_color = accent_color_to_color(settings.accent_color().await?);
			let _ = accent_color_sender.send(initial_color);
			tracing::info!("Accent color initialized to {:?}", initial_color);
			let mut accent_color_stream = settings.receive_accent_color_changed().await?;
			tracing::info!("Got accent color stream");

			while let Some(accent_color) = accent_color_stream.next().await {
				let accent_color = accent_color_to_color(accent_color);
				tracing::info!("Accent color changed to {:?}", accent_color);
				let _ = accent_color_sender.send(accent_color);
			}

			tracing::error!("why the sigma is this activating");

			Ok::<(), ashpd::Error>(())
		})
		.abort_handle();
		Self {
			accent_color_loop,
			accent_color,
		}
	}
}
impl Drop for AccentColorListenerResource {
	fn drop(&mut self) {
		self.accent_color_loop.abort();
	}
}

pub struct AccentColorInner {
	spatial: SpatialRef,
	color_rx: watch::Receiver<Color>,
}

#[derive_where::derive_where(Debug, PartialEq)]
#[allow(clippy::type_complexity)]
pub struct AccentColorListener<State: ValidState> {
	pub on_accent_color_changed: FnWrapper<dyn Fn(&mut State, Color) + Send + Sync>,
}

impl<State: ValidState> AccentColorListener<State> {
	pub fn new<F: Fn(&mut State, Color) + Send + Sync + 'static>(
		on_accent_color_changed: F,
	) -> Self {
		AccentColorListener {
			on_accent_color_changed: FnWrapper(Box::new(on_accent_color_changed)),
		}
	}
}
impl<State: ValidState> ElementTrait<State> for AccentColorListener<State> {
	type Inner = AccentColorInner;
	type Resource = AccentColorListenerResource;
	type Error = NodeError;

	fn create_inner(
		&self,
		parent_space: &SpatialRef,
		_dbus_conn: &Connection,
		resource: &mut Self::Resource,
	) -> Result<Self::Inner, Self::Error> {
		Ok(AccentColorInner {
			spatial: parent_space.clone(),
			color_rx: resource.accent_color.clone(),
		})
	}

	fn update(
		&self,
		_old_decl: &Self,
		state: &mut State,
		inner: &mut Self::Inner,
		_resource: &mut Self::Resource,
	) {
		if inner.color_rx.has_changed().is_ok_and(|t| t) {
			(self.on_accent_color_changed.0)(state, inner.color_rx.borrow().clone())
		}
	}

	fn spatial_aspect(&self, inner: &Self::Inner) -> SpatialRef {
		inner.spatial.clone()
	}
}
