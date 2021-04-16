use amethyst::{
    core::{math::Point2, Transform},
    ecs::{storage::DenseVecStorage, Component},
};

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct BoundingBox {
    pub width: f32,
    pub origin: Point2<f32>,
}

impl BoundingBox {
    pub fn as_2d_points(
        &self,
        local: &Transform,
    ) -> (Point2<f32>, Point2<f32>, Point2<f32>, Point2<f32>) {
        let centre_x = local.translation().x;
        let centre_y = local.translation().y;

        let top_left = Point2::new(
            centre_x + self.origin.x - self.width * 0.5,
            centre_y + self.origin.y + self.width * 0.5,
        );

        let top_right = Point2::new(
            centre_x + self.origin.x + self.width * 0.5,
            centre_y + self.origin.y + self.width * 0.5,
        );

        let bottom_right = Point2::new(
            centre_x + self.origin.x + self.width * 0.5,
            centre_y + self.origin.y - self.width * 0.5,
        );

        let bottom_left = Point2::new(
            centre_x + self.origin.x - self.width * 0.5,
            centre_y + self.origin.y - self.width * 0.5,
        );

        (top_left, top_right, bottom_right, bottom_left)
    }

    pub fn as_boundaries(&self, local: &Transform) -> (f32, f32, f32, f32) {
        let centre_x = local.translation().x;
        let centre_y = local.translation().y;

        let left = centre_x + self.origin.x - self.width * 0.5;
        let right = centre_x + self.origin.x + self.width * 0.5;
        let top = centre_y + self.origin.y + self.width * 0.5;
        let bottom = centre_y + self.origin.y - self.width * 0.5;

        (left, right, top, bottom)
    }
}
