use amethyst::{
    core::math::Point2,
    ecs::{storage::DenseVecStorage, Component},
};
use std::collections::VecDeque;

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct Course {
    pub waypoints: VecDeque<Point2<f32>>,
}

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct Patrol {
    pub waypoints: Vec<Point2<f32>>,
    pub next_waypoint_index: usize,
}
