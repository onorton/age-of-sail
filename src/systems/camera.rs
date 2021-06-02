use amethyst::{
    core::{Time, Transform},
    ecs::{Join, Read, ReadExpect, ReadStorage, System, WriteStorage},
    input::{InputHandler, StringBindings},
    renderer::Camera,
    window::ScreenDimensions,
};

const CAMERA_SPEED: f32 = 30.0;
const PANNING_REGION_PIXELS: f32 = 15.0;

pub struct PanningSystem;

impl<'s> System<'s> for PanningSystem {
    type SystemData = (
        ReadStorage<'s, Camera>,
        WriteStorage<'s, Transform>,
        Read<'s, InputHandler<StringBindings>>,
        ReadExpect<'s, ScreenDimensions>,
        Read<'s, Time>,
    );

    fn run(&mut self, (cameras, mut locals, input, screen_dimensions, time): Self::SystemData) {
        for (_, local) in (&cameras, &mut locals).join() {
            if let Some((mouse_x, mouse_y)) = input.mouse_position() {
                let camera_pixel_shift = CAMERA_SPEED * time.delta_real_seconds();

                let shift_x = if mouse_x < PANNING_REGION_PIXELS {
                    -camera_pixel_shift
                } else if mouse_x > screen_dimensions.width() - PANNING_REGION_PIXELS {
                    camera_pixel_shift
                } else {
                    0.0
                };

                let shift_y = if mouse_y < PANNING_REGION_PIXELS {
                    camera_pixel_shift
                } else if mouse_y > screen_dimensions.height() - PANNING_REGION_PIXELS {
                    -camera_pixel_shift
                } else {
                    0.0
                };

                local.prepend_translation_x(shift_x);
                local.prepend_translation_y(shift_y);
            }
        }
    }
}
