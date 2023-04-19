lazy_static! {
	pub static ref PANEL_RESOURCE: ResourceID = ResourceID::new_namespaced("flatland", "panel");
}

pub const PPM: f32 = 1000.0;
pub struct PanelItemUI {
	item: PanelItem,
	model: Model,
	cursor: Cursor,
	mapped: bool,
	size: Vector2<f32>,
	title: Text,
	toplevel_info: Option<ToplevelInfo>,
	field: BoxField,
	keyboard: HandlerWrapper<PulseReceiver, Keyboard>,
	mouse: HandlerWrapper<PulseReceiver, Mouse>,
	grabbable: Grabbable,
	touch_plane: TouchPlane,
}
