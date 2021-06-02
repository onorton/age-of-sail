use std::cmp::Ordering;
use itertools::Itertools;

use amethyst::{
    core::{
        alga::linear::EuclideanSpace,
        math::{Point2, Vector2},
        Time, Transform, Named
    },
    derive::SystemDesc,
    ecs::{Entities, Join, Read, ReadExpect, Write, ReadStorage, System, SystemData, WriteStorage},
    input::{InputHandler, StringBindings, VirtualKeyCode},
    window::ScreenDimensions,
    winit::MouseButton,
    renderer::Camera,
};

use crate::{
    age_of_sail::{Notifications, point_mouse_to_world, DISTANCE_THRESHOLD},
    components::{Action, Ai, Cargo, Controllable, Course, Patrol, Port, Selected, Ship},
};

pub const SNAP_THRESHOLD: f32 = 5.0;

#[derive(SystemDesc)]
pub struct MoveShipsSystem;

impl<'s> System<'s> for MoveShipsSystem {
    type SystemData = (
        ReadStorage<'s, Ship>,
        WriteStorage<'s, Course>,
        WriteStorage<'s, Transform>,
        Read<'s, Time>,
    );

    fn run(&mut self, (ships, mut courses, mut locals, time): Self::SystemData) {
        for (ship, course, local) in (&ships, &mut courses, &mut locals).join() {
            let ship_x = local.translation().x;
            let ship_y = local.translation().y;

            let ship_location = Point2::new(ship_x, ship_y);

            if let Some(next_waypoint_index) = course.next_waypoint_index {
                if ship_location.distance(&course.waypoints[next_waypoint_index])
                    < DISTANCE_THRESHOLD
                {
                    let new_next_waypoint_index = next_waypoint_index + 1;

                    if new_next_waypoint_index == course.waypoints.len() {
                        course.next_waypoint_index.take();
                        continue;
                    } else {
                        course.next_waypoint_index.replace(new_next_waypoint_index);
                    }
                }

                let next_waypoint_index = course.next_waypoint_index.unwrap();
                let next_waypoint = course.waypoints[next_waypoint_index];

                let direction =
                    Vector2::new(next_waypoint.x - ship_x, next_waypoint.y - ship_y).normalize();

                let distance = ship_location.distance(&next_waypoint);
                let closeness_modifier = if distance < DISTANCE_THRESHOLD * time.time_scale() {
                    distance / (time.time_scale() + f32::EPSILON)
                } else {
                    1.0
                };

                local.prepend_translation_x(
                    closeness_modifier * ship.base_speed * direction.x * time.delta_seconds(),
                );
                local.prepend_translation_y(
                    closeness_modifier * ship.base_speed * direction.y * time.delta_seconds(),
                );
            }
        }
    }
}

#[derive(SystemDesc)]
pub struct PatrolSystem;

impl<'s> System<'s> for PatrolSystem {
    type SystemData = (
        Entities<'s>,
        ReadStorage<'s, Ai>,
        ReadStorage<'s, Transform>,
        WriteStorage<'s, Patrol>,
        WriteStorage<'s, Course>,
    );

    fn run(&mut self, (entities, ais, locals, mut patrols, mut courses): Self::SystemData) {
        for (e, ai, patrol, local) in (&entities, &ais, &mut patrols, &locals).join() {
            if ai.current_state().action == Action::Patrol {
                let e_location = Point2::new(local.translation().x, local.translation().y);

                let previous_action_is_patrol = ai.previous_state().action == Action::Patrol;

                let new_next_waypoint_index = if previous_action_is_patrol {
                    (patrol.next_waypoint_index + 1) % patrol.waypoints.len()
                } else {
                    patrol
                        .waypoints
                        .iter()
                        .enumerate()
                        .min_by(|(_, a), (_, b)| {
                            a.distance(&e_location)
                                .partial_cmp(&(b.distance(&e_location)))
                                .unwrap_or(Ordering::Equal)
                        })
                        .map(|(index, _)| index)
                        .unwrap()
                };

                if e_location.distance(&patrol.waypoints[patrol.next_waypoint_index])
                    < DISTANCE_THRESHOLD
                    || courses.get(e).is_none()
                    || !previous_action_is_patrol
                {
                    patrol.next_waypoint_index = new_next_waypoint_index;
                    courses
                        .insert(
                            e,
                            Course {
                                waypoints: vec![patrol.waypoints[new_next_waypoint_index]],
                                next_waypoint_index: Some(0),
                            },
                        )
                        .unwrap();
                }
            }
        }
    }
}

#[derive(SystemDesc)]
pub struct ChaseSystem;

impl<'s> System<'s> for ChaseSystem {
    type SystemData = (
        Entities<'s>,
        ReadStorage<'s, Ai>,
        ReadStorage<'s, Transform>,
        ReadStorage<'s, Ship>,
        WriteStorage<'s, Course>,
    );

    fn run(&mut self, (entities, ais, locals, ships, mut courses): Self::SystemData) {
        for (e, ai, local) in (&entities, &ais, &locals).join() {
            if ai.current_state().action == Action::Chase {
                let e_location = Point2::new(local.translation().x, local.translation().y);

                // Chase closest ship
                let closest_ship = (&entities, &locals, &ships)
                    .join()
                    .filter(|(other_e, _, _)| e != *other_e)
                    .min_by(|(_, a_local, _), (_, b_local, _)| {
                        let a_location =
                            Point2::new(a_local.translation().x, a_local.translation().y);
                        let b_location =
                            Point2::new(b_local.translation().x, b_local.translation().y);

                        a_location
                            .distance(&e_location)
                            .partial_cmp(&(b_location.distance(&e_location)))
                            .unwrap_or(Ordering::Equal)
                    });

                if let Some((_, other_local, _)) = closest_ship {
                    let other_location =
                        Point2::new(other_local.translation().x, other_local.translation().y);
                    courses
                        .insert(
                            e,
                            Course {
                                waypoints: vec![other_location],
                                next_waypoint_index: Some(0),
                            },
                        )
                        .unwrap();
                }
            }
        }
    }
}

pub struct PlotCourseSystem;

impl<'s> System<'s> for PlotCourseSystem {
    type SystemData = (
        Entities<'s>,
        ReadStorage<'s, Camera>,
        ReadStorage<'s, Ship>,
        ReadStorage<'s, Transform>,
        ReadStorage<'s, Selected>,
        ReadStorage<'s, Controllable>,
        WriteStorage<'s, Course>,
        Read<'s, InputHandler<StringBindings>>,
        ReadExpect<'s, ScreenDimensions>,
    );

    fn run(
        &mut self,
        (entities, cameras, ships, locals, selecteds, controllables, mut courses, input, screen_dimensions): Self::SystemData,
    ) {
        for (_, camera_local) in (&cameras, &locals).join() {
            for (e, _, _, _) in (&entities, &ships, &selecteds, &controllables).join() {
                if let Some((mouse_x, mouse_y)) = input.mouse_position() {
                    if input.mouse_button_is_down(MouseButton::Right) {
                        let point_in_world =
                            point_mouse_to_world(mouse_x, mouse_y, &*screen_dimensions, camera_local.translation());

                        // Snap to any entity if close enough
                        let point = &locals
                            .join()
                            .map(|l| Point2::new(l.translation().x, l.translation().y))
                            .filter(|p| point_in_world.distance(p) < SNAP_THRESHOLD)
                            .next()
                            .unwrap_or(point_in_world);

                        if !input.key_is_down(VirtualKeyCode::LShift) {
                            courses
                                .insert(
                                    e,
                                    Course {
                                        waypoints: vec![*point],
                                        next_waypoint_index: Some(0),
                                    },
                                )
                                .unwrap();
                        } else {
                            match courses.get_mut(e) {
                                Some(c) => c.waypoints.push(*point),
                                None => {
                                    courses
                                        .insert(
                                            e,
                                            Course {
                                                waypoints: vec![*point],
                                                next_waypoint_index: Some(0),
                                            },
                                        )
                                        .unwrap();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub struct DockingSystem;

impl<'s> System<'s> for DockingSystem {
    type SystemData = (
        Entities<'s>,
        ReadStorage<'s, Ship>,
        ReadStorage<'s, Port>,
        ReadStorage<'s, Named>,
        WriteStorage<'s, Cargo>,
        WriteStorage<'s, Transform>,
        Write<'s, Notifications>,
    );

    fn run(&mut self, (entities, ships, ports, names, mut cargos, locals, mut notifications): Self::SystemData) {
        for (p, _, port_local) in (&entities, &ports, &locals).join() {
            let port_location = Point2::new(port_local.translation().x, port_local.translation().y);

            // If a ship is nearby prepare to load ship
            let suitable_ship = (&entities, &ships, &locals)
                .join()
                .filter(|(_, _, l)| {
                    let ship_location = Point2::new(l.translation().x, l.translation().y);
                    ship_location.distance(&port_location) < DISTANCE_THRESHOLD
                })
                .map(|(e, _, _)| e)
                .next();

            if let Some(ship) = suitable_ship {
                let port_cargo = cargos.get(p).unwrap().items.clone();
                let ship_cargo = cargos.get_mut(ship).unwrap();
                for (item, amount) in &port_cargo {
                    *ship_cargo.items.entry(*item).or_insert(0) += amount;
                }

                cargos.get_mut(p).unwrap().items.clear();
                if !port_cargo.is_empty() {
                    let items_notification = port_cargo
                        .iter()
                        .sorted_by_key(|(&item, _)| item)
                        .map(|(item, amount)| format!("{} tons of {}", amount, item))
                        .collect::<Vec<_>>()
                        .join(", ");
                    

                    notifications.push_back(format!(
                        "{} loaded onto ship at {}.",
                        items_notification,
                        names.get(p).unwrap().name.to_string()
                    ));
                }

            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{AiState, Cargo, ItemType};
    use amethyst::{ecs::Entity, prelude::*, Result};    
    use amethyst_test::prelude::*;
    use std::collections::HashMap;

    #[test]
    fn moves_ships_chooses_next_waypoint_if_close_enough_to_current_one() -> Result<()> {
        const CURRENT_WAYPOINT_INDEX: usize = 1;
        let waypoints = vec![Point2::new(2.0, 3.0), Point2::new(0.00001, 0.0002), Point2::new(20.0, -5.0)];

         AmethystApplication::blank()
            .with_system(MoveShipsSystem, "move_ships", &[])
            .with_effect(move |world| {
                let ship = world
                    .create_entity()
                    .with(Ship {base_speed: 1.0
                    })
                    .with(Course {waypoints: waypoints, next_waypoint_index: Some(CURRENT_WAYPOINT_INDEX)})
                    .with(Transform::default())
                    .build();

                world.insert(EffectReturn(ship));
            })
            .with_assertion(|world| {
                let ship_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();

                let courses = world.read_storage::<Course>();

                let course = courses.get(ship_entity).unwrap();
                assert_eq!(Some(CURRENT_WAYPOINT_INDEX + 1), course.next_waypoint_index, "Next waypoint index");
            })
            .run()
    }

    #[test]
    fn moves_ships_keeps_current_waypoint_if_not_close_enough_to_current_one() -> Result<()> {
        const CURRENT_WAYPOINT_INDEX: usize = 1;
        let waypoints = vec![Point2::new(2.0, 3.0), Point2::new(10.0, 20.0), Point2::new(20.0, -5.0)];

         AmethystApplication::blank()
            .with_system(MoveShipsSystem, "move_ships", &[])
            .with_effect(move |world| {
                let ship = world
                    .create_entity()
                    .with(Ship {base_speed: 1.0
                    })
                    .with(Course {waypoints: waypoints, next_waypoint_index: Some(CURRENT_WAYPOINT_INDEX)})
                    .with(Transform::default())
                    .build();

                world.insert(EffectReturn(ship));
            })
            .with_assertion(|world| {
                let ship_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();

                let courses = world.read_storage::<Course>();

                let course = courses.get(ship_entity).unwrap();
                assert_eq!(Some(CURRENT_WAYPOINT_INDEX), course.next_waypoint_index, "Next waypoint index");
            })
            .run()
    }
    
    #[test]
    fn moves_ships_moves_closer_to_current_waypoint() -> Result<()> {
        const CURRENT_WAYPOINT_INDEX: usize = 1;
        let current_waypoint = Point2::new(10.0, 20.0);
        let waypoints = vec![Point2::new(2.0, 3.0), current_waypoint, Point2::new(20.0, -5.0)];

        let original_local = Transform::default();
        let original_ship_location = Point2::new(original_local.translation().x, original_local.translation().y); 

         AmethystApplication::blank()
            .with_system(MoveShipsSystem, "move_ships", &[])
            .with_effect(move |world| {
                let ship = world
                    .create_entity()
                    .with(Ship {base_speed: 1.0
                    })
                    .with(Course {waypoints: waypoints, next_waypoint_index: Some(CURRENT_WAYPOINT_INDEX)})
                    .with(original_local)
                    .build();

                world.insert(EffectReturn(ship));
            })
            .with_assertion(move |world| {
                let ship_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();

                let locals = world.read_storage::<Transform>();
                let ship_local = locals.get(ship_entity).unwrap();
                    
                let ship_location = Point2::new(ship_local.translation().x, ship_local.translation().y);

                assert!(ship_location.distance(&current_waypoint) < original_ship_location.distance(&current_waypoint), "Ship closer to waypoint");
            })
            .run()
    }

    #[test]
    fn moves_ships_higher_base_speed_more() -> Result<()> {
        const CURRENT_WAYPOINT_INDEX: usize = 1;
        let current_waypoint = Point2::new(10.0, 20.0);
        let waypoints = vec![Point2::new(2.0, 3.0), current_waypoint, Point2::new(20.0, -5.0)];

        let original_local = Transform::default();

         AmethystApplication::blank()
            .with_system(MoveShipsSystem, "move_ships", &[])
            .with_effect(move |world| {
                let ship = world
                    .create_entity()
                    .with(Ship {base_speed: 1.0
                    })
                    .with(Course {waypoints: waypoints.clone(), next_waypoint_index: Some(CURRENT_WAYPOINT_INDEX)})
                    .with(original_local.clone())
                    .build();

                let faster_ship = world
                    .create_entity()
                    .with(Ship {base_speed: 2.0
                    })
                    .with(Course {waypoints: waypoints.clone(), next_waypoint_index: Some(CURRENT_WAYPOINT_INDEX)})
                    .with(original_local.clone())
                    .build();


                world.insert(EffectReturn((ship, faster_ship)));
            })
            .with_assertion(move |world| {
 let locals = world.read_storage::<Transform>();
 
                let ship_entity = world.read_resource::<EffectReturn<(Entity, Entity)>>().0.0.clone();
                let ship_local = locals.get(ship_entity).unwrap();
                let ship_location = Point2::new(ship_local.translation().x, ship_local.translation().y);

                let faster_ship_entity = world.read_resource::<EffectReturn<(Entity, Entity)>>().0.1.clone();
                let faster_ship_local = locals.get(faster_ship_entity).unwrap();
                let faster_ship_location = Point2::new(faster_ship_local.translation().x, faster_ship_local.translation().y);

                assert!(faster_ship_location.distance(&current_waypoint) < ship_location.distance(&current_waypoint), "Faster ship closer to waypoint");
            })
            .run()
    }

    #[test]
    fn moves_ships_next_waypoint_none_if_reached_final_waypoint() -> Result<()> {
        const CURRENT_WAYPOINT_INDEX: usize = 2;
        let waypoints = vec![Point2::new(2.0, 3.0), Point2::new(20.0, -5.0), Point2::new(0.00001, 0.0002)];

         AmethystApplication::blank()
            .with_system(MoveShipsSystem, "move_ships", &[])
            .with_effect(move |world| {
                let ship = world
                    .create_entity()
                    .with(Ship {base_speed: 1.0
                    })
                    .with(Course {waypoints: waypoints, next_waypoint_index: Some(CURRENT_WAYPOINT_INDEX)})
                    .with(Transform::default())
                    .build();

                world.insert(EffectReturn(ship));
            })
            .with_assertion(|world| {
                let ship_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();

                let courses = world.read_storage::<Course>();

                let course = courses.get(ship_entity).unwrap();
                assert_eq!(None, course.next_waypoint_index, "Next waypoint index");
            })
            .run()
    }

    #[test] 
    fn moves_ships_does_not_move_if_no_next_waypoint() -> Result<()> {
        let waypoints = vec![Point2::new(2.0, 3.0), Point2::new(20.0, -5.0), Point2::new(0.00001, 0.0002)];
        let original_local = Transform::default();
        let original_local_cloned = original_local.clone();

         AmethystApplication::blank()
            .with_system(MoveShipsSystem, "move_ships", &[])
            .with_effect(move |world| {
                let ship = world
                    .create_entity()
                    .with(Ship {base_speed: 1.0
                    })
                    .with(Course {waypoints: waypoints, next_waypoint_index: None})
                    .with(original_local)
                    .build();

                world.insert(EffectReturn(ship));
            })
            .with_assertion(move |world| {
                let ship_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();

                let locals = world.read_storage::<Transform>();

                let ship_local = locals.get(ship_entity).unwrap();
                assert_eq!(original_local_cloned, *ship_local, "Ship transform");
            })
            .run()
    }
    
    #[test] 
    fn moves_ships_does_not_move_non_ship_with_course() -> Result<()> {
        let waypoints = vec![Point2::new(2.0, 3.0), Point2::new(20.0, -5.0), Point2::new(25.0, 30.0)];
        let original_local = Transform::default();
        let original_local_cloned = original_local.clone();

         AmethystApplication::blank()
            .with_system(MoveShipsSystem, "move_ships", &[])
            .with_effect(move |world| {
                let entity = world
                    .create_entity()
                    .with(Course {waypoints: waypoints, next_waypoint_index: Some(0)})
                    .with(original_local)
                    .build();

                world.insert(EffectReturn(entity));
            })
            .with_assertion(move |world| {
                let entity = world.read_resource::<EffectReturn<Entity>>().0.clone();

                let locals = world.read_storage::<Transform>();

                let local = locals.get(entity).unwrap();
                assert_eq!(original_local_cloned, *local, "Ship transform");
            })
            .run()
    }

    #[test]
    fn ai_chooses_nearest_waypoint_if_previously_was_not_patrolling() -> Result<()> {
        let target_location = Point2::new(1.0, 5.0);
        let waypoints = vec![target_location.clone(), Point2::new(-10.0, -20.0), Point2::new(20.0, -5.0)];

         AmethystApplication::blank()
            .with_system(PatrolSystem, "patrol", &[])
            .with_effect(move |world| {
                let ai = world
                    .create_entity()
                    .with(Ai {
                        states: vec![
                            AiState {
                                transitions: HashMap::new(),
                                action: Action::Patrol,
                            },
                            AiState {
                                transitions: HashMap::new(),
                                action: Action::Chase,
                            },
                        ],
                        current_state_index: 0,
                        previous_state_index: 1,
                    })
                    .with(Patrol {waypoints: waypoints, next_waypoint_index: 1})
                    .with(Transform::default())
                    .build();

                world.insert(EffectReturn(ai));
            })
            .with_assertion(move |world| {
                let ai_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();

                let patrols = world.read_storage::<Patrol>();

                let ai_patrol = patrols.get(ai_entity).unwrap();
                assert_eq!(0, ai_patrol.next_waypoint_index);

                let courses = world.read_storage::<Course>();

                let ai_course = courses.get(ai_entity).unwrap();
                assert_eq!(1, ai_course.waypoints.len(), "Number of waypoints in course");
                assert_eq!(target_location, ai_course.waypoints[0], "Waypoint location");

            })
            .run()
    }

    #[test]
    fn ai_plots_a_course_to_next_waypoint_if_close_enough_to_current_one() -> Result<()> {
        let target_location = Point2::new(20.0, -5.0);
        let waypoints = vec![Point2::new(10.0, 5.0), Point2::new(0.0001, 0.0002), target_location.clone()];

         AmethystApplication::blank()
            .with_system(PatrolSystem, "patrol", &[])
            .with_effect(move |world| {
                let ai = world
                    .create_entity()
                    .with(Ai {
                        states: vec![
                            AiState {
                                transitions: HashMap::new(),
                                action: Action::Patrol,
                            },
                        ],
                        current_state_index: 0,
                        previous_state_index: 0,
                    })
                    .with(Patrol {waypoints: waypoints, next_waypoint_index: 1})
                    .with(Transform::default())
                    .build();

                world.insert(EffectReturn(ai));
            })
            .with_assertion(move |world| {
                let ai_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();

                let patrols = world.read_storage::<Patrol>();

                let ai_patrol = patrols.get(ai_entity).unwrap();
                assert_eq!(2, ai_patrol.next_waypoint_index);

                let courses = world.read_storage::<Course>();

                let ai_course = courses.get(ai_entity).unwrap();
                assert_eq!(1, ai_course.waypoints.len(), "Number of waypoints in course");
                assert_eq!(target_location, ai_course.waypoints[0], "Waypoint location");

            })
            .run()

     }

    #[test]
    fn ai_does_not_plot_a_course_to_next_waypoint_if_not_close_enough_to_current() -> Result<()> {
        let current_waypoint= Point2::new(1.0, 2.0);
        let waypoints = vec![Point2::new(10.0, 5.0), current_waypoint, Point2::new(20.0, 3.0)];

         AmethystApplication::blank()
            .with_system(PatrolSystem, "patrol", &[])
            .with_effect(move |world| {
                let ai = world
                    .create_entity()
                    .with(Ai {
                        states: vec![
                            AiState {
                                transitions: HashMap::new(),
                                action: Action::Patrol,
                            },
                        ],
                        current_state_index: 0,
                        previous_state_index: 0,
                    })
                    .with(Course {waypoints: vec![current_waypoint], next_waypoint_index: Some(0)}) 
                    .with(Patrol {waypoints: waypoints, next_waypoint_index: 1})
                    .with(Transform::default())
                    .build();

                world.insert(EffectReturn(ai));
            })
            .with_assertion(move |world| {
                let ai_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();

                let patrols = world.read_storage::<Patrol>();

                let ai_patrol = patrols.get(ai_entity).unwrap();
                assert_eq!(1, ai_patrol.next_waypoint_index);

                let courses = world.read_storage::<Course>();

                let ai_course = courses.get(ai_entity).unwrap();
                assert_eq!(1, ai_course.waypoints.len(), "Number of waypoints in course");
                assert_eq!(current_waypoint, ai_course.waypoints[0], "Waypoint location");

            })
            .run()
   }


     #[test]
     fn ai_does_not_patrol_if_action_is_not_to_patrol() -> Result<()> {
        let waypoints = vec![Point2::new(10.0, 5.0), Point2::new(1.0, 2.0), Point2::new(20.0, 3.0)];

         AmethystApplication::blank()
            .with_system(PatrolSystem, "patrol", &[])
            .with_effect(move |world| {
                let ai = world
                    .create_entity()
                    .with(Ai {
                        states: vec![
                            AiState {
                                transitions: HashMap::new(),
                                action: Action::Chase,
                            },
                        ],
                        current_state_index: 0,
                        previous_state_index: 0,
                    })
                    .with(Patrol {waypoints: waypoints, next_waypoint_index: 1})
                    .with(Transform::default())
                    .build();

                world.insert(EffectReturn(ai));
            })
            .with_assertion(move |world| {
                let ai_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();

                let courses = world.read_storage::<Course>();

                assert!( courses.get(ai_entity).is_none(), "Ai course does not exist");
            })
            .run()
    }

     #[test]
     fn ai_does_not_patrol_if_it_has_no_patrol() -> Result<()> {
         AmethystApplication::blank()
            .with_system(PatrolSystem, "patrol", &[])
            .with_effect(move |world| {
                let ai = world
                    .create_entity()
                    .with(Ai {
                        states: vec![
                            AiState {
                                transitions: HashMap::new(),
                                action: Action::Patrol,
                            },
                        ],
                        current_state_index: 0,
                        previous_state_index: 0,
                    })
                    .with(Transform::default())
                    .build();

                world.insert(EffectReturn(ai));
            })
            .with_assertion(move |world| {
                let ai_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();

                let courses = world.read_storage::<Course>();

                assert!( courses.get(ai_entity).is_none(), "Ai course does not exist");
            })
            .run()
    }

   
    #[test]
    fn ai_chases_nearest_ship() -> Result<()> {
        AmethystApplication::blank()
            .with_system(ChaseSystem, "chase", &[])
            .with_effect(|world| {
                let ai = world
                    .create_entity()
                    .with(Ai {
                        states: vec![
                            AiState {
                                transitions: HashMap::new(),
                                action: Action::Chase,
                            },
                        ],
                        current_state_index: 0,
                        previous_state_index: 0,
                    })
                    .with(Transform::default())
                    .build();

                let mut target_transform = Transform::default();
                target_transform.set_translation_xyz(2.0, 1.0, 0.0);

                let target = world
                    .create_entity()
                    .with(Ship { base_speed: 1.0 })
                    .with(target_transform)
                    .build();

                let mut other_transform = Transform::default();
                other_transform.set_translation_xyz(6.0, 5.0, 0.0);

                world.create_entity()
                    .with(Ship { base_speed: 1.0 })
                    .with(other_transform)
                    .build();

                world.insert(EffectReturn((ai, target)));
            })
            .with_assertion(move |world| {
                let ai_entity = world.read_resource::<EffectReturn<(Entity, Entity)>>().0.0.clone();
                let target_entity = world.read_resource::<EffectReturn<(Entity, Entity)>>().0.1.clone();
                

                let locals = world.read_storage::<Transform>();

                let target_transform = locals.get(target_entity).unwrap();
                let target_location = Point2::new(target_transform.translation().x, target_transform.translation().y);

                let courses = world.read_storage::<Course>();
                
                let ai_course = courses.get(ai_entity).unwrap();
                assert_eq!(1, ai_course.waypoints.len(), "Number of waypoints in course");
                assert_eq!(target_location, ai_course.waypoints[0], "Waypoint location");

            })
            .run()
    }

    #[test]
    fn ai_does_not_chase_itself() -> Result<()> {
        AmethystApplication::blank()
            .with_system(ChaseSystem, "chase", &[])
            .with_effect(|world| {
                let ai = world
                    .create_entity()
                    .with(Ai {
                        states: vec![
                            AiState {
                                transitions: HashMap::new(),
                                action: Action::Chase,
                            },
                        ],
                        current_state_index: 0,
                        previous_state_index: 0,
                    })
                    .with(Transform::default())
                    .build();

                world.insert(EffectReturn(ai));
            })
            .with_assertion(|world| {
                let ai_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();
                let courses = world.read_storage::<Course>();
                
                assert!(courses.get(ai_entity).is_none(), "Ai course does not exist");
            })
            .run()
    }

    #[test]
    fn ai_only_chases_ships() -> Result<()> {
        AmethystApplication::blank()
            .with_system(ChaseSystem, "chase", &[])
            .with_effect(|world| {
                let ai = world
                    .create_entity()
                    .with(Ai {
                        states: vec![
                            AiState {
                                transitions: HashMap::new(),
                                action: Action::Chase,
                            },
                        ],
                        current_state_index: 0,
                        previous_state_index: 0,
                    })
                    .with(Transform::default())
                    .build();

                let mut target_transform = Transform::default();
                target_transform.set_translation_xyz(2.0, 1.0, 0.0);

                world
                    .create_entity()
                    .with(target_transform)
                    .build();

                world.insert(EffectReturn(ai));
            })
            .with_assertion(|world| {
                let ai_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();
                let courses = world.read_storage::<Course>();
                
                assert!(courses.get(ai_entity).is_none(), "Ai course does not exist");

            })
            .run()
    }

    #[test]
    fn ai_does_not_chase_if_action_is_not_to_chase() -> Result<()> {
        AmethystApplication::blank()
            .with_system(ChaseSystem, "chase", &[])
            .with_effect(|world| {
                let ai = world
                    .create_entity()
                    .with(Ai {
                        states: vec![
                            AiState {
                                transitions: HashMap::new(),
                                action: Action::Patrol,
                            },
                        ],
                        current_state_index: 0,
                        previous_state_index: 0,
                    })
                    .with(Transform::default())
                    .build();

                let mut target_transform = Transform::default();
                target_transform.set_translation_xyz(2.0, 1.0, 0.0);

                world
                    .create_entity()
                    .with(Ship { base_speed: 1.0 })
                    .with(target_transform)
                    .build();

                world.insert(EffectReturn(ai));
            })
            .with_assertion(|world| {
                let ai_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();

                let courses = world.read_storage::<Course>();
                
                assert!(courses.get(ai_entity).is_none(), "Ai course does not exist");
            })
            .run()
    }   

    #[test]
    fn cargo_transferred_if_ship_nearby() -> Result<()> {
        let goods_in_port: HashMap<ItemType, u32> = [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
            .iter()
            .cloned()
            .collect();

        let original_goods_on_ship: HashMap<ItemType, u32> = [
            (ItemType::Sugar, 10),
            (ItemType::Whiskey, 5),
            (ItemType::Rum, 10),
        ]
        .iter()
        .cloned()
        .collect();

        let mut expected_goods_on_ship = original_goods_on_ship.clone();
        for (k, v) in goods_in_port.iter() {
            *expected_goods_on_ship.entry(*k).or_insert(0) += v;
        }

        AmethystApplication::blank()
            .with_system(DockingSystem, "docking", &[])
            .with_effect(move |world| {
                let port = world
                    .create_entity()
                    .with(Port)
                    .named("A port")
                    .with(Cargo {
                        items: goods_in_port.clone(),
                    })
                    .with(Transform::default())
                    .build();

                let mut ship_transform = Transform::default();
                ship_transform.set_translation_xyz(0.0001, 0.0002, 0.0);

                let ship = world
                    .create_entity()
                    .with(Ship { base_speed: 1.0 })
                    .with(Cargo {
                        items: original_goods_on_ship.clone(),
                    })
                    .with(ship_transform)
                    .build();

                world.insert(EffectReturn((ship, port)));
            })
            .with_assertion(move |world| {
                let ship_entity = world.read_resource::<EffectReturn<(Entity, Entity)>>().0.0.clone();
                let port_entity = world.read_resource::<EffectReturn<(Entity, Entity)>>().0.1.clone();
                
                let cargos = world.read_storage::<Cargo>();
                
                let port_cargo = cargos.get(port_entity).unwrap();
                assert_eq!(HashMap::new(), port_cargo.items, "Cargo on port");

                let ship_cargo = cargos.get(ship_entity).unwrap();
                assert_eq!(expected_goods_on_ship, ship_cargo.items, "Cargon on ship");
            })
            .run()
    }

    #[test]
    fn notification_sent_if_cargo_loaded() -> Result<()> {
        const PORT: &str = "London";
        let goods_in_port: HashMap<ItemType, u32> = [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
            .iter()
            .cloned()
            .collect();

        AmethystApplication::blank()
            .with_system(DockingSystem, "docking", &[])
            .with_effect(move |world| {
                world
                    .create_entity()
                    .with(Port)
                    .named(PORT)
                    .with(Cargo {
                        items: goods_in_port.clone(),
                    })
                    .with(Transform::default())
                    .build();

                let mut ship_transform = Transform::default();
                ship_transform.set_translation_xyz(0.0001, 0.0002, 0.0);

                world
                    .create_entity()
                    .with(Ship { base_speed: 1.0 })
                    .with(Cargo {
                        items: HashMap::new(),
                    })
                    .with(ship_transform)
                    .build();

            })
            .with_assertion(move |world| {
                let notifications = world.read_resource::<Notifications>().clone();
                assert_eq!(1, notifications.len(), "Number of notifications");
                assert_eq!(
                    &format!(
                        "5 tons of Rum, 10 tons of Sugar loaded onto ship at {}.",
                        PORT
                    ),
                    notifications.front().unwrap(),
                    "Notification"
                );

            })
            .run()
    }

    #[test]
    fn cargo_not_transferred_if_ship_not_nearby() -> Result<()> {
        let goods_in_port: HashMap<ItemType, u32> = [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
            .iter()
            .cloned()
            .collect();

        let goods_in_port_cloned = goods_in_port.clone();

        let goods_on_ship: HashMap<ItemType, u32> = [
            (ItemType::Sugar, 10),
            (ItemType::Whiskey, 5),
            (ItemType::Rum, 10),
        ]
        .iter()
        .cloned()
        .collect();

        let goods_on_ship_cloned = goods_on_ship.clone();

        AmethystApplication::blank()
            .with_system(DockingSystem, "docking", &[])
            .with_effect(move |world| {
                let port = world
                    .create_entity()
                    .with(Port)
                    .named("A port")
                    .with(Cargo {
                        items: goods_in_port.clone(),
                    })
                    .with(Transform::default())
                    .build();

                let mut ship_transform = Transform::default();
                ship_transform.set_translation_xyz(10.0, 0.0, 0.0);

                let ship = world
                    .create_entity()
                    .with(Ship { base_speed: 1.0 })
                    .with(Cargo {
                        items: goods_on_ship.clone(),
                    })
                    .with(ship_transform)
                    .build();

                world.insert(EffectReturn((ship, port)));
            })
            .with_assertion(move |world| {
                let ship_entity = world.read_resource::<EffectReturn<(Entity, Entity)>>().0.0.clone();
                let port_entity = world.read_resource::<EffectReturn<(Entity, Entity)>>().0.1.clone();
                
                let cargos = world.read_storage::<Cargo>();
                
                let port_cargo = cargos.get(port_entity).unwrap();
                assert_eq!(goods_in_port_cloned, port_cargo.items, "Cargo on port");

                let ship_cargo = cargos.get(ship_entity).unwrap();
                assert_eq!(goods_on_ship_cloned, ship_cargo.items, "Cargo on ship");
            })
            .run()
    }

}
