use ::amethyst::core::SystemDesc;
use amethyst::{
    core::{Parent, RunNowDesc, timing::Time},
    ecs::{
        Entities, Entity, Join, Read, ReadExpect, ReadStorage, System, SystemData, World, Write,
        WriteStorage,
    },
    shrev::{EventChannel, ReaderId},
    ui::{
        Anchor, Interactable, LineMode, UiEvent, UiEventType, UiFinder, UiImage, UiText,
        UiTransform,
    },
};

use crate::{
    age_of_sail::{Date, PlayerStatus, UiAssets, Notifications},
    components::{Contract, OwnedBy},
};
use crate::{components::Port, event::UiUpdateEvent};

const NOTIFICATION_TIME: f32 = 5.0;

pub struct PortPanelSystem {
    reader_id: ReaderId<UiUpdateEvent>,
    selected_port: Option<Entity>,
}

impl PortPanelSystem {
    fn new(reader_id: ReaderId<UiUpdateEvent>) -> Self {
        PortPanelSystem {
            reader_id,
            selected_port: None,
        }
    }
}

impl<'s> System<'s> for PortPanelSystem {
    type SystemData = (
        Entities<'s>,
        ReadStorage<'s, Port>,
        ReadStorage<'s, Contract>,
        WriteStorage<'s, OwnedBy>,
        Read<'s, EventChannel<UiUpdateEvent>>,
        WriteStorage<'s, UiText>,
        WriteStorage<'s, UiTransform>,
        WriteStorage<'s, UiImage>,
        WriteStorage<'s, Interactable>,
        WriteStorage<'s, Parent>,
        ReadExpect<'s, UiAssets>,
    );

    fn run(
        &mut self,
        (
            entities,
            ports,
            contracts,
            mut owned_bys,
            channel,
            mut ui_texts,
            mut ui_transforms,
            mut ui_images,
            mut ui_interactables,
            mut parents,
            ui_assets,
        ): Self::SystemData,
    ) {
        for event in channel.read(&mut self.reader_id) {
            let target = match event {
                UiUpdateEvent::Target(e) => Some(*e),
                _ => None,
            };

            if let Some(target) = target {
                let e = if ports.get(target).is_some() {
                    self.selected_port.replace(target);
                    target
                } else {
                    self.selected_port.unwrap_or(target)
                };

                if let Some(port) = ports.get(e) {
                    let contract_ui_elements =
                        find_ui_elements(&entities, &ui_transforms, "contract");
                    for c in contract_ui_elements {
                        entities.delete(c).unwrap();
                    }

                    let port_name_element =
                        find_ui_element(&entities, &ui_transforms, "port_info_name").unwrap();

                    if let Some(text) = ui_texts.get_mut(port_name_element) {
                        text.text = port.name.clone();
                    };

                    let contracts_held_by_port = (&entities, &contracts, &owned_bys)
                        .join()
                        .filter(|(_, _, o)| o.entity == e)
                        .map(|(e, c, _)| (e, c))
                        .collect::<Vec<(Entity, &Contract)>>();

                    let port_info_container =
                        find_ui_element(&entities, &ui_transforms, "port_info").unwrap();

                    let mut offset = 50.;

                    for (e, c) in contracts_held_by_port {
                        let destination_name = ports.get(c.destination).unwrap().name.clone();

                        let contract_ui_height = 70. + 20. * c.goods_required.keys().len() as f32;
                        let contract_ui_width = 175.;

                        let contract_parent = entities
                            .build_entity()
                            .with(
                                UiTransform::new(
                                    "contract".to_string(),
                                    Anchor::TopMiddle,
                                    Anchor::TopMiddle,
                                    0.,
                                    -offset,
                                    1.,
                                    contract_ui_width,
                                    contract_ui_height,
                                ),
                                &mut ui_transforms,
                            )
                            .with(
                                UiImage::NineSlice {
                                    x_start: 4,
                                    y_start: 4,
                                    width: 56,
                                    height: 56,
                                    left_dist: 4,
                                    right_dist: 4,
                                    top_dist: 4,
                                    bottom_dist: 4,
                                    tex: ui_assets.panel.clone(),
                                    texture_dimensions: [64, 64],
                                },
                                &mut ui_images,
                            )
                            .with(
                                Parent {
                                    entity: port_info_container,
                                },
                                &mut parents,
                            )
                            .build();

                        entities
                            .build_entity()
                            .with(
                                UiText::new(
                                    ui_assets.font.clone(),
                                    format!("For: {}", destination_name),
                                    [1.0, 1.0, 1.0, 1.0],
                                    15.,
                                    LineMode::Single,
                                    Anchor::Middle,
                                ),
                                &mut ui_texts,
                            )
                            .with(
                                UiTransform::new(
                                    "contract_destination".to_string(),
                                    Anchor::TopMiddle,
                                    Anchor::TopMiddle,
                                    0.,
                                    -5.,
                                    1.,
                                    contract_ui_width,
                                    20.,
                                ),
                                &mut ui_transforms,
                            )
                            .with(
                                Parent {
                                    entity: contract_parent,
                                },
                                &mut parents,
                            )
                            .build();

                        entities
                            .build_entity()
                            .with(
                                UiText::new(
                                    ui_assets.font.clone(),
                                    format!("£{}", c.payment),
                                    [1.0, 1.0, 1.0, 1.0],
                                    15.,
                                    LineMode::Single,
                                    Anchor::Middle,
                                ),
                                &mut ui_texts,
                            )
                            .with(
                                UiTransform::new(
                                    "contract_payment".to_string(),
                                    Anchor::TopMiddle,
                                    Anchor::TopMiddle,
                                    0.,
                                    -25.,
                                    1.,
                                    contract_ui_width,
                                    20.,
                                ),
                                &mut ui_transforms,
                            )
                            .with(
                                Parent {
                                    entity: contract_parent,
                                },
                                &mut parents,
                            )
                            .build();

                        let mut goods_offset = 45.;
                        for (item, amount) in &c.goods_required {
                            entities
                                .build_entity()
                                .with(
                                    UiText::new(
                                        ui_assets.font.clone(),
                                        format!("{}: {} tons", item, amount),
                                        [1.0, 1.0, 1.0, 1.0],
                                        15.,
                                        LineMode::Single,
                                        Anchor::Middle,
                                    ),
                                    &mut ui_texts,
                                )
                                .with(
                                    UiTransform::new(
                                        "goods".to_string(),
                                        Anchor::TopMiddle,
                                        Anchor::TopMiddle,
                                        0.,
                                        -goods_offset,
                                        1.,
                                        175.,
                                        20.,
                                    ),
                                    &mut ui_transforms,
                                )
                                .with(
                                    Parent {
                                        entity: contract_parent,
                                    },
                                    &mut parents,
                                )
                                .build();

                            goods_offset += 20.;
                        }

                        entities
                            .build_entity()
                            .with(
                                UiText::new(
                                    ui_assets.font.clone(),
                                    "Accept".to_string(),
                                    [1.0, 1.0, 1.0, 1.0],
                                    15.,
                                    LineMode::Single,
                                    Anchor::Middle,
                                ),
                                &mut ui_texts,
                            )
                            .with(
                                UiTransform::new(
                                    "accept_button".to_string(),
                                    Anchor::BottomMiddle,
                                    Anchor::BottomMiddle,
                                    0.,
                                    5.,
                                    1.,
                                    60.,
                                    20.,
                                ),
                                &mut ui_transforms,
                            )
                            .with(
                                UiImage::NineSlice {
                                    x_start: 6,
                                    y_start: 6,
                                    width: 52,
                                    height: 52,
                                    left_dist: 2,
                                    right_dist: 2,
                                    top_dist: 2,
                                    bottom_dist: 2,
                                    tex: ui_assets.panel.clone(),
                                    texture_dimensions: [64, 64],
                                },
                                &mut ui_images,
                            )
                            .with(
                                Parent {
                                    entity: contract_parent,
                                },
                                &mut parents,
                            )
                            .with(Interactable, &mut ui_interactables)
                            .with(OwnedBy { entity: e }, &mut owned_bys)
                            .build();

                        offset += contract_ui_height + 5.;
                    }
                }
            }
        }
    }
}

pub struct PortPanelSystemDesc;

impl Default for PortPanelSystemDesc {
    fn default() -> Self {
        PortPanelSystemDesc {}
    }
}

impl<'a, 'b> SystemDesc<'a, 'b, PortPanelSystem> for PortPanelSystemDesc {
    fn build(self, world: &mut World) -> PortPanelSystem {
        <PortPanelSystem as System<'_>>::SystemData::setup(world);

        let reader_id = world
            .fetch_mut::<EventChannel<UiUpdateEvent>>()
            .register_reader();

        PortPanelSystem::new(reader_id)
    }
}

impl<'a, 'b> RunNowDesc<'a, 'b, PortPanelSystem> for PortPanelSystemDesc {
    fn build(self, world: &mut World) -> PortPanelSystem {
        <PortPanelSystemDesc as SystemDesc<'a, 'b, PortPanelSystem>>::build(self, world)
    }
}

pub struct PlayerStatusSystem {
    reader_id: ReaderId<UiUpdateEvent>,
}

impl PlayerStatusSystem {
    fn new(reader_id: ReaderId<UiUpdateEvent>) -> Self {
        PlayerStatusSystem { reader_id }
    }
}

impl<'s> System<'s> for PlayerStatusSystem {
    type SystemData = (
        WriteStorage<'s, UiText>,
        Read<'s, PlayerStatus>,
        Read<'s, EventChannel<UiUpdateEvent>>,
        UiFinder<'s>,
    );

    fn run(&mut self, (mut ui_texts, player_status, channel, finder): Self::SystemData) {
        for event in channel.read(&mut self.reader_id) {
            if let UiUpdateEvent::PlayerStatus = event {
                let player_money = finder.find("player_money").unwrap();
                if let Some(ui_text) = ui_texts.get_mut(player_money) {
                    ui_text.text = format!("£{}", player_status.money);
                }
            }
        }
    }
}

pub struct PlayerStatusSystemDesc;

impl Default for PlayerStatusSystemDesc {
    fn default() -> Self {
        PlayerStatusSystemDesc {}
    }
}

impl<'a, 'b> SystemDesc<'a, 'b, PlayerStatusSystem> for PlayerStatusSystemDesc {
    fn build(self, world: &mut World) -> PlayerStatusSystem {
        <PlayerStatusSystem as System<'_>>::SystemData::setup(world);

        let reader_id = world
            .fetch_mut::<EventChannel<UiUpdateEvent>>()
            .register_reader();

        PlayerStatusSystem::new(reader_id)
    }
}

pub struct GameSpeedSystem {
    reader_id: ReaderId<UiEvent>,
}

impl GameSpeedSystem {
    fn new(reader_id: ReaderId<UiEvent>) -> Self {
        GameSpeedSystem { reader_id }
    }
}

impl<'s> System<'s> for GameSpeedSystem {
    type SystemData = (
        Entities<'s>,
        WriteStorage<'s, UiTransform>,
        Read<'s, EventChannel<UiEvent>>,
        Write<'s, Date>,
    );

    fn run(&mut self, (entities, mut ui_transforms, channel, mut date): Self::SystemData) {
        for event in channel.read(&mut self.reader_id) {
            if event.event_type == UiEventType::ClickStop {
                let button = ui_transforms
                    .get(event.target)
                    .map_or(None, |t| Some(t.id.clone()));

                if let Some(button_id) = button {
                    match button_id.as_ref() {
                        "play_button" => {
                            date.paused = false;

                            let mut pause_button = ui_transforms
                                .get_mut(
                                    find_ui_element(&entities, &ui_transforms, "pause_button")
                                        .unwrap(),
                                )
                                .unwrap();
                            pause_button.local_z = 1.;

                            let mut play_button = ui_transforms.get_mut(event.target).unwrap();
                            play_button.local_z = -1.
                        }
                        "pause_button" => {
                            date.paused = true;
                            let mut play_button = ui_transforms
                                .get_mut(
                                    find_ui_element(&entities, &ui_transforms, "play_button")
                                        .unwrap(),
                                )
                                .unwrap();
                            play_button.local_z = 1.;

                            let mut pause_button = ui_transforms.get_mut(event.target).unwrap();
                            pause_button.local_z = -1.
                        }
                        "increase_speed_button" => {
                            let new_speed = 2. * date.current_speed;
                            let max = 8.;
                            date.current_speed = if new_speed <= max { new_speed } else { max };
                        }
                        "decrease_speed_button" => {
                            let new_speed = 0.5 * date.current_speed;
                            let min = 1.;
                            date.current_speed = if new_speed >= min { new_speed } else { min };
                        }
                        _ => (),
                    };
                }
            }
        }
    }
}

pub struct GameSpeedSystemDesc;

impl Default for GameSpeedSystemDesc {
    fn default() -> Self {
        GameSpeedSystemDesc {}
    }
}

impl<'a, 'b> SystemDesc<'a, 'b, GameSpeedSystem> for GameSpeedSystemDesc {
    fn build(self, world: &mut World) -> GameSpeedSystem {
        <GameSpeedSystem as System<'_>>::SystemData::setup(world);

        let reader_id = world.fetch_mut::<EventChannel<UiEvent>>().register_reader();

        GameSpeedSystem::new(reader_id)
    }
}


#[derive(Default)]
pub struct NotificationSystem {
    time_passed: Option<f32>,
}

impl<'s> System<'s> for NotificationSystem {
    type SystemData = (
        WriteStorage<'s, UiText>,
        Write<'s, Notifications>, 
        Read<'s, Time>,
        UiFinder<'s>, 
    );

    fn run(&mut self, (mut ui_texts, mut notifications, time, finder): Self::SystemData) {
        let mut replace_message = true;
        let mut new_message = "".to_string();
        if let Some(t) = self.time_passed {
             let new_t = t + time.delta_real_seconds();
             if new_t > NOTIFICATION_TIME {
                 new_message = notifications.pop_front().unwrap_or("".to_string());
                 self.time_passed = None;
             } else if notifications.len() > 0 {
                new_message = notifications.pop_front().unwrap();
             } else {
                self.time_passed.replace(new_t);
                replace_message = false;
             }
         } else {
            new_message = notifications.pop_front().unwrap_or("".to_string());
        }

        if replace_message {
            if new_message != "".to_string() {
                self.time_passed.replace(0.0);
            }

            if let Some(notification) = finder.find("notification") {
                if let Some(ui_text) = ui_texts.get_mut(notification) {
                    ui_text.text = new_message;
                }
            }
        }
    }
}

fn find_ui_element<'a>(
    entities: &Entities<'a>,
    ui_transforms: &WriteStorage<'a, UiTransform>,
    id: &str,
) -> Option<Entity> {
    (entities, ui_transforms)
        .join()
        .filter(|(_, t)| t.id == id)
        .map(|(e, _)| e)
        .next()
}

fn find_ui_elements<'a>(
    entities: &Entities<'a>,
    ui_transforms: &WriteStorage<'a, UiTransform>,
    id: &str,
) -> Vec<Entity> {
    (entities, ui_transforms)
        .join()
        .filter(|(_, t)| t.id == id)
        .map(|(e, _)| e)
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use amethyst::{
        input::StringBindings,
        prelude::*,
        ecs::Entity,
        assets::{Loader, AssetStorage},
        ui::{Anchor, UiTransform, FontAsset, TtfFormat},
        Result,
    };
    use amethyst_test::prelude::*;
    use test_case::test_case;


    #[test]
    fn text_for_player_status_is_set() -> Result<()> {
        const MONEY: i32 = 100;

        AmethystApplication::ui_base::<StringBindings>()
            .with_system_desc(PlayerStatusSystemDesc, "player_status", &[])
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
                        "player_money".to_string(),
                        Anchor::TopLeft,
                        Anchor::TopLeft,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                    ))
                    .build();

                world.insert(PlayerStatus {money: MONEY});
                world.insert(EffectReturn(ui_entity));
            })
            .with_assertion(|world| {
                let ui_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();

                let ui_texts = world.read_storage::<UiText>();

                let ui_text = ui_texts.get(ui_entity).unwrap();

                assert_ne!(format!("£{}", MONEY), ui_text.text)
            })
            .run()
    }

    #[test_case("play", "pause", false ; "play button")]
    #[test_case("pause", "play", true ; "pause button")]
    fn pressing_play_or_pause_button_causes_state_toggle<'a>(
        pressed: &'a str,
        other: &'a str,
        paused: bool,
    ) {
        let pressed_id = pressed.to_string();
        let other_id = other.to_string();

        AmethystApplication::ui_base::<StringBindings>()
            .with_system_desc(GameSpeedSystemDesc, "game_speed", &[])
            .with_effect(move |world| {
                let pressed = world
                    .create_entity()
                    .with(UiTransform::new(
                        format!("{}_button", pressed_id),
                        Anchor::TopLeft,
                        Anchor::TopLeft,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                    ))
                    .build();

                world
                    .create_entity()
                    .with(UiTransform::new(
                        format!("{}_button", other_id),
                        Anchor::TopLeft,
                        Anchor::TopLeft,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                    ))
                    .build();
                let mut channel = world.fetch_mut::<EventChannel<UiEvent>>();

                channel.single_write(UiEvent {
                    event_type: UiEventType::ClickStop,
                    target: pressed,
                });
            })
            .with_assertion(move |world| {
                let date = world.read_resource::<Date>();
                assert_eq!(paused, date.paused);
            })
            .run()
            .unwrap();
    }

    #[test_case("play", "pause" ; "play button")]
    #[test_case("pause", "play" ; "pause button")]
    fn pressing_play_or_pause_button_sets_the_other_as_above_in_ui<'a>(
        pressed: &'a str,
        other: &'a str,
    ) {
        let pressed_id = pressed.to_string();
        let other_id = other.to_string();

        AmethystApplication::ui_base::<StringBindings>()
            .with_system_desc(GameSpeedSystemDesc, "game_speed", &[])
            .with_effect(move |world| {
                let pressed = world
                    .create_entity()
                    .with(UiTransform::new(
                        format!("{}_button", pressed_id),
                        Anchor::TopLeft,
                        Anchor::TopLeft,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                    ))
                    .build();

                let other = world
                    .create_entity()
                    .with(UiTransform::new(
                        format!("{}_button", other_id),
                        Anchor::TopLeft,
                        Anchor::TopLeft,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                    ))
                    .build();

                world.insert(EffectReturn((pressed, other)));

                let mut channel = world.fetch_mut::<EventChannel<UiEvent>>();

                channel.single_write(UiEvent {
                    event_type: UiEventType::ClickStop,
                    target: pressed,
                });
            })
            .with_assertion(move |world| {
                let pressed_entity = world.read_resource::<EffectReturn<(Entity, Entity)>>().0.0.clone();
                let other_entity = world.read_resource::<EffectReturn<(Entity, Entity)>>().0.1.clone();
                let ui_transforms = world.read_storage::<UiTransform>();
            
                let pressed_transform = ui_transforms.get(pressed_entity).unwrap();
                assert_eq!(-1.0, pressed_transform.local_z);

                let other_transform = ui_transforms.get(other_entity).unwrap();
                assert_eq!(1.0, other_transform.local_z);
            })
            .run()
            .unwrap();
    }

    #[test_case("increase", 2.0, 4.0 ; "increase")]
    #[test_case("increase", 8.0, 8.0 ; "increase at maximum")]
    #[test_case("increase", 1.0, 2.0 ; "increase at minimum")]
    #[test_case("decrease", 4.0, 2.0 ; "decrease")]
    #[test_case("decrease", 8.0, 4.0 ; "decrease at maximum")]
    #[test_case("decrease", 1.0, 1.0 ; "decrease at minimum")]
    fn pressing_the_increase_or_decrease_buttons_changes_speed<'a>(
        pressed: &'a str,
        old_speed: f32,
        new_speed: f32,
    ) {
        let button_id = pressed.to_string();

        AmethystApplication::ui_base::<StringBindings>()
            .with_system_desc(GameSpeedSystemDesc, "game_speed", &[])
            .with_effect(move |world| {
                let button = world
                    .create_entity()
                    .with(UiTransform::new(
                        format!("{}_speed_button", button_id),
                        Anchor::TopLeft,
                        Anchor::TopLeft,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                    ))
                    .build();

                let mut date = world.write_resource::<Date>();
                date.current_speed = old_speed;

                let mut channel = world.fetch_mut::<EventChannel<UiEvent>>();

                channel.single_write(UiEvent {
                    event_type: UiEventType::ClickStop,
                    target: button,
                });
            })
            .with_assertion(move |world| {
                let date = world.read_resource::<Date>();
                assert_eq!(new_speed, date.current_speed);
            })
            .run()
            .unwrap();
    }
    
    #[test]
    fn notification_gets_replaced_if_time_elapsed_is_five_seconds_or_more() {
        AmethystApplication::ui_base::<StringBindings>()
            .with_system(NotificationSystem{time_passed: Some(5.0)}, "notifications", &[])
            .with_effect(move |world| {
                let font_handle = {
                    let loader = world.read_resource::<Loader>();
                    let font_storage = world.read_resource::<AssetStorage<FontAsset>>();
                    loader.load("font/square.ttf", TtfFormat, (), &font_storage)
                };

                let notification = world
                    .create_entity()
                    .with(UiText::new(
                        font_handle.clone(),
                        "a notification".to_string(),
                        [0.0, 0.0, 0.0, 1.0],
                        10.0,
                        LineMode::Single,
                        Anchor::TopLeft,
                    ))
                    .with(UiTransform::new(
                        "notification".to_string(),
                        Anchor::TopLeft,
                        Anchor::TopLeft,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                    ))
                    .build();


                world.insert(EffectReturn(notification));

            })
            .with_assertion(move |world| {
                let notification = world.read_resource::<EffectReturn<Entity>>().0.clone();
                let ui_texts = world.read_storage::<UiText>();

                let ui_text = ui_texts.get(notification).unwrap();
                assert_eq!("".to_string(), ui_text.text, "Notification");
            })
            .run()
            .unwrap();
    }

    #[test]
    fn notification_gets_replaced_immediately() {
        AmethystApplication::ui_base::<StringBindings>()
            .with_system(NotificationSystem{time_passed: Some(1.0)}, "notifications", &[])
            .with_effect(move |world| {
                let font_handle = {
                    let loader = world.read_resource::<Loader>();
                    let font_storage = world.read_resource::<AssetStorage<FontAsset>>();
                    loader.load("font/square.ttf", TtfFormat, (), &font_storage)
                };

                let notification = world
                    .create_entity()
                    .with(UiText::new(
                        font_handle.clone(),
                        "a notification".to_string(),
                        [0.0, 0.0, 0.0, 1.0],
                        10.0,
                        LineMode::Single,
                        Anchor::TopLeft,
                    ))
                    .with(UiTransform::new(
                        "notification".to_string(),
                        Anchor::TopLeft,
                        Anchor::TopLeft,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                    ))
                    .build();


                world.insert(EffectReturn(notification));

                let mut notifications = world.write_resource::<Notifications>();
                notifications.push_back("notification in queue".to_string());
            })
            .with_assertion(move |world| {
                let notification = world.read_resource::<EffectReturn<Entity>>().0.clone();
                let ui_texts = world.read_storage::<UiText>();

                let ui_text = ui_texts.get(notification).unwrap();
                assert_eq!("notification in queue".to_string(), ui_text.text, "Notification");
            })
            .run()
            .unwrap();
    }

    #[test]
    fn notification_gets_replaced_with_front_of_queue() {
        AmethystApplication::ui_base::<StringBindings>()
            .with_system(NotificationSystem{time_passed: Some(1.0)}, "notifications", &[])
            .with_effect(move |world| {
                let font_handle = {
                    let loader = world.read_resource::<Loader>();
                    let font_storage = world.read_resource::<AssetStorage<FontAsset>>();
                    loader.load("font/square.ttf", TtfFormat, (), &font_storage)
                };

                let notification = world
                    .create_entity()
                    .with(UiText::new(
                        font_handle.clone(),
                        "a notification".to_string(),
                        [0.0, 0.0, 0.0, 1.0],
                        10.0,
                        LineMode::Single,
                        Anchor::TopLeft,
                    ))
                    .with(UiTransform::new(
                        "notification".to_string(),
                        Anchor::TopLeft,
                        Anchor::TopLeft,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                    ))
                    .build();

                world.insert(EffectReturn(notification));
                
                let mut notifications = world.write_resource::<Notifications>();
                notifications.push_back("notification in front of queue".to_string());
                notifications.push_back("notification in back of queue".to_string());
            })
            .with_assertion(move |world| {
                let notification = world.read_resource::<EffectReturn<Entity>>().0.clone();
                let ui_texts = world.read_storage::<UiText>();

                let ui_text = ui_texts.get(notification).unwrap();
                assert_eq!("notification in front of queue".to_string(), ui_text.text, "Notification");
            })
            .run()
            .unwrap();
    }

    #[test]
    fn notification_gets_replaced_with_front_of_queue_if_no_existing_notification() {
        AmethystApplication::ui_base::<StringBindings>()
            .with_system(NotificationSystem{time_passed: None}, "notifications", &[])
            .with_effect(move |world| {
                let font_handle = {
                    let loader = world.read_resource::<Loader>();
                    let font_storage = world.read_resource::<AssetStorage<FontAsset>>();
                    loader.load("font/square.ttf", TtfFormat, (), &font_storage)
                };

                let notification = world
                    .create_entity()
                    .with(UiText::new(
                        font_handle.clone(),
                        "".to_string(),
                        [0.0, 0.0, 0.0, 1.0],
                        10.0,
                        LineMode::Single,
                        Anchor::TopLeft,
                    ))
                    .with(UiTransform::new(
                        "notification".to_string(),
                        Anchor::TopLeft,
                        Anchor::TopLeft,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                    ))
                    .build();

                world.insert(EffectReturn(notification));
                
                let mut notifications = world.write_resource::<Notifications>();
                notifications.push_back("notification in front of queue".to_string());
                notifications.push_back("notification in back of queue".to_string());
            })
            .with_assertion(move |world| {
                let notification = world.read_resource::<EffectReturn<Entity>>().0.clone();
                let ui_texts = world.read_storage::<UiText>();

                let ui_text = ui_texts.get(notification).unwrap();
                assert_eq!("notification in front of queue".to_string(), ui_text.text, "Notification");
            })
            .run()
            .unwrap();
    }
}
