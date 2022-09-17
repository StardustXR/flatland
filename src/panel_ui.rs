use crate::{cursor::Cursor, single_actor_action::SingleActorAction};
use glam::Quat;
use input_event_codes::BTN_LEFT;
use lazy_static::lazy_static;
use mint::Vector2;
use stardust_xr_fusion::{
	drawable::Model,
	fields::BoxField,
	input::{
		action::{BaseInputAction, InputAction, InputActionHandler},
		InputDataType, InputHandler,
	},
	items::panel::{PanelItem, PanelItemCursor, PanelItemHandler, PanelItemInitData},
	node::NodeType,
	resource::Resource,
	HandlerWrapper, WeakNodeRef,
};

lazy_static! {
	static ref PANEL_RESOURCE: Resource = Resource::new("flatland", "panel.glb");
}

pub const PPM: f32 = 1000.0;
pub struct PanelItemUI {
	pub item: WeakNodeRef<PanelItem>,
	pub model: Model,
	cursor: Cursor,
	cursor_pos: Vector2<f32>,
	size: Vector2<f32>,
	field: BoxField,
	hover_action: BaseInputAction<()>,
	click_action: SingleActorAction<()>,
	input_handler: HandlerWrapper<InputHandler, InputActionHandler<()>>,
}
impl PanelItemUI {
	pub fn new(
		init_data: PanelItemInitData,
		weak_item: WeakNodeRef<PanelItem>,
		item: &PanelItem,
	) -> Self {
		println!("Panel item created with {:#?}", init_data);
		item.set_spatial_parent(item.client().unwrap().get_root())
			.unwrap();
		item.set_transform(
			Some(item.client().unwrap().get_hmd()),
			Some(glam::vec3(0.0, 0.0, -0.5).into()),
			Some(Quat::IDENTITY.into()),
			Some(glam::vec3(1.0, 1.0, 1.0).into()),
		)
		.unwrap();
		let size = glam::vec3(
			init_data.size.x as f32 / PPM,
			init_data.size.y as f32 / PPM,
			0.01,
		);
		let model = Model::resource_builder()
			.spatial_parent(&item)
			// .spatial_parent(item.node.client.upgrade().unwrap().get_root())
			.resource(&PANEL_RESOURCE)
			.scale(size)
			.build()
			.unwrap();

		item.apply_surface_material(&model, 0).unwrap();

		let cursor = Cursor::new(&item.spatial);
		cursor.update_info(&init_data.cursor, &item);
		cursor.update_position(Vector2::from([size.x, size.y]), Vector2::from([0.0, 0.0]));

		let field = BoxField::builder()
			.spatial_parent(&item)
			.size(size)
			.build()
			.unwrap();
		let hover_action =
			BaseInputAction::new(false, |input_data, _: &()| input_data.distance < 0.0);
		let click_action = SingleActorAction::new(
			true,
			|input_data, _: &()| {
				input_data
					.datamap
					.with_data(|datamap| datamap.idx("grab").as_bool())
			},
			false,
		);
		let input_handler = InputHandler::create(&model, None, None, &field, |_, _| {
			InputActionHandler::new(())
		})
		.unwrap();

		PanelItemUI {
			item: weak_item,
			model,
			cursor,
			cursor_pos: Vector2::from([0.0, 0.0]),
			size: Vector2::from([init_data.size.x as f32, init_data.size.y as f32]),
			field,
			hover_action,
			click_action,
			input_handler,
		}
	}

	pub fn step(&mut self) -> f32 {
		self.input_handler
			.lock_inner()
			.update_actions([self.hover_action.type_erase()]);
		self.click_action.update(&mut self.hover_action);

		let closest_input = self
			.hover_action
			.actively_acting
			.iter()
			.reduce(|a, b| if a.distance > b.distance { b } else { a })
			.cloned();
		let distance = if let Some(closest_input) = closest_input {
			if closest_input.distance < 0.01 {
				match &closest_input.input {
					InputDataType::Pointer(pointer) => {
						let pos = Vector2::from([
							(pointer.deepest_point().x + 0.5) * self.size.x as f32,
							(pointer.deepest_point().y - 0.5) * -self.size.y as f32,
						]);
						self.set_pointer_pos(pos);
					}
					InputDataType::Hand(_) => (),
				}
			}
			closest_input.distance
		} else {
			f32::MAX
		};

		if self.click_action.actor_started() {
			let _ = self
				.item
				.with_node(|item| item.pointer_button(BTN_LEFT!(), 1));
		}
		if self.click_action.actor_stopped() {
			let _ = self
				.item
				.with_node(|item| item.pointer_button(BTN_LEFT!(), 0));
		}

		distance
	}

	pub fn pointer_delta(&mut self, delta: mint::Vector2<f32>) {
		let pos = Vector2::from([
			(self.cursor_pos.x + delta.x).clamp(0.0, self.size.x - 1.0),
			(self.cursor_pos.y + delta.y).clamp(0.0, self.size.y - 1.0),
		]);
		self.set_pointer_pos(pos);
	}

	pub fn set_pointer_pos(&mut self, pos: mint::Vector2<f32>) {
		self.cursor_pos = pos;
		self.item.with_node(|panel_item| {
			panel_item.pointer_motion(pos).unwrap();
		});
		self.cursor.update_position(self.size, pos);
	}
}
impl PanelItemHandler for PanelItemUI {
	fn resize(&mut self, size: Vector2<u32>) {
		println!("Got resize of {}, {}", size.x, size.y);
		self.size = Vector2::from_slice(&[size.x as f32, size.y as f32]);
		let size = glam::vec3(self.size.x / PPM, self.size.y / PPM, 0.01);
		self.model.set_scale(None, size).unwrap();
		self.field.set_size(size).unwrap();
	}

	fn set_cursor(&mut self, info: Option<PanelItemCursor>) {
		println!("Set cursor with info {:?}", info);

		self.item.with_node(|panel_item| {
			self.cursor.update_info(&info, panel_item);
		});
	}
}
impl Drop for PanelItemUI {
	fn drop(&mut self) {
		println!("Panel item destroyed");
	}
}
