use amethyst::ecs::{storage::DenseVecStorage, Component, Entity};

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct OwnedBy {
    pub entity: Entity,
}
