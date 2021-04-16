use amethyst::{
    core::math::Point2,
    ecs::{storage::DenseVecStorage, Component},
};

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct Course {
    pub waypoints: Vec<Point2<f32>>,
    pub next_waypoint_index: Option<usize>,
}

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct Patrol {
    pub waypoints: Vec<Point2<f32>>,
    pub next_waypoint_index: usize,
}
