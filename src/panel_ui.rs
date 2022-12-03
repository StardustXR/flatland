use crate::{cursor::Cursor, keyboard::Keyboard, mouse::Mouse};
use glam::{Quat, Vec3};
use lazy_static::lazy_static;
use mint::{Vector2, Vector3};
use stardust_xr_molecules::{
	fusion::{
		data::PulseReceiver,
		drawable::Model,
		fields::BoxField,
		input::{
			action::{BaseInputAction, InputAction, InputActionHandler},
			InputData, InputDataType, InputHandler,
		},
		items::panel::{PanelItem, PanelItemCursor, PanelItemHandler, PanelItemInitData},
		node::NodeType,
		resource::NamespacedResource,
		HandlerWrapper,
	},
	Grabbable, SingleActorAction,
};

lazy_static! {
	static ref PANEL_RESOURCE: NamespacedResource =
		NamespacedResource::new("flatland", "panel.glb");
}

pub const PPM: f32 = 1000.0;
pub struct PanelItemUI {
	pub item: PanelItem,
	pub model: Model,
	cursor: Cursor,
	cursor_pos: Vector2<f32>,
	size: Vector2<f32>,
	field: BoxField,
	keyboard: HandlerWrapper<PulseReceiver, Keyboard>,
	pub mouse: HandlerWrapper<PulseReceiver, Mouse>,
	grabbable: Grabbable,
	hover_action: BaseInputAction<()>,
	click_action: SingleActorAction<()>,
	input_handler: HandlerWrapper<InputHandler, InputActionHandler<()>>,
}
impl PanelItemUI {
	pub fn new(init_data: PanelItemInitData, item: PanelItem) -> Self {
		// println!("Panel item created with {:#?}", init_data);
		if init_data.size.x < 200 || init_data.size.y < 200 {
			item.resize(1600, 900).unwrap();
		}
		// let size = glam::vec3(
		// 	init_data.size.x as f32 / PPM,
		// 	init_data.size.y as f32 / PPM,
		// 	0.01,
		// );
		item.set_transform(
			Some(item.client().unwrap().get_hmd()),
			Some(glam::vec3(0.0, 0.0, -0.5).into()),
			Some(Quat::IDENTITY.into()),
			Some(glam::vec3(1.0, 1.0, 1.0).into()),
		)
		.unwrap();
		let field = BoxField::builder()
			.spatial_parent(&item)
			.size(Vector3::from([1.0; 3]))
			.build()
			.unwrap();
		let grabbable = Grabbable::new(item.client().unwrap().get_root(), &field).unwrap();
		grabbable
			.content_parent()
			.set_transform(Some(&item), None, None, None)
			.unwrap();
		item.set_spatial_parent_in_place(grabbable.content_parent())
			.unwrap();
		let keyboard = Keyboard::new(&item, &field, None, Some(item.alias())).unwrap();
		let mouse = Mouse::new(&item, &field, None, Some(item.alias()), None).unwrap();
		let model = Model::builder()
			.spatial_parent(&item)
			.resource(&*PANEL_RESOURCE)
			.build()
			.unwrap();

		item.apply_surface_material(&model, 0).unwrap();

		let cursor = Cursor::new(&item.spatial);
		cursor.update_info(&init_data.cursor, &item);
		// cursor.update_position(Vector2::from([size.x, size.y]), Vector2::from([0.0, 0.0]));

		let hover_action =
			BaseInputAction::new(false, |input_data, _: &()| input_data.distance < 0.05);
		let click_action = SingleActorAction::new(
			true,
			|input_data: &InputData, _| {
				input_data
					.datamap
					.with_data(|data| match &input_data.input {
						InputDataType::Hand(h) => {
							Vec3::from(h.thumb.tip.position)
								.distance(Vec3::from(h.index.tip.position))
								< 0.02
						}
						_ => data.idx("select").as_f32() > 0.90,
					})
			},
			false,
		);

		let input_handler = InputHandler::create(&model, None, None, &field)
			.unwrap()
			.wrap(InputActionHandler::new(()))
			.unwrap();

		let mut ui = PanelItemUI {
			item,
			model,
			cursor,
			cursor_pos: Vector2::from([0.0, 0.0]),
			size: Vector2::from([0.0; 2]),
			field,
			keyboard,
			mouse,
			grabbable,
			hover_action,
			click_action,
			input_handler,
		};
		ui.resize_surf(init_data.size);
		ui
	}

	pub fn step(&mut self) -> f32 {
		self.grabbable.update();
		self.input_handler.lock_wrapped().update_actions([
			self.hover_action.type_erase(),
			self.click_action.type_erase(),
		]);
		self.click_action.update(&mut self.hover_action);

		if self.click_action.actor_started()
			|| self.click_action.actor_changed()
			|| self.click_action.actor_stopped()
		{
			self.item
				.pointer_button(
					input_event_codes::BTN_LEFT!(),
					self.click_action.actor_acting() as u32,
				)
				.unwrap();
		}

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
							(pointer.deepest_point.x + 0.5) * self.size.x as f32,
							(pointer.deepest_point.y - 0.5) * -self.size.y as f32,
						]);
						self.set_pointer_pos(pos);
					}
					InputDataType::Hand(_) => (),
					InputDataType::Tip(tip) => {
						let pos = Vector2::from([
							(tip.origin.x + 0.5) * self.size.x as f32,
							(tip.origin.y - 0.5) * -self.size.y as f32,
						]);
						self.set_pointer_pos(pos);
					}
				}
			}
		}

		self.grabbable.min_distance()
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
		self.item.pointer_motion(pos).unwrap();
		self.cursor.update_position(self.size, pos);
	}

	pub fn resize_surf(&mut self, size: Vector2<u32>) {
		self.size = Vector2::from_slice(&[size.x as f32, size.y as f32]);
		let size = glam::vec3(self.size.x / PPM, self.size.y / PPM, 0.01);
		self.model.set_scale(None, size).unwrap();
		self.field.set_size(size).unwrap();
		self.keyboard
			.node()
			.spatial
			.set_position(None, Vector3::from([-0.01, size.y * -0.5, 0.0]))
			.unwrap();
		self.mouse
			.node()
			.spatial
			.set_position(None, Vector3::from([0.01, size.y * -0.5, 0.0]))
			.unwrap();
	}
}
impl PanelItemHandler for PanelItemUI {
	fn resize(&mut self, size: Vector2<u32>) {
		println!("Got resize of {}, {}", size.x, size.y);
		self.resize_surf(size);
	}

	fn set_cursor(&mut self, info: Option<PanelItemCursor>) {
		// println!("Set cursor with info {:?}", info);

		self.cursor.update_info(&info, &self.item);
	}
}
impl Drop for PanelItemUI {
	fn drop(&mut self) {
		println!("Panel item destroyed");
	}
}
