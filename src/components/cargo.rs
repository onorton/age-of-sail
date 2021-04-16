use amethyst::ecs::{storage::DenseVecStorage, Component};
use std::collections::HashMap;

use super::ItemType;

#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct Cargo {
    pub items: HashMap<ItemType, u32>,
}
