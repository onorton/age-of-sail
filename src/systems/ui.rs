use ::amethyst::core::SystemDesc;
use amethyst::{
    core::{Parent, RunNowDesc},
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
    age_of_sail::{Date, PlayerStatus, UiAssets},
    components::{Contract, OwnedBy},
};
use crate::{components::Port, event::UiUpdateEvent};

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
        ReadExpect<'s, PlayerStatus>,
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
