use amethyst::ecs::{storage::DenseVecStorage, Component, Entity};
use rand::{seq::SliceRandom, thread_rng};
use std::collections::HashMap;

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct Contract {
    pub payment: u32,
    pub destination: Entity,
    pub goods_required: HashMap<ItemType, u32>,
}

#[derive(Eq, PartialEq, Hash, Ord, PartialOrd, Clone, Copy, Debug)]
pub enum ItemType {
    Rum,
    Sugar,
    Whiskey,
}

impl std::fmt::Display for ItemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl ItemType {
    pub fn choose() -> ItemType {
        let mut rng = thread_rng();
        let choices = [ItemType::Sugar, ItemType::Rum, ItemType::Whiskey];
        *choices.choose(&mut rng).unwrap()
    }
}
