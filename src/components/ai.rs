use amethyst::ecs::{storage::DenseVecStorage, Component};
use std::collections::HashMap;

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct Ai {
    pub states: Vec<AiState>,
    pub current_state_index: usize,
    pub previous_state_index: usize,
}

impl Ai {
    pub fn current_state(&self) -> &AiState {
        &self.states[self.current_state_index]
    }

    pub fn previous_state(&self) -> &AiState {
        &self.states[self.previous_state_index]
    }
}

pub struct AiState {
    pub transitions: HashMap<StateQuery, usize>,
    pub action: Action,
}

#[derive(Clone, Hash, PartialEq, Eq)]
pub enum StateQuery {
    TargetNearby(u32),
    TargetNotNearby(u32),
}

#[derive(Clone, PartialEq, Eq)]
pub enum Action {
    Patrol,
    Chase,
}
