use std::cmp::Ordering;

use amethyst::{
    core::{
        alga::linear::EuclideanSpace,
        math::{Point2, Vector2},
        Time, Transform,
    },
    derive::SystemDesc,
    ecs::{Entities, Join, Read, ReadExpect, ReadStorage, System, SystemData, WriteStorage},
    input::{InputHandler, StringBindings, VirtualKeyCode},
    window::ScreenDimensions,
    winit::MouseButton,
};

use crate::{
    age_of_sail::{point_mouse_to_world, DISTANCE_THRESHOLD},
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
        (entities, ships, locals, selecteds, controllables, mut courses, input, screen_dimensions): Self::SystemData,
    ) {
        for (e, _, _, _) in (&entities, &ships, &selecteds, &controllables).join() {
            if let Some((mouse_x, mouse_y)) = input.mouse_position() {
                if input.mouse_button_is_down(MouseButton::Right) {
                    let point_in_world =
                        point_mouse_to_world(mouse_x, mouse_y, &*screen_dimensions);

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

pub struct DockingSystem;

impl<'s> System<'s> for DockingSystem {
    type SystemData = (
        Entities<'s>,
        ReadStorage<'s, Ship>,
        ReadStorage<'s, Port>,
        WriteStorage<'s, Cargo>,
        WriteStorage<'s, Transform>,
    );

    fn run(&mut self, (entities, ships, ports, mut cargos, locals): Self::SystemData) {
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
                for (item, amount) in port_cargo {
                    *ship_cargo.items.entry(item).or_insert(0) += amount;
                }

                cargos.get_mut(p).unwrap().items.clear();
            }
        }
    }
}
