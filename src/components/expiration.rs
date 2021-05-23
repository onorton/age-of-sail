use amethyst::ecs::{storage::DenseVecStorage, Component};
use chrono::{Date, Utc};

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct Expiration {
    pub expiration_date: Date<Utc>,
    pub expired: bool,
}
