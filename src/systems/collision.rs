use std::collections::HashSet;

use amethyst::{
    core::Transform,
    ecs::{Entities, Join, Read, ReadStorage, System, SystemData, World, Write},
    prelude::SystemDesc,
    shrev::{EventChannel, ReaderId},
};

use crate::{
    age_of_sail::{point_in_rect, Notifications},
    components::{bounding_box::BoundingBox, Pirate, Ship},
    event::CollisionEvent,
};

pub struct CollisionSystem;

impl<'s> System<'s> for CollisionSystem {
    type SystemData = (
        Entities<'s>,
        ReadStorage<'s, BoundingBox>,
        ReadStorage<'s, Transform>,
        Write<'s, EventChannel<CollisionEvent>>,
    );

    fn run(&mut self, (entities, bounding_boxes, locals, mut channel): Self::SystemData) {
        for (e, bounding_box, local) in (&entities, &bounding_boxes, &locals).join() {
            for (other_e, other_bounding_box, other_local) in
                (&entities, &bounding_boxes, &locals).join()
            {
                if e != other_e
                    && bounding_boxes_intersect(
                        bounding_box,
                        local,
                        other_bounding_box,
                        other_local,
                    )
                {
                    channel.single_write(CollisionEvent {
                        entity: e,
                        other_entity: other_e,
                    });
                }
            }
        }
    }
}

pub struct DestroySystem {
    reader_id: ReaderId<CollisionEvent>,
}

impl DestroySystem {
    fn new(reader_id: ReaderId<CollisionEvent>) -> Self {
        DestroySystem { reader_id }
    }
}

impl<'s> System<'s> for DestroySystem {
    type SystemData = (
        Entities<'s>,
        ReadStorage<'s, Ship>,
        ReadStorage<'s, Pirate>,
        Write<'s, Notifications>,
        Read<'s, EventChannel<CollisionEvent>>,
    );

    fn run(&mut self, (entities, ships, pirates, mut notifications, channel): Self::SystemData) {
        let mut entities_to_destroy = HashSet::new();

        for collision in channel.read(&mut self.reader_id) {
            if ships.get(collision.entity).is_some()
                && pirates.get(collision.other_entity).is_some()
            {
                entities_to_destroy.insert(collision.entity);
            }

            if ships.get(collision.other_entity).is_some()
                && pirates.get(collision.entity).is_some()
            {
                entities_to_destroy.insert(collision.other_entity);
            }
        }

        for entity in entities_to_destroy {
            entities.delete(entity).unwrap();
            notifications.push_back("Ship destroyed by pirate".to_string());
        }
    }
}

pub struct DestroySystemDesc;

impl Default for DestroySystemDesc {
    fn default() -> Self {
        DestroySystemDesc {}
    }
}

impl<'a, 'b> SystemDesc<'a, 'b, DestroySystem> for DestroySystemDesc {
    fn build(self, world: &mut World) -> DestroySystem {
        <DestroySystem as System<'_>>::SystemData::setup(world);

        let reader_id = world
            .fetch_mut::<EventChannel<CollisionEvent>>()
            .register_reader();

        DestroySystem::new(reader_id)
    }
}

fn bounding_boxes_intersect(
    bounding_box: &BoundingBox,
    local: &Transform,
    other_bounding_box: &BoundingBox,
    other_local: &Transform,
) -> bool {
    let (top_left, top_right, bottom_right, bottom_left) = bounding_box.as_2d_points(local);
    let (other_left, other_right, other_top, other_bottom) =
        other_bounding_box.as_boundaries(other_local);

    point_in_rect(top_left, other_left, other_right, other_top, other_bottom)
        || point_in_rect(top_right, other_left, other_right, other_top, other_bottom)
        || point_in_rect(
            bottom_right,
            other_left,
            other_right,
            other_top,
            other_bottom,
        )
        || point_in_rect(
            bottom_left,
            other_left,
            other_right,
            other_top,
            other_bottom,
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use amethyst::{core::math::Point2, ecs::Entity, prelude::*, Result};
    use amethyst_test::prelude::*;

    #[test]
    fn collision_events_not_sent_for_entities_colliding_with_themselves() -> Result<()> {
        AmethystApplication::blank()
            .with_system(CollisionSystem, "collision", &[])
            .with_effect(|world| {
                world
                    .create_entity()
                    .with(BoundingBox {
                        width: 5.0,
                        origin: Point2::<f32>::origin(),
                    })
                    .with(Transform::default())
                    .build();

                let reader_id = world
                    .fetch_mut::<EventChannel<CollisionEvent>>()
                    .register_reader();

                world.insert(reader_id);
            })
            .with_assertion(|world| {
                let collision_event_channel = world.fetch_mut::<EventChannel<CollisionEvent>>();
                let mut reader_id = world.fetch_mut::<ReaderId<CollisionEvent>>();
                assert_eq!(0, collision_event_channel.read(&mut reader_id).len());
            })
            .run()
    }

    #[test]
    fn collision_events_not_sent_for_entities_that_do_not_collide() -> Result<()> {
        AmethystApplication::blank()
            .with_system(CollisionSystem, "collision", &[])
            .with_effect(|world| {
                world
                    .create_entity()
                    .with(BoundingBox {
                        width: 5.0,
                        origin: Point2::<f32>::origin(),
                    })
                    .with(Transform::default())
                    .build();

                let mut other_transform = Transform::default();
                other_transform.set_translation_xyz(20.0, 0.0, 0.0);

                world
                    .create_entity()
                    .with(BoundingBox {
                        width: 5.0,
                        origin: Point2::<f32>::origin(),
                    })
                    .with(other_transform)
                    .build();

                let reader_id = world
                    .fetch_mut::<EventChannel<CollisionEvent>>()
                    .register_reader();

                world.insert(reader_id);
            })
            .with_assertion(|world| {
                let collision_event_channel = world.fetch_mut::<EventChannel<CollisionEvent>>();
                let mut reader_id = world.fetch_mut::<ReaderId<CollisionEvent>>();
                assert_eq!(0, collision_event_channel.read(&mut reader_id).len());
            })
            .run()
    }

    #[test]
    fn two_collision_events_sent_for_each_pair_of_collisions() -> Result<()> {
        AmethystApplication::blank()
            .with_system(CollisionSystem, "collision", &[])
            .with_effect(|world| {
                world
                    .create_entity()
                    .with(BoundingBox {
                        width: 5.0,
                        origin: Point2::<f32>::origin(),
                    })
                    .with(Transform::default())
                    .build();

                let mut other_transform = Transform::default();
                other_transform.set_translation_xyz(2.0, 2.0, 0.0);

                world
                    .create_entity()
                    .with(BoundingBox {
                        width: 5.0,
                        origin: Point2::<f32>::origin(),
                    })
                    .with(other_transform)
                    .build();

                let reader_id = world
                    .fetch_mut::<EventChannel<CollisionEvent>>()
                    .register_reader();

                world.insert(reader_id);
            })
            .with_assertion(|world| {
                let collision_event_channel = world.fetch_mut::<EventChannel<CollisionEvent>>();
                let mut reader_id = world.fetch_mut::<ReaderId<CollisionEvent>>();
                assert_eq!(2, collision_event_channel.read(&mut reader_id).len());
            })
            .run()
    }

    #[test]
    fn more_than_two_entities_can_collide_at_a_time() -> Result<()> {
        AmethystApplication::blank()
            .with_system(CollisionSystem, "collision", &[])
            .with_effect(|world| {
                world
                    .create_entity()
                    .with(BoundingBox {
                        width: 5.0,
                        origin: Point2::<f32>::origin(),
                    })
                    .with(Transform::default())
                    .build();

                let mut other_transform = Transform::default();
                other_transform.set_translation_xyz(2.0, 2.0, 0.0);

                world
                    .create_entity()
                    .with(BoundingBox {
                        width: 5.0,
                        origin: Point2::<f32>::origin(),
                    })
                    .with(other_transform)
                    .build();

                let mut third_transform = Transform::default();
                third_transform.set_translation_xyz(3.0, 1.0, 0.0);

                world
                    .create_entity()
                    .with(BoundingBox {
                        width: 5.0,
                        origin: Point2::<f32>::origin(),
                    })
                    .with(third_transform)
                    .build();

                let reader_id = world
                    .fetch_mut::<EventChannel<CollisionEvent>>()
                    .register_reader();

                world.insert(reader_id);
            })
            .with_assertion(|world| {
                let collision_event_channel = world.fetch_mut::<EventChannel<CollisionEvent>>();
                let mut reader_id = world.fetch_mut::<ReaderId<CollisionEvent>>();
                assert_eq!(6, collision_event_channel.read(&mut reader_id).len());
            })
            .run()
    }

    #[test]
    fn pirate_does_not_destroy_non_ship() -> Result<()> {
        AmethystApplication::blank()
            .with_system_desc(DestroySystemDesc, "destroy", &[])
            .with_effect(|world| {
                let pirate = world.create_entity().with(Pirate).build();

                let mut entity_transform = Transform::default();
                entity_transform.set_translation_xyz(2.0, 3.0, 0.0);

                let entity = world.create_entity().build();

                world.insert(EffectReturn(entity));

                let mut channel = world.fetch_mut::<EventChannel<CollisionEvent>>();
                channel.single_write(CollisionEvent {
                    entity: pirate,
                    other_entity: entity,
                });

                channel.single_write(CollisionEvent {
                    entity: entity,
                    other_entity: pirate,
                });
            })
            .with_assertion(|world| {
                world.maintain();
                let entity = world.read_resource::<EffectReturn<Entity>>().0.clone();
                assert!(world.entities().is_alive(entity));
            })
            .run()
    }

    #[test]
    fn pirate_does_destroy_ship() -> Result<()> {
        AmethystApplication::blank()
            .with_system_desc(DestroySystemDesc, "destroy", &[])
            .with_effect(|world| {
                let pirate = world.create_entity().with(Pirate).build();

                let mut entity_transform = Transform::default();
                entity_transform.set_translation_xyz(2.0, 3.0, 0.0);

                let entity = world.create_entity().with(Ship { base_speed: 1.0 }).build();

                world.insert(EffectReturn(entity));

                let mut channel = world.fetch_mut::<EventChannel<CollisionEvent>>();
                channel.single_write(CollisionEvent {
                    entity: pirate,
                    other_entity: entity,
                });

                channel.single_write(CollisionEvent {
                    entity: entity,
                    other_entity: pirate,
                });
            })
            .with_assertion(|world| {
                world.maintain();
                let entity = world.read_resource::<EffectReturn<Entity>>().0.clone();
                assert!(!world.entities().is_alive(entity));
            })
            .run()
    }

    #[test]
    fn notification_sent_when_pirate_destroys_ship() -> Result<()> {
        AmethystApplication::blank()
            .with_system_desc(DestroySystemDesc, "destroy", &[])
            .with_effect(|world| {
                let pirate = world.create_entity().with(Pirate).build();

                let mut entity_transform = Transform::default();
                entity_transform.set_translation_xyz(2.0, 3.0, 0.0);

                let entity = world.create_entity().with(Ship { base_speed: 1.0 }).build();

                world.insert(EffectReturn(entity));

                let mut channel = world.fetch_mut::<EventChannel<CollisionEvent>>();
                channel.single_write(CollisionEvent {
                    entity: pirate,
                    other_entity: entity,
                });

                channel.single_write(CollisionEvent {
                    entity: entity,
                    other_entity: pirate,
                });
            })
            .with_assertion(|world| {
                let notifications = world.read_resource::<Notifications>().clone();
                assert_eq!(1, notifications.len(), "Number of notifications");
                assert_eq!(
                    "Ship destroyed by pirate",
                    notifications.front().unwrap(),
                    "Notification"
                );
            })
            .run()
    }
}
