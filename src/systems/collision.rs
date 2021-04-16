use std::collections::HashSet;

use amethyst::{
    core::Transform,
    ecs::{Entities, Join, ReadStorage, System},
};

use crate::{
    age_of_sail::point_in_rect,
    components::{bounding_box::BoundingBox, Pirate, Ship},
};

pub struct CollisionSystem;

impl<'s> System<'s> for CollisionSystem {
    type SystemData = (
        Entities<'s>,
        ReadStorage<'s, BoundingBox>,
        ReadStorage<'s, Pirate>,
        ReadStorage<'s, Ship>,
        ReadStorage<'s, Transform>,
    );

    fn run(&mut self, (entities, bounding_boxes, pirates, ships, locals): Self::SystemData) {
        let mut ships_to_destroy = HashSet::new();

        // TODO Send collision events and have other systems process them
        for (e, bounding_box, local) in (&entities, &bounding_boxes, &locals).join() {
            for (other_e, other_bounding_box, other_local) in
                (&entities, &bounding_boxes, &locals).join()
            {
                if e == other_e {
                    continue;
                }

                if bounding_boxes_intersect(bounding_box, local, other_bounding_box, other_local) {
                    if ships.get(e).is_some() && pirates.get(other_e).is_some() {
                        ships_to_destroy.insert(e);
                    }

                    if ships.get(other_e).is_some() && pirates.get(e).is_some() {
                        ships_to_destroy.insert(other_e);
                    }
                }
            }
        }

        for ship in ships_to_destroy {
            entities.delete(ship).unwrap();
        }
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
    fn pirate_does_not_destroy_itself() -> Result<()> {
        AmethystApplication::blank()
            .with_system(CollisionSystem, "collision", &[])
            .with_effect(|world| {
                let entity = world
                    .create_entity()
                    .with(Ship { base_speed: 1.0 })
                    .with(Pirate)
                    .with(BoundingBox {
                        width: 5.0,
                        origin: Point2::<f32>::origin(),
                    })
                    .with(Transform::default())
                    .build();

                world.insert(EffectReturn(entity));
            })
            .with_assertion(|world| {
                world.maintain();
                let entity = world.read_resource::<EffectReturn<Entity>>().0.clone();
                assert!(world.entities().is_alive(entity));
            })
            .run()
    }

    #[test]
    fn pirate_does_not_destroy_non_ship() -> Result<()> {
        AmethystApplication::blank()
            .with_system(CollisionSystem, "collision", &[])
            .with_effect(|world| {
                world
                    .create_entity()
                    .with(Pirate)
                    .with(BoundingBox {
                        width: 5.0,
                        origin: Point2::<f32>::origin(),
                    })
                    .with(Transform::default())
                    .build();

                let mut entity_transform = Transform::default();
                entity_transform.set_translation_xyz(2.0, 3.0, 0.0);

                let entity = world
                    .create_entity()
                    .with(BoundingBox {
                        width: 5.0,
                        origin: Point2::<f32>::origin(),
                    })
                    .with(Transform::default())
                    .build();

                world.insert(EffectReturn(entity));
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
            .with_system(CollisionSystem, "collision", &[])
            .with_effect(|world| {
                world
                    .create_entity()
                    .with(Pirate)
                    .with(BoundingBox {
                        width: 5.0,
                        origin: Point2::<f32>::origin(),
                    })
                    .with(Transform::default())
                    .build();

                let mut entity_transform = Transform::default();
                entity_transform.set_translation_xyz(2.0, 3.0, 0.0);

                let entity = world
                    .create_entity()
                    .with(Ship { base_speed: 1.0 })
                    .with(BoundingBox {
                        width: 5.0,
                        origin: Point2::<f32>::origin(),
                    })
                    .with(Transform::default())
                    .build();

                world.insert(EffectReturn(entity));
            })
            .with_assertion(|world| {
                world.maintain();
                let entity = world.read_resource::<EffectReturn<Entity>>().0.clone();
                assert!(!world.entities().is_alive(entity));
            })
            .run()
    }
}
