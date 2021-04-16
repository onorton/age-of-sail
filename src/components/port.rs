use amethyst::ecs::{storage::DenseVecStorage, Component};

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct Port {
    pub name: String,
}
