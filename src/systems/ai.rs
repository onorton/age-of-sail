use amethyst::{
    core::{alga::linear::EuclideanSpace, math::Point2, Transform},
    ecs::{Entities, Entity, Join, ReadStorage, System, WriteStorage},
};

use crate::components::{Ai, Ship, StateQuery};

// Only handle transitions
pub struct AiSystem;

impl<'s> System<'s> for AiSystem {
    type SystemData = (
        Entities<'s>,
        ReadStorage<'s, Transform>,
        ReadStorage<'s, Ship>,
        WriteStorage<'s, Ai>,
    );

    fn run(&mut self, (entities, locals, ships, mut ais): Self::SystemData) {
        for (e, ai) in (&entities, &mut ais).join() {
            let current_state = ai.current_state();
            let mut next_state = ai.current_state_index;
            for (query, s) in current_state.transitions.iter() {
                let transition = match query {
                    StateQuery::TargetNearby(d) => target_nearby(e, &entities, &locals, &ships, *d),
                    StateQuery::TargetNotNearby(d) => {
                        !target_nearby(e, &entities, &locals, &ships, *d)
                    }
                };

                if transition {
                    next_state = *s;
                    break;
                }
            }
            ai.previous_state_index = ai.current_state_index;
            ai.current_state_index = next_state;
        }
    }
}

fn target_nearby<'a>(
    e: Entity,
    entities: &Entities<'a>,
    locals: &ReadStorage<'a, Transform>,
    ships: &ReadStorage<'a, Ship>,
    distance: u32,
) -> bool {
    let e_transform = locals.get(e).unwrap();
    let e_location = Point2::new(e_transform.translation().x, e_transform.translation().y);

    for (other_e, local, _) in (entities, locals, ships).join() {
        if other_e != e {
            let other_e_location = Point2::new(local.translation().x, local.translation().y);
            if other_e_location.distance(&e_location) < distance as f32 {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Action, AiState};
    use amethyst::{prelude::*, Result};
    use amethyst_test::prelude::*;
    use std::collections::HashMap;

    #[test]
    fn ai_transitions_to_correct_state_when_target_nearby() -> Result<()> {
        const ORIGINAL_STATE_INDEX: usize = 0;
        const NEXT_STATE_INDEX: usize = 1;

        AmethystApplication::blank()
            .with_system(AiSystem, "ai", &[])
            .with_effect(|world| {
                let mut target_transform = Transform::default();
                target_transform.set_translation_xyz(5.0, 5.0, 0.0);
                world
                    .create_entity()
                    .with(target_transform)
                    .with(Ship { base_speed: 1.0 })
                    .build();

                let mut ai_transform = Transform::default();
                ai_transform.set_translation_xyz(2.0, 0.0, 0.0);
                let ai_entity = world
                    .create_entity()
                    .with(Ai {
                        states: vec![
                            AiState {
                                transitions: [(StateQuery::TargetNearby(10), NEXT_STATE_INDEX)]
                                    .iter()
                                    .cloned()
                                    .collect(),
                                action: Action::Patrol,
                            },
                            AiState {
                                transitions: HashMap::new(),
                                action: Action::Chase,
                            },
                        ],
                        current_state_index: ORIGINAL_STATE_INDEX,
                        previous_state_index: 0,
                    })
                    .with(ai_transform)
                    .build();

                world.insert(EffectReturn(ai_entity));
            })
            .with_assertion(|world| {
                let ai_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();
                let ai_storage = world.read_storage::<Ai>();
                let ai = ai_storage
                    .get(ai_entity)
                    .expect("Entity should have an `Ai` component.");
                assert_eq!(NEXT_STATE_INDEX, ai.current_state_index);
            })
            .run()
    }

    #[test]
    fn ai_transitions_to_correct_state_when_no_target_nearby() -> Result<()> {
        const ORIGINAL_STATE_INDEX: usize = 0;
        const NEXT_STATE_INDEX: usize = 1;

        AmethystApplication::blank()
            .with_system(AiSystem, "ai", &[])
            .with_effect(|world| {
                let mut target_transform = Transform::default();
                target_transform.set_translation_xyz(100.0, 5.0, 0.0);
                world
                    .create_entity()
                    .with(target_transform)
                    .with(Ship { base_speed: 1.0 })
                    .build();

                let mut ai_transform = Transform::default();
                ai_transform.set_translation_xyz(2.0, 0.0, 0.0);
                let ai_entity = world
                    .create_entity()
                    .with(Ai {
                        states: vec![
                            AiState {
                                transitions: [(StateQuery::TargetNotNearby(10), NEXT_STATE_INDEX)]
                                    .iter()
                                    .cloned()
                                    .collect(),
                                action: Action::Chase,
                            },
                            AiState {
                                transitions: HashMap::new(),
                                action: Action::Patrol,
                            },
                        ],
                        current_state_index: ORIGINAL_STATE_INDEX,
                        previous_state_index: 0,
                    })
                    .with(ai_transform)
                    .build();

                world.insert(EffectReturn(ai_entity));
            })
            .with_assertion(|world| {
                let ai_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();
                let ai_storage = world.read_storage::<Ai>();
                let ai = ai_storage
                    .get(ai_entity)
                    .expect("Entity should have an `Ai` component.");
                assert_eq!(NEXT_STATE_INDEX, ai.current_state_index);
            })
            .run()
    }

    #[test]
    fn ai_does_not_transition_if_no_state_queries_are_satisfied() -> Result<()> {
        const ORIGINAL_STATE_INDEX: usize = 0;

        AmethystApplication::blank()
            .with_system(AiSystem, "ai", &[])
            .with_effect(|world| {
                let mut target_transform = Transform::default();
                target_transform.set_translation_xyz(100.0, 5.0, 0.0);
                world
                    .create_entity()
                    .with(target_transform)
                    .with(Ship { base_speed: 1.0 })
                    .build();

                let mut ai_transform = Transform::default();
                ai_transform.set_translation_xyz(2.0, 0.0, 0.0);
                let ai_entity = world
                    .create_entity()
                    .with(Ai {
                        states: vec![
                            AiState {
                                transitions: [(StateQuery::TargetNearby(10), 1)]
                                    .iter()
                                    .cloned()
                                    .collect(),
                                action: Action::Patrol,
                            },
                            AiState {
                                transitions: HashMap::new(),
                                action: Action::Chase,
                            },
                        ],
                        current_state_index: ORIGINAL_STATE_INDEX,
                        previous_state_index: 0,
                    })
                    .with(ai_transform)
                    .build();

                world.insert(EffectReturn(ai_entity));
            })
            .with_assertion(|world| {
                let ai_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();
                let ai_storage = world.read_storage::<Ai>();
                let ai = ai_storage
                    .get(ai_entity)
                    .expect("Entity should have an `Ai` component.");
                assert_eq!(ORIGINAL_STATE_INDEX, ai.current_state_index);
            })
            .run()
    }
}
