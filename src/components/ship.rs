use amethyst::ecs::{
    storage::{DenseVecStorage, NullStorage},
    Component,
};

#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct Ship {
    pub base_speed: f32,
}

#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct Affiliation {
    pub name: String,
}

#[derive(Component, Default)]
#[storage(NullStorage)]
pub struct Pirate;
