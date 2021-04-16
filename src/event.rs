use amethyst::ecs::Entity;

#[derive(Debug, PartialEq, Eq)]
pub enum UiUpdateEvent {
    Target(Entity),
    PlayerStatus,
}
