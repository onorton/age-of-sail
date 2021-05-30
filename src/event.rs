use amethyst::ecs::Entity;

#[derive(Debug, PartialEq, Eq)]
pub enum UiUpdateEvent {
    Target(Entity),
    Deselected(Entity),
    PlayerStatus,
}

pub struct CollisionEvent {
    pub entity: Entity,
    pub other_entity: Entity,
}
