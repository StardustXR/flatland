use crate::{cursor::Cursor, keyboard::Keyboard, mouse::Mouse};
use glam::{Quat, Vec3};
use lazy_static::lazy_static;
use mint::{Vector2, Vector3};
use stardust_xr_molecules::{
	fusion::{
		core::values::Transform,
		data::PulseReceiver,
		drawable::Model,
		fields::BoxField,
		input::{
			action::{BaseInputAction, InputAction, InputActionHandler},
			InputData, InputDataType, InputHandler,
		},
		items::panel::{
			CursorInfo, PanelItem, PanelItemHandler, PanelItemInitData, RequestedState, State,
			ToplevelInfo,
		},
		node::NodeType,
		resource::NamespacedResource,
		HandlerWrapper,
	},
	GrabData, Grabbable, SingleActorAction,
};
use std::sync::Weak;

lazy_static! {
	static ref PANEL_RESOURCE: NamespacedResource = NamespacedResource::new("flatland", "panel");
}

pub const PPM: f32 = 1000.0;
pub struct PanelItemUI {
	pub item: PanelItem,
	pub model: Model,
	cursor: Cursor,
	size: Vector2<f32>,
	toplevel_info: Option<ToplevelInfo>,
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
		// if init_data.size.x < 200 || init_data.size.y < 200 {
		// 	item.resize(1600, 900).unwrap();
		// }

		item.configure_toplevel(
			None,
			&[
				State::Maximized,
				// State::Fullscreen,
				// State::Resizing,
				State::Activated,
				// State::TiledLeft,
				// State::TiledRight,
				// State::TiledTop,
				// State::TiledBottom,
			],
			None,
		)
		.unwrap();
		item.set_toplevel_capabilities(&[]).unwrap();
		item.set_transform(
			Some(item.client().unwrap().get_hmd()),
			Transform::from_position_rotation_scale([0.0, 0.0, -0.5], Quat::IDENTITY, [1.0; 3]),
		)
		.unwrap();
		let field = BoxField::create(&item, Transform::default(), Vector3::from([1.0; 3])).unwrap();
		let grabbable = Grabbable::new(
			item.client().unwrap().get_root(),
			Transform::default(),
			&field,
			GrabData { max_distance: 0.05 },
		)
		.unwrap();
		grabbable
			.content_parent()
			.set_transform(Some(&item), Transform::default())
			.unwrap();
		item.set_spatial_parent_in_place(grabbable.content_parent())
			.unwrap();
		let keyboard =
			Keyboard::new(&item, Transform::default(), &field, Some(item.alias())).unwrap();
		let mouse = Mouse::new(
			&item,
			Transform::default(),
			&field,
			Some(item.alias()),
			Weak::new(),
		)
		.unwrap();
		let model = Model::create(&item, Transform::default(), &*PANEL_RESOURCE).unwrap();

		let cursor = Cursor::new(&item, &init_data.cursor, &item);
		cursor.update_info(&None, &item);

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

		let input_handler = InputHandler::create(&model, Transform::default(), &field)
			.unwrap()
			.wrap(InputActionHandler::new(()))
			.unwrap();

		let mut ui = PanelItemUI {
			item,
			model,
			cursor,
			size: Vector2::from([0.0; 2]),
			toplevel_info: None,
			field,
			keyboard,
			mouse,
			grabbable,
			hover_action,
			click_action,
			input_handler,
		};
		ui.update_toplevel_info(init_data.toplevel);
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
					self.click_action.actor_acting(),
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
			(self.cursor.pos.x + delta.x).clamp(0.0, self.size.x - 1.0),
			(self.cursor.pos.y + delta.y).clamp(0.0, self.size.y - 1.0),
		]);
		self.set_pointer_pos(pos);
	}

	pub fn set_pointer_pos(&mut self, pos: mint::Vector2<f32>) {
		self.cursor.pos = pos;
		let _ = self.item.pointer_motion(pos);
		self.cursor.update_position(self.size, pos);
	}

	pub fn update_toplevel_info(&mut self, toplevel_info: Option<ToplevelInfo>) {
		dbg!(&toplevel_info);
		if let Some(toplevel_info) = &toplevel_info {
			self.item.apply_toplevel_material(&self.model, 0).unwrap();
			self.size =
				Vector2::from_slice(&[toplevel_info.size.x as f32, toplevel_info.size.y as f32]);
			let size = glam::vec3(self.size.x / PPM, self.size.y / PPM, 0.01);
			self.model.set_scale(None, size).unwrap();
			self.field.set_size(size).unwrap();
			self.keyboard
				.node()
				.set_position(None, Vector3::from([-0.01, size.y * -0.5, 0.0]))
				.unwrap();
			self.mouse
				.node()
				.set_position(None, Vector3::from([0.01, size.y * -0.5, 0.0]))
				.unwrap();
		}
		self.toplevel_info = toplevel_info;
	}
}
impl PanelItemHandler for PanelItemUI {
	fn commit_toplevel(&mut self, state: Option<ToplevelInfo>) {
		self.update_toplevel_info(state);
	}

	fn set_cursor(&mut self, info: Option<CursorInfo>) {
		self.cursor.update_info(&info, &self.item);
	}

	fn recommend_toplevel_state(&mut self, state: RequestedState) {
		dbg!(&state);
		let new_states = match state {
			RequestedState::Maximize(true) => vec![State::Activated, State::Maximized],
			RequestedState::Fullscreen(true) => vec![State::Activated, State::Fullscreen],
			_ => vec![State::Activated],
		};
		self.item
			.configure_toplevel(None, &new_states, None)
			.unwrap();
	}

	fn show_window_menu(&mut self) {}
}
impl Drop for PanelItemUI {
	fn drop(&mut self) {
		println!("Panel item destroyed");
	}
}
