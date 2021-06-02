use amethyst::{
    core::Transform,
    ecs::{Entities, Join, Read, ReadExpect, ReadStorage, System, Write, WriteStorage},
    input::{InputHandler, StringBindings},
    shrev::EventChannel,
    renderer::Camera,
    window::ScreenDimensions,
    winit::MouseButton,
};

use crate::{
    age_of_sail::{point_in_rect, point_mouse_to_world},
    components::{Port, Ship, Selected},
};
use crate::{components::bounding_box::BoundingBox, event::UiUpdateEvent};

#[derive(Default)]
pub struct SelectSystem {
    currently_selecting: bool,
}

impl<'s> System<'s> for SelectSystem {
    type SystemData = (
        Entities<'s>,
        ReadStorage<'s, Camera>,
        ReadStorage<'s, BoundingBox>,
        ReadStorage<'s, Transform>,
        WriteStorage<'s, Selected>,
        Read<'s, InputHandler<StringBindings>>,
        ReadExpect<'s, ScreenDimensions>,
        Write<'s, EventChannel<UiUpdateEvent>>,
    );

    fn run(
        &mut self,
        (
            entities,
            cameras,
            bounding_boxes,
            locals,
            mut selecteds,
            input,
            screen_dimensions,
            mut channel,
        ): Self::SystemData,
    ) {
        if input.mouse_button_is_down(MouseButton::Left) {
            if self.currently_selecting {
                return;
            } else {
                for (e, _) in (&entities, &selecteds).join() {
                    channel.single_write(UiUpdateEvent::Deselected(e));    
                }
                selecteds.clear();
            }
        }

        if !input.mouse_button_is_down(MouseButton::Left) {
            self.currently_selecting = false;
        }

        for (_, camera_local) in (&cameras, &locals).join() {
            for (e, bounding_box, local) in (&entities, &bounding_boxes, &locals).join() {
                if let Some((mouse_x, mouse_y)) = input.mouse_position() {
                    let (left, right, top, bottom) = bounding_box.as_boundaries(local);
                    if point_in_rect(
                        point_mouse_to_world(mouse_x, mouse_y, &*screen_dimensions, camera_local.translation()),
                        left,
                        right,
                        top,
                        bottom,
                    ) && input.mouse_button_is_down(MouseButton::Left)
                    {
                        self.currently_selecting = true;
                        selecteds.insert(e, Selected::default()).unwrap();
                    }
                }
            }
        }
    }
}

pub struct SelectPortSystem;

impl<'s> System<'s> for SelectPortSystem {
    type SystemData = (
        Entities<'s>,
        ReadStorage<'s, Port>,
        WriteStorage<'s, Selected>,
        Write<'s, EventChannel<UiUpdateEvent>>,
    );

    fn run(&mut self, (entities, ports, mut selecteds, mut update_channel): Self::SystemData) {
        let mut port_entities_selected = Vec::new();
        for (e, _, _) in (&entities, &ports, &selecteds).join() {
            update_channel.single_write(UiUpdateEvent::Target(e));
            port_entities_selected.push(e);
        }

        for e in port_entities_selected {
            selecteds.remove(e);
        }
    }
}

pub struct SelectShipSystem;

impl<'s> System<'s> for SelectShipSystem {
    type SystemData = (
        Entities<'s>,
        ReadStorage<'s, Ship>,
        ReadStorage<'s, Selected>,
        Write<'s, EventChannel<UiUpdateEvent>>,
    );

    fn run(&mut self, (entities, ships, selecteds, mut update_channel): Self::SystemData) {
        for (e, _, _) in (&entities, &ships, &selecteds).join() {
            update_channel.single_write(UiUpdateEvent::Target(e));
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use amethyst::{ecs::Entity, prelude::*, shrev::ReaderId, Result};
    use amethyst_test::prelude::*;
   
    #[test]
    fn select_port_sends_ui_update_events_for_each_selected_port() -> Result<()> {
        AmethystApplication::blank()
            .with_system(SelectPortSystem, "fulfill_contract", &[])
            .with_effect(move |world| {
                let port = world
                    .create_entity()
                    .with(Port)
                    .with(Selected)
                    .build();

                let another_port = world
                    .create_entity()
                    .with(Port)
                    .with(Selected)
                    .build();

                let reader_id = world
                    .fetch_mut::<EventChannel<UiUpdateEvent>>()
                    .register_reader();

                world.insert(EffectReturn((port, another_port)));

                world.insert(reader_id);
            })
            .with_assertion(|world| {
                let port = world.read_resource::<EffectReturn<(Entity, Entity)>>().0.0.clone();
                let another_port = world.read_resource::<EffectReturn<(Entity, Entity)>>().0.1.clone();

                let ui_update_event_channel = world.fetch_mut::<EventChannel<UiUpdateEvent>>();
                let mut reader_id = world.fetch_mut::<ReaderId<UiUpdateEvent>>();
                let mut channel_iterator = ui_update_event_channel.read(&mut reader_id);
                let update_event = channel_iterator.next().unwrap();
               
                match update_event {
                    UiUpdateEvent::Target(t) => assert_eq!(port, *t),
                    _ => panic!("Expected event to be of type `Target`"),
                }

                let update_event = channel_iterator.next().unwrap();

                match update_event {
                    UiUpdateEvent::Target(t) => assert_eq!(another_port, *t),
                    _ => panic!("Expected event to be of type `Target`"),
                }
            })
            .run()
    }
   
    #[test]
    fn select_port_does_not_consider_non_ports() -> Result<()> {
        AmethystApplication::blank()
            .with_system(SelectPortSystem, "select_port", &[])
            .with_effect(move |world| {
                world
                    .create_entity()
                    .with(Selected)
                    .build();

                world
                    .create_entity()
                    .with(Selected)
                    .build();

                let reader_id = world
                    .fetch_mut::<EventChannel<UiUpdateEvent>>()
                    .register_reader();

                world.insert(reader_id);
            })
            .with_assertion(|world| {

                let ui_update_event_channel = world.fetch_mut::<EventChannel<UiUpdateEvent>>();
                let mut reader_id = world.fetch_mut::<ReaderId<UiUpdateEvent>>();
                let channel_iterator = ui_update_event_channel.read(&mut reader_id);
                assert_eq!(0, channel_iterator.len(), "UiUpdateEvent channel length")
            })
            .run()
    }


    #[test]
    fn select_port_deselects_selected_ports() -> Result<()> {
        AmethystApplication::blank()
            .with_system(SelectPortSystem, "select_port", &[])
            .with_effect(move |world| {
                let port = world
                    .create_entity()
                    .with(Port)
                    .with(Selected)
                    .build();

                let another_port = world
                    .create_entity()
                    .with(Port)
                    .with(Selected)
                    .build();

                let reader_id = world
                    .fetch_mut::<EventChannel<UiUpdateEvent>>()
                    .register_reader();

                world.insert(EffectReturn(vec![port, another_port]));

                world.insert(reader_id);
            })
            .with_assertion(|world| {
                let ports = world.read_resource::<EffectReturn<Vec<Entity>>>().0.clone();

                let selecteds = world.read_storage::<Selected>();
                for port in ports {
                   assert!(selecteds.get(port).is_none(), "Port not selected"); 
                }
            })
            .run()
    }

    #[test]
    fn select_ship_sends_ui_update_events_for_each_selected_ship() -> Result<()> {
        AmethystApplication::blank()
            .with_system(SelectShipSystem, "fulfill_contract", &[])
            .with_effect(move |world| {
                let ship = world
                    .create_entity()
                    .with(Ship {base_speed: 5.0})
                    .with(Selected)
                    .build();

                let another_ship = world
                    .create_entity()
                    .with(Ship {base_speed: 5.0})
                    .with(Selected)
                    .build();

                let reader_id = world
                    .fetch_mut::<EventChannel<UiUpdateEvent>>()
                    .register_reader();

                world.insert(EffectReturn((ship, another_ship)));

                world.insert(reader_id);
            })
            .with_assertion(|world| {
                let ship = world.read_resource::<EffectReturn<(Entity, Entity)>>().0.0.clone();
                let another_ship = world.read_resource::<EffectReturn<(Entity, Entity)>>().0.1.clone();

                let ui_update_event_channel = world.fetch_mut::<EventChannel<UiUpdateEvent>>();
                let mut reader_id = world.fetch_mut::<ReaderId<UiUpdateEvent>>();
                let mut channel_iterator = ui_update_event_channel.read(&mut reader_id);
                let update_event = channel_iterator.next().unwrap();
               
                match update_event {
                    UiUpdateEvent::Target(t) => assert_eq!(ship, *t),
                    _ => panic!("Expected event to be of type `Target`"),
                }

                let update_event = channel_iterator.next().unwrap();

                match update_event {
                    UiUpdateEvent::Target(t) => assert_eq!(another_ship, *t),
                    _ => panic!("Expected event to be of type `Target`"),
                }
            })
            .run()
    }
   
    #[test]
    fn select_ship_does_not_consider_non_ships() -> Result<()> {
        AmethystApplication::blank()
            .with_system(SelectShipSystem, "select_ship", &[])
            .with_effect(move |world| {
                world
                    .create_entity()
                    .with(Selected)
                    .build();

                world
                    .create_entity()
                    .with(Selected)
                    .build();

                let reader_id = world
                    .fetch_mut::<EventChannel<UiUpdateEvent>>()
                    .register_reader();

                world.insert(reader_id);
            })
            .with_assertion(|world| {

                let ui_update_event_channel = world.fetch_mut::<EventChannel<UiUpdateEvent>>();
                let mut reader_id = world.fetch_mut::<ReaderId<UiUpdateEvent>>();
                let channel_iterator = ui_update_event_channel.read(&mut reader_id);
                assert_eq!(0, channel_iterator.len(), "UiUpdateEvent channel length")
            })
            .run()
    }
}
