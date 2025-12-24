use derive_setters::Setters;
use stardust_xr_asteroids::{Context, CreateInnerInfo, CustomElement, FnWrapper, ValidState};
use stardust_xr_fusion::{
	items::panel::{ChildInfo, Geometry, PanelItem, PanelItemAspect, PanelItemEvent::*},
	node::NodeError,
	spatial::SpatialRef,
	values::Vector2,
};

#[derive_where::derive_where(Debug, PartialEq)]
#[derive(Setters)]
#[setters(into, strip_option)]
#[allow(clippy::type_complexity)]
pub struct PanelWrapper<State: ValidState> {
	pub panel_item: PanelItem,
	#[setters(skip)]
	pub on_toplevel_parent_changed: FnWrapper<dyn Fn(&mut State, u64) + Send + Sync>,
	#[setters(skip)]
	pub on_toplevel_title_changed: FnWrapper<dyn Fn(&mut State, String) + Send + Sync>,
	#[setters(skip)]
	pub on_toplevel_app_id_changed: FnWrapper<dyn Fn(&mut State, String) + Send + Sync>,
	#[setters(skip)]
	pub on_toplevel_fullscreen_active: FnWrapper<dyn Fn(&mut State, bool) + Send + Sync>,
	#[setters(skip)]
	pub on_toplevel_move_request: FnWrapper<dyn Fn(&mut State) + Send + Sync>,
	#[setters(skip)]
	pub on_toplevel_resize_request:
		FnWrapper<dyn Fn(&mut State, bool, bool, bool, bool) + Send + Sync>,
	#[setters(skip)]
	pub on_toplevel_size_changed: FnWrapper<dyn Fn(&mut State, Vector2<u32>) + Send + Sync>,
	#[setters(skip)]
	pub on_set_cursor: FnWrapper<dyn Fn(&mut State, Geometry) + Send + Sync>,
	#[setters(skip)]
	pub on_hide_cursor: FnWrapper<dyn Fn(&mut State) + Send + Sync>,
	#[setters(skip)]
	pub on_create_child: FnWrapper<dyn Fn(&mut State, u64, ChildInfo) + Send + Sync>,
	#[setters(skip)]
	pub on_reposition_child: FnWrapper<dyn Fn(&mut State, u64, Geometry) + Send + Sync>,
	#[setters(skip)]
	pub on_destroy_child: FnWrapper<dyn Fn(&mut State, u64) + Send + Sync>,
}

impl<State: ValidState> PanelWrapper<State> {
	pub fn new(panel_item: PanelItem) -> Self {
		PanelWrapper {
			panel_item,
			on_toplevel_parent_changed: FnWrapper(Box::new(move |_, _| {})),
			on_toplevel_title_changed: FnWrapper(Box::new(move |_, _| {})),
			on_toplevel_app_id_changed: FnWrapper(Box::new(move |_, _| {})),
			on_toplevel_fullscreen_active: FnWrapper(Box::new(move |_, _| {})),
			on_toplevel_move_request: FnWrapper(Box::new(move |_| {})),
			on_toplevel_resize_request: FnWrapper(Box::new(move |_, _, _, _, _| {})),
			on_toplevel_size_changed: FnWrapper(Box::new(move |_, _| {})),
			on_set_cursor: FnWrapper(Box::new(move |_, _| {})),
			on_hide_cursor: FnWrapper(Box::new(move |_| {})),

			on_create_child: FnWrapper(Box::new(move |_, _, _| {})),
			on_reposition_child: FnWrapper(Box::new(move |_, _, _| {})),
			on_destroy_child: FnWrapper(Box::new(move |_, _| {})),
		}
	}
	pub fn on_toplevel_parent_changed(
		mut self,
		f: impl Fn(&mut State, u64) + Send + Sync + 'static,
	) -> Self {
		self.on_toplevel_parent_changed = FnWrapper(Box::new(f));
		self
	}
	pub fn on_toplevel_title_changed(
		mut self,
		f: impl Fn(&mut State, String) + Send + Sync + 'static,
	) -> Self {
		self.on_toplevel_title_changed = FnWrapper(Box::new(f));
		self
	}
	pub fn on_toplevel_app_id_changed(
		mut self,
		f: impl Fn(&mut State, String) + Send + Sync + 'static,
	) -> Self {
		self.on_toplevel_app_id_changed = FnWrapper(Box::new(f));
		self
	}
	pub fn on_toplevel_fullscreen_active(
		mut self,
		f: impl Fn(&mut State, bool) + Send + Sync + 'static,
	) -> Self {
		self.on_toplevel_fullscreen_active = FnWrapper(Box::new(f));
		self
	}
	pub fn on_toplevel_move_request(
		mut self,
		f: impl Fn(&mut State) + Send + Sync + 'static,
	) -> Self {
		self.on_toplevel_move_request = FnWrapper(Box::new(f));
		self
	}
	pub fn on_toplevel_resize_request(
		mut self,
		f: impl Fn(&mut State, bool, bool, bool, bool) + Send + Sync + 'static,
	) -> Self {
		self.on_toplevel_resize_request = FnWrapper(Box::new(f));
		self
	}
	pub fn on_toplevel_size_changed(
		mut self,
		f: impl Fn(&mut State, Vector2<u32>) + Send + Sync + 'static,
	) -> Self {
		self.on_toplevel_size_changed = FnWrapper(Box::new(f));
		self
	}
	pub fn on_set_cursor(
		mut self,
		f: impl Fn(&mut State, Geometry) + Send + Sync + 'static,
	) -> Self {
		self.on_set_cursor = FnWrapper(Box::new(f));
		self
	}
	pub fn on_hide_cursor(mut self, f: impl Fn(&mut State) + Send + Sync + 'static) -> Self {
		self.on_hide_cursor = FnWrapper(Box::new(f));
		self
	}
	pub fn on_create_child(
		mut self,
		f: impl Fn(&mut State, u64, ChildInfo) + Send + Sync + 'static,
	) -> Self {
		self.on_create_child = FnWrapper(Box::new(f));
		self
	}
	pub fn on_reposition_child(
		mut self,
		f: impl Fn(&mut State, u64, Geometry) + Send + Sync + 'static,
	) -> Self {
		self.on_reposition_child = FnWrapper(Box::new(f));
		self
	}
	pub fn on_destroy_child(mut self, f: impl Fn(&mut State, u64) + Send + Sync + 'static) -> Self {
		self.on_destroy_child = FnWrapper(Box::new(f));
		self
	}
}
impl<State: ValidState> CustomElement<State> for PanelWrapper<State> {
	type Inner = PanelItem;
	type Resource = ();
	type Error = NodeError;

	fn create_inner(
		&self,
		_context: &Context,
		_info: CreateInnerInfo,
		_resource: &mut Self::Resource,
	) -> Result<Self::Inner, Self::Error> {
		Ok(self.panel_item.clone())
	}

	fn diff(&self, _old_self: &Self, _inner: &mut Self::Inner, _resource: &mut Self::Resource) {}

	fn frame(
		&self,
		_context: &Context,
		_info: &stardust_xr_fusion::root::FrameInfo,
		state: &mut State,
		inner: &mut Self::Inner,
	) {
		while let Some(event) = inner.recv_panel_item_event() {
			match event {
				ToplevelParentChanged { parent_id } => {
					(self.on_toplevel_parent_changed.0)(state, parent_id)
				}
				ToplevelTitleChanged { title } => (self.on_toplevel_title_changed.0)(state, title),
				ToplevelAppIdChanged { app_id } => {
					(self.on_toplevel_app_id_changed.0)(state, app_id)
				}
				ToplevelFullscreenActive { active } => {
					(self.on_toplevel_fullscreen_active.0)(state, active)
				}
				ToplevelMoveRequest {} => (self.on_toplevel_move_request.0)(state),
				ToplevelResizeRequest {
					up,
					down,
					left,
					right,
				} => (self.on_toplevel_resize_request.0)(state, up, down, left, right),
				ToplevelSizeChanged { size } => (self.on_toplevel_size_changed.0)(state, size),
				SetCursor { geometry } => (self.on_set_cursor.0)(state, geometry),
				HideCursor {} => (self.on_hide_cursor.0)(state),

				CreateChild { uid, info } => (self.on_create_child.0)(state, uid, info),
				RepositionChild { uid, geometry } => {
					(self.on_reposition_child.0)(state, uid, geometry)
				}
				DestroyChild { uid } => (self.on_destroy_child.0)(state, uid),
			}
		}
	}

	fn spatial_aspect(&self, inner: &Self::Inner) -> SpatialRef {
		inner.clone().as_item().as_spatial().as_spatial_ref()
	}
}
