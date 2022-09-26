use crate::cursor::Cursor;
use glam::Quat;
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
	resource::NamespacedResource,
	HandlerWrapper, WeakNodeRef,
};
use stardust_xr_molecules::Grabbable;

lazy_static! {
	static ref PANEL_RESOURCE: NamespacedResource =
		NamespacedResource::new("flatland", "panel.glb");
}

pub const PPM: f32 = 1000.0;
pub struct PanelItemUI {
	pub item: WeakNodeRef<PanelItem>,
	pub model: Model,
	cursor: Cursor,
	cursor_pos: Vector2<f32>,
	size: Vector2<f32>,
	field: BoxField,
	grabbable: Grabbable,
	hover_action: BaseInputAction<()>,
	input_handler: HandlerWrapper<InputHandler, InputActionHandler<()>>,
}
impl PanelItemUI {
	pub fn new(
		init_data: PanelItemInitData,
		weak_item: WeakNodeRef<PanelItem>,
		item: &PanelItem,
	) -> Self {
		println!("Panel item created with {:#?}", init_data);
		let size = glam::vec3(
			init_data.size.x as f32 / PPM,
			init_data.size.y as f32 / PPM,
			0.01,
		);
		item.set_transform(
			Some(item.client().unwrap().get_hmd()),
			Some(glam::vec3(0.0, 0.0, -0.5).into()),
			Some(Quat::IDENTITY.into()),
			Some(glam::vec3(1.0, 1.0, 1.0).into()),
		)
		.unwrap();
		let field = BoxField::builder()
			.spatial_parent(&item)
			.size(size)
			.build()
			.unwrap();
		let grabbable = Grabbable::new(item.client().unwrap().get_root(), &field).unwrap();
		grabbable
			.content_parent()
			.set_transform(Some(&item), None, None, None)
			.unwrap();
		item.set_spatial_parent_in_place(grabbable.content_parent())
			.unwrap();
		let model = Model::builder()
			.spatial_parent(&item)
			.resource(&*PANEL_RESOURCE)
			.scale(size)
			.build()
			.unwrap();

		item.apply_surface_material(&model, 0).unwrap();

		let cursor = Cursor::new(&item.spatial);
		cursor.update_info(&init_data.cursor, &item);
		cursor.update_position(Vector2::from([size.x, size.y]), Vector2::from([0.0, 0.0]));

		let hover_action =
			BaseInputAction::new(false, |input_data, _: &()| input_data.distance < 0.0);

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
			grabbable,
			hover_action,
			input_handler,
		}
	}

	pub fn step(&mut self) -> f32 {
		self.grabbable.update();
		self.input_handler
			.lock_inner()
			.update_actions([self.hover_action.type_erase()]);

		let closest_input = self
			.hover_action
			.actively_acting
			.iter()
			.reduce(|a, b| if a.distance > b.distance { b } else { a })
			.cloned();

		// Closest object distance calculation for focus
		if let Some(closest_input) = closest_input {
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
		}
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
		// println!("Set cursor with info {:?}", info);

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
