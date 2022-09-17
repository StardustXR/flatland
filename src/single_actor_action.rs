use stardust_xr_fusion::input::{
	action::{ActiveCondition, BaseInputAction, InputAction, InputActionState},
	InputData,
};
use std::sync::Arc;

pub struct SingleActorAction<S: InputActionState> {
	pub base_action: BaseInputAction<S>,
	pub capture_on_trigger: bool,
	pub change_actor: bool,

	actor_started: bool,
	actor_changed: bool,
	actor_acting: bool,
	actor_stopped: bool,

	actor: Option<Arc<InputData>>,
}
impl<S: InputActionState> SingleActorAction<S> {
	pub fn new(
		capture_on_trigger: bool,
		active_condition: ActiveCondition<S>,
		change_actor: bool,
	) -> Self {
		Self {
			base_action: BaseInputAction::new(false, active_condition),
			capture_on_trigger,
			change_actor,

			actor_started: false,
			actor_changed: false,
			actor_acting: false,
			actor_stopped: false,

			actor: None,
		}
	}
	pub fn update<O: InputActionState>(&mut self, condition_action: &mut impl InputAction<O>) {
		let old_actor = self.actor.clone();

		if let Some(actor) = &self.actor {
			if self.base_action.stopped_acting.contains(actor) {
				self.actor = None;
				self.base_action.capture_on_trigger = false;
			}
		}
		if self.change_actor || self.actor.is_none() {
			let started_acting = self
				.base_action
				.started_acting
				.intersection(&condition_action.base().actively_acting)
				.next();
			if let Some(started_acting) = started_acting {
				self.actor = Some(started_acting.clone());
				self.base_action.capture_on_trigger = self.capture_on_trigger;
			}
		}

		self.actor_started = false;
		self.actor_changed = false;
		self.actor_acting = false;
		self.actor_stopped = false;

		if old_actor.is_none() && self.actor.is_some() {
			self.actor_started = true;
		}
		if old_actor.is_some() && self.actor.is_some() && old_actor != self.actor {
			self.actor_changed = true;
		}
		if self.actor.is_some() {
			self.actor_acting = true;
		}
		if old_actor.is_some() && self.actor.is_none() {
			self.actor_stopped = true;
		}
	}

	pub fn actor_started(&self) -> bool {
		self.actor_started
	}
	pub fn actor_changed(&self) -> bool {
		self.actor_changed
	}
	pub fn actor_acting(&self) -> bool {
		self.actor_acting
	}
	pub fn actor_stopped(&self) -> bool {
		self.actor_stopped
	}
	pub fn actor(&self) -> Option<&Arc<InputData>> {
		self.actor.as_ref()
	}
}
impl<S: InputActionState> InputAction<S> for SingleActorAction<S> {
	fn base(&mut self) -> &mut BaseInputAction<S> {
		&mut self.base_action
	}
}
