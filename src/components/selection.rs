use amethyst::ecs::{storage::NullStorage, Component};

#[derive(Component, Default)]
#[storage(NullStorage)]
pub struct Selected;

#[derive(Component, Default)]
#[storage(NullStorage)]
pub struct Controllable;
