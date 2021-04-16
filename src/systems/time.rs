use amethyst::{
    core::Time,
    ecs::{System, Write, WriteStorage},
    ui::{UiFinder, UiText},
};

// How many seconds pass by in the game world compared to in real time
// At base speed
pub const IN_GAME_TO_REAL_TIME_SECONDS: f32 = 3600.0;

use crate::age_of_sail::Date;

pub struct UpdateTimeSystem;

impl<'s> System<'s> for UpdateTimeSystem {
    type SystemData = (
        WriteStorage<'s, UiText>,
        Write<'s, Date>,
        Write<'s, Time>,
        UiFinder<'s>,
    );

    fn run(&mut self, (mut ui_texts, mut date, mut time, finder): Self::SystemData) {
        time.set_time_scale(date.game_speed());
        date.time_elapsed += (IN_GAME_TO_REAL_TIME_SECONDS * time.delta_seconds()) as f64;

        let current_time = finder.find("current_time");

        if let Some(current_time_ui) = current_time {
            if let Some(ui_text) = ui_texts.get_mut(current_time_ui) {
                ui_text.text = date.current_date();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use amethyst::{
        assets::{AssetStorage, Loader},
        ecs::Entity,
        input::StringBindings,
        prelude::*,
        ui::{Anchor, FontAsset, LineMode, TtfFormat, UiText, UiTransform},
        Result,
    };
    use amethyst_test::prelude::*;

    #[test]
    fn accepting_contract_sends_ui_update_event() -> Result<()> {
        AmethystApplication::ui_base::<StringBindings>()
            .with_system(UpdateTimeSystem, "update_time", &[])
            .with_effect(|world| {
                let font_handle = {
                    let loader = world.read_resource::<Loader>();
                    let font_storage = world.read_resource::<AssetStorage<FontAsset>>();
                    loader.load("font/square.ttf", TtfFormat, (), &font_storage)
                };

                let ui_entity = world
                    .create_entity()
                    .with(UiText::new(
                        font_handle.clone(),
                        "string".to_string(),
                        [0.0, 0.0, 0.0, 1.0],
                        10.0,
                        LineMode::Single,
                        Anchor::TopLeft,
                    ))
                    .with(UiTransform::new(
                        "current_time".to_string(),
                        Anchor::TopLeft,
                        Anchor::TopLeft,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                    ))
                    .build();

                world.insert(EffectReturn(ui_entity));
            })
            .with_assertion(|world| {
                let ui_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();

                let ui_texts = world.read_storage::<UiText>();

                let ui_text = ui_texts.get(ui_entity).unwrap();

                assert_ne!("string".to_string(), ui_text.text)
            })
            .run()
    }
}
