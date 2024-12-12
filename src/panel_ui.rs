use asteroids::{
	custom::{ElementTrait, FnWrapper},
	ValidState,
};
use derive_setters::Setters;
use stardust_xr_fusion::{
	core::schemas::zbus::Connection,
	fields::Field,
	items::{
		panel::{
			PanelItem, PanelItemAcceptor, PanelItemInitData, PanelItemUi, PanelItemUiAspect,
			PanelItemUiEvent::*,
		},
		ItemUiAspect,
		ItemUiEvent::*,
	},
	node::{NodeError, NodeType},
	spatial::SpatialRef,
};

#[derive_where::derive_where(Debug, PartialEq)]
#[derive(Setters)]
#[setters(into, strip_option)]
#[allow(clippy::type_complexity)]
pub struct PanelUI<State: ValidState> {
	pub on_create_item: FnWrapper<dyn Fn(&mut State, PanelItem, PanelItemInitData) + Send + Sync>,
	pub on_create_acceptor: FnWrapper<dyn Fn(&mut State, PanelItemAcceptor, Field) + Send + Sync>,
	pub on_capture_item: FnWrapper<dyn Fn(&mut State, u64, u64) + Send + Sync>,
	pub on_release_item: FnWrapper<dyn Fn(&mut State, u64, u64) + Send + Sync>,
	pub on_destroy_item: FnWrapper<dyn Fn(&mut State, u64) + Send + Sync>,
	pub on_destroy_acceptor: FnWrapper<dyn Fn(&mut State, u64) + Send + Sync>,
}
impl<State: ValidState> Default for PanelUI<State> {
	fn default() -> Self {
		PanelUI {
			on_create_item: FnWrapper(Box::new(move |_, _, _| {})),
			on_create_acceptor: FnWrapper(Box::new(move |_, _, _| {})),
			on_capture_item: FnWrapper(Box::new(move |_, _, _| {})),
			on_release_item: FnWrapper(Box::new(move |_, _, _| {})),
			on_destroy_item: FnWrapper(Box::new(move |_, _| {})),
			on_destroy_acceptor: FnWrapper(Box::new(move |_, _| {})),
		}
	}
}
impl<State: ValidState> ElementTrait<State> for PanelUI<State> {
	type Inner = (PanelItemUi, SpatialRef);
	type Error = NodeError;

	fn create_inner(
		&self,
		parent_space: &SpatialRef,
		_dbus_conn: &Connection,
	) -> Result<Self::Inner, Self::Error> {
		let panel_item_ui = PanelItemUi::register(&parent_space.client()?)?;
		Ok((panel_item_ui, parent_space.clone()))
	}

	fn update(&self, _old_decl: &Self, state: &mut State, inner: &mut Self::Inner) {
		while let Some(event) = inner.0.recv_panel_item_ui_event() {
			match event {
				CreateItem { item, initial_data } => {
					(self.on_create_item.0)(state, item, initial_data)
				}
				CreateAcceptor {
					acceptor,
					acceptor_field,
				} => (self.on_create_acceptor.0)(state, acceptor, acceptor_field),
			}
		}
		while let Some(event) = inner.0.recv_item_ui_event() {
			match event {
				CaptureItem {
					item_id,
					acceptor_id,
				} => (self.on_capture_item.0)(state, item_id, acceptor_id),
				ReleaseItem {
					item_id,
					acceptor_id,
				} => (self.on_release_item.0)(state, item_id, acceptor_id),
				DestroyItem { id } => (self.on_destroy_item.0)(state, id),
				DestroyAcceptor { id } => (self.on_destroy_acceptor.0)(state, id),
			}
		}
	}

	fn spatial_aspect(&self, inner: &Self::Inner) -> SpatialRef {
		inner.1.clone()
	}
}
