use amethyst::{
    core::{alga::linear::EuclideanSpace, math::Point2, Transform},
    ecs::{Entities, Join, Read, ReadStorage, System, SystemData, Write, WriteStorage},
    prelude::SystemDesc,
    shred::World,
    shrev::{EventChannel, ReaderId},
    ui::{UiEvent, UiEventType},
};

use crate::{
    age_of_sail::{Notifications, PlayerStatus, DISTANCE_THRESHOLD},
    components::{Cargo, Contract, OwnedBy, Ship},
    event::UiUpdateEvent,
};

pub struct AcceptContractSystem {
    reader_id: ReaderId<UiEvent>,
}

impl AcceptContractSystem {
    fn new(reader_id: ReaderId<UiEvent>) -> Self {
        AcceptContractSystem { reader_id }
    }
}

impl<'s> System<'s> for AcceptContractSystem {
    type SystemData = (
        ReadStorage<'s, Contract>,
        WriteStorage<'s, OwnedBy>,
        WriteStorage<'s, Cargo>,
        Read<'s, EventChannel<UiEvent>>,
        Write<'s, EventChannel<UiUpdateEvent>>,
    );

    fn run(
        &mut self,
        (contracts, mut owned_bys, mut cargos, channel, mut update_channel): Self::SystemData,
    ) {
        for event in channel.read(&mut self.reader_id) {
            let target = match event.event_type {
                UiEventType::ClickStop => Some(event.target),
                _ => None,
            };

            if let Some(clicked) = target {
                if let Some(associated_entity) =
                    owned_bys.get(clicked).map_or(None, |o| Some(o.entity))
                {
                    if let Some(contract) = contracts.get(associated_entity) {
                        let port_cargo = cargos
                            .get_mut(owned_bys.get(associated_entity).unwrap().entity)
                            .unwrap();

                        owned_bys.remove(associated_entity);

                        for (item_type, amount) in &contract.goods_required {
                            *port_cargo.items.entry(*item_type).or_insert(0) += amount;
                        }
                        update_channel.single_write(UiUpdateEvent::Target(associated_entity));
                    }
                }
            }
        }
    }
}

pub struct AcceptContractSystemDesc;

impl Default for AcceptContractSystemDesc {
    fn default() -> Self {
        AcceptContractSystemDesc {}
    }
}

impl<'a, 'b> SystemDesc<'a, 'b, AcceptContractSystem> for AcceptContractSystemDesc {
    fn build(self, world: &mut World) -> AcceptContractSystem {
        <AcceptContractSystem as System<'_>>::SystemData::setup(world);

        let reader_id = world.fetch_mut::<EventChannel<UiEvent>>().register_reader();

        AcceptContractSystem::new(reader_id)
    }
}

pub struct FulfillContractSystem;

impl<'s> System<'s> for FulfillContractSystem {
    type SystemData = (
        Entities<'s>,
        ReadStorage<'s, Contract>,
        ReadStorage<'s, Ship>,
        ReadStorage<'s, OwnedBy>,
        ReadStorage<'s, Transform>,
        WriteStorage<'s, Cargo>,
        Write<'s, Notifications>,
        Write<'s, PlayerStatus>,
        Write<'s, EventChannel<UiUpdateEvent>>,
    );

    fn run(
        &mut self,
        (
            entities,
            contracts,
            ships,
            owned_bys,
            locals,
            mut cargos,
            mut notifications,
            mut player_status,
            mut channel,
        ): Self::SystemData,
    ) {
        // for each active contract (not owned by a port)
        for (e, contract, _) in (&entities, &contracts, !&owned_bys).join() {
            let port_transform = locals.get(contract.destination).unwrap();
            let port_location = Point2::new(
                port_transform.translation().x,
                port_transform.translation().y,
            );

            // If a ship is nearby and has items in cargo, contract is fulfilled
            //TODO: consider event for docking in the future rather than distance
            let suitable_ship = (&entities, &ships, &cargos, &locals)
                .join()
                .filter(|(_, _, cargo, l)| {
                    let ship_location = Point2::new(l.translation().x, l.translation().y);

                    let ship_has_cargo = contract
                        .goods_required
                        .iter()
                        .map(|(item, amount)| {
                            cargo.items.get(item).unwrap_or(&(0 as u32)) >= amount
                        })
                        .fold(true, |a, b| a && b);

                    ship_location.distance(&port_location) < DISTANCE_THRESHOLD && ship_has_cargo
                })
                .map(|(e, _, _, _)| e)
                .next();

            if let Some(ship) = suitable_ship {
                let cargo = cargos.get_mut(ship).unwrap();
                for (item, amount) in &contract.goods_required {
                    *cargo.items.get_mut(&item).unwrap() -= amount;
                }

                player_status.money += contract.payment as i32;
                channel.single_write(UiUpdateEvent::PlayerStatus);
                entities.delete(e).unwrap();
                notifications.push_back(format!("Completed contract for £{}", contract.payment));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Cargo, Contract, ItemType, OwnedBy};
    use amethyst::{ecs::Entity, prelude::*, Result};
    use amethyst_test::prelude::*;
    use std::collections::HashMap;

    #[test]
    fn accepting_contract_sends_ui_update_event() -> Result<()> {
        AmethystApplication::blank()
            .with_system_desc(AcceptContractSystemDesc, "accept_contract", &[])
            .with_effect(|world| {
                let port = world
                    .create_entity()
                    .with(Cargo {
                        items: HashMap::new(),
                    })
                    .build();
                let destination = world.create_entity().build();

                let contract = world
                    .create_entity()
                    .with(Contract {
                        payment: 0,
                        destination: destination,
                        goods_required: [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
                            .iter()
                            .cloned()
                            .collect(),
                    })
                    .with(OwnedBy { entity: port })
                    .build();

                world.insert(EffectReturn(contract));

                let ui_entity = world
                    .create_entity()
                    .with(OwnedBy { entity: contract })
                    .build();

                let reader_id = world
                    .fetch_mut::<EventChannel<UiUpdateEvent>>()
                    .register_reader();

                world.insert(reader_id);

                let mut channel = world.fetch_mut::<EventChannel<UiEvent>>();
                channel.single_write(UiEvent {
                    event_type: UiEventType::ClickStop,
                    target: ui_entity,
                });
            })
            .with_assertion(|world| {
                let contract_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();

                let ui_update_event_channel = world.fetch_mut::<EventChannel<UiUpdateEvent>>();
                let mut reader_id = world.fetch_mut::<ReaderId<UiUpdateEvent>>();
                let update_event = ui_update_event_channel.read(&mut reader_id).next().unwrap();
                match update_event {
                    UiUpdateEvent::Target(t) => assert_eq!(contract_entity, *t),
                    _ => panic!("Expected event to be of type `Target`"),
                }
            })
            .run()
    }

    #[test]
    fn accepted_contract_no_longer_owned_by_port() -> Result<()> {
        AmethystApplication::blank()
            .with_system_desc(AcceptContractSystemDesc, "accept_contract", &[])
            .with_effect(|world| {
                let port = world
                    .create_entity()
                    .with(Cargo {
                        items: HashMap::new(),
                    })
                    .build();
                let destination = world.create_entity().build();

                let contract = world
                    .create_entity()
                    .with(Contract {
                        payment: 0,
                        destination: destination,
                        goods_required: [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
                            .iter()
                            .cloned()
                            .collect(),
                    })
                    .with(OwnedBy { entity: port })
                    .build();

                world.insert(EffectReturn(contract));

                let ui_entity = world
                    .create_entity()
                    .with(OwnedBy { entity: contract })
                    .build();

                let reader_id = world
                    .fetch_mut::<EventChannel<UiUpdateEvent>>()
                    .register_reader();

                world.insert(reader_id);

                let mut channel = world.fetch_mut::<EventChannel<UiEvent>>();
                channel.single_write(UiEvent {
                    event_type: UiEventType::ClickStop,
                    target: ui_entity,
                });
            })
            .with_assertion(|world| {
                let contract_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();
                let owned_bys = world.read_storage::<OwnedBy>();
                assert!(owned_bys.get(contract_entity).is_none());
            })
            .run()
    }

    #[test]
    fn accepted_contract_goods_in_port_cargo() -> Result<()> {
        let goods_required: HashMap<ItemType, u32> = [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
            .iter()
            .cloned()
            .collect();

        let goods_required_2 = goods_required.clone();

        AmethystApplication::blank()
            .with_system_desc(AcceptContractSystemDesc, "accept_contract", &[])
            .with_effect(move |world| {
                let port = world
                    .create_entity()
                    .with(Cargo {
                        items: HashMap::new(),
                    })
                    .build();

                world.insert(EffectReturn(port));

                let destination = world.create_entity().build();

                let contract = world
                    .create_entity()
                    .with(Contract {
                        payment: 0,
                        destination: destination,
                        goods_required: goods_required,
                    })
                    .with(OwnedBy { entity: port })
                    .build();

                let ui_entity = world
                    .create_entity()
                    .with(OwnedBy { entity: contract })
                    .build();

                let reader_id = world
                    .fetch_mut::<EventChannel<UiUpdateEvent>>()
                    .register_reader();

                world.insert(reader_id);

                let mut channel = world.fetch_mut::<EventChannel<UiEvent>>();
                channel.single_write(UiEvent {
                    event_type: UiEventType::ClickStop,
                    target: ui_entity,
                });
            })
            .with_assertion(move |world| {
                let port_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();
                let cargos = world.read_storage::<Cargo>();
                let port_cargo = cargos.get(port_entity).unwrap();

                for (k, v) in goods_required_2.iter() {
                    assert!(
                        port_cargo.items.contains_key(&k),
                        format!("Cargo should contain item type {}", k)
                    );
                    assert_eq!(
                        *v,
                        port_cargo.items[&k],
                        "{}",
                        format!("Number of {} in cargo", k)
                    );
                }
            })
            .run()
    }

    #[test]
    fn fulfilling_contract_sends_ui_update_event_for_player_status() -> Result<()> {
        let goods_required: HashMap<ItemType, u32> = [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
            .iter()
            .cloned()
            .collect();

        AmethystApplication::blank()
            .with_system(FulfillContractSystem, "fulfill_contract", &[])
            .with_effect(move |world| {
                let port = world
                    .create_entity()
                    .with(Cargo {
                        items: HashMap::new(),
                    })
                    .with(Transform::default())
                    .build();

                world
                    .create_entity()
                    .with(Ship { base_speed: 1.0 })
                    .with(Cargo {
                        items: goods_required.clone(),
                    })
                    .with(Transform::default())
                    .build();

                world
                    .create_entity()
                    .with(Contract {
                        payment: 0,
                        destination: port,
                        goods_required: goods_required.clone(),
                    })
                    .build();

                let reader_id = world
                    .fetch_mut::<EventChannel<UiUpdateEvent>>()
                    .register_reader();

                world.insert(reader_id);
            })
            .with_assertion(|world| {
                let ui_update_event_channel = world.fetch_mut::<EventChannel<UiUpdateEvent>>();
                let mut reader_id = world.fetch_mut::<ReaderId<UiUpdateEvent>>();
                let update_event = ui_update_event_channel.read(&mut reader_id).next().unwrap();
                assert_eq!(UiUpdateEvent::PlayerStatus, *update_event);
            })
            .run()
    }

    #[test]
    fn fulfilling_contract_updates_player_status() -> Result<()> {
        const PAYMENT: u32 = 30;

        let goods_required: HashMap<ItemType, u32> = [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
            .iter()
            .cloned()
            .collect();

        AmethystApplication::blank()
            .with_system(FulfillContractSystem, "fulfill_contract", &[])
            .with_effect(move |world| {
                let port = world
                    .create_entity()
                    .with(Cargo {
                        items: HashMap::new(),
                    })
                    .with(Transform::default())
                    .build();

                world
                    .create_entity()
                    .with(Ship { base_speed: 1.0 })
                    .with(Cargo {
                        items: goods_required.clone(),
                    })
                    .with(Transform::default())
                    .build();

                world
                    .create_entity()
                    .with(Contract {
                        payment: PAYMENT,
                        destination: port,
                        goods_required: goods_required.clone(),
                    })
                    .build();
            })
            .with_assertion(|world| {
                let player_status = world.fetch::<PlayerStatus>();
                assert_eq!(PAYMENT as i32, player_status.money);
            })
            .run()
    }

    #[test]
    fn fulfilling_contract_transfers_cargo_from_ship_to_port() -> Result<()> {
        let goods_required: HashMap<ItemType, u32> = [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
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
        for (k, v) in goods_required.iter() {
            *expected_goods_on_ship.get_mut(k).unwrap() -= v;
        }

        AmethystApplication::blank()
            .with_system(FulfillContractSystem, "fulfill_contract", &[])
            .with_effect(move |world| {
                let port = world
                    .create_entity()
                    .with(Cargo {
                        items: HashMap::new(),
                    })
                    .with(Transform::default())
                    .build();

                let ship = world
                    .create_entity()
                    .with(Ship { base_speed: 1.0 })
                    .with(Cargo {
                        items: original_goods_on_ship.clone(),
                    })
                    .with(Transform::default())
                    .build();

                world
                    .create_entity()
                    .with(Contract {
                        payment: 0,
                        destination: port,
                        goods_required: goods_required.clone(),
                    })
                    .build();

                world.insert(EffectReturn(ship));
            })
            .with_assertion(move |world| {
                let ship_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();
                let cargos = world.read_storage::<Cargo>();
                let ship_cargo = cargos.get(ship_entity).unwrap();

                for (k, v) in expected_goods_on_ship.iter() {
                    assert!(
                        ship_cargo.items.contains_key(&k),
                        format!("Cargo should contain item type {}", k)
                    );
                    assert_eq!(
                        *v,
                        ship_cargo.items[&k],
                        "{}",
                        format!("Number of {} in cargo", k)
                    );
                }
            })
            .run()
    }

    #[test]
    fn fulfilling_sends_notification() -> Result<()> {
        const PAYMENT: u32 = 100;
        let goods_required: HashMap<ItemType, u32> = [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
            .iter()
            .cloned()
            .collect();

        AmethystApplication::blank()
            .with_system(FulfillContractSystem, "fulfill_contract", &[])
            .with_effect(move |world| {
                let port = world
                    .create_entity()
                    .with(Cargo {
                        items: HashMap::new(),
                    })
                    .with(Transform::default())
                    .build();

                world
                    .create_entity()
                    .with(Ship { base_speed: 1.0 })
                    .with(Cargo {
                        items: goods_required.clone(),
                    })
                    .with(Transform::default())
                    .build();

                world
                    .create_entity()
                    .with(Contract {
                        payment: PAYMENT,
                        destination: port,
                        goods_required: goods_required.clone(),
                    })
                    .build();
            })
            .with_assertion(move |world| {
                let notifications = world.read_resource::<Notifications>().clone();
                assert_eq!(1, notifications.len(), "Number of notifications");
                assert_eq!(
                    &format!("Completed contract for £{}", PAYMENT),
                    notifications.front().unwrap(),
                    "Notification"
                );
            })
            .run()
    }

    #[test]
    fn contract_not_fulfilled_if_owned_by_something() -> Result<()> {
        const PAYMENT: u32 = 30;
        const ORIGINAL_MONEY: i32 = 10;

        let goods_required: HashMap<ItemType, u32> = [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
            .iter()
            .cloned()
            .collect();

        AmethystApplication::blank()
            .with_system(FulfillContractSystem, "fulfill_contract", &[])
            .with_effect(move |world| {
                let port = world
                    .create_entity()
                    .with(Cargo {
                        items: HashMap::new(),
                    })
                    .with(Transform::default())
                    .build();

                world
                    .create_entity()
                    .with(Ship { base_speed: 1.0 })
                    .with(Cargo {
                        items: goods_required.clone(),
                    })
                    .with(Transform::default())
                    .build();

                world.insert(PlayerStatus {
                    money: ORIGINAL_MONEY,
                });

                let entity = world.create_entity().build();

                world
                    .create_entity()
                    .with(Contract {
                        payment: PAYMENT,
                        destination: port,
                        goods_required: goods_required.clone(),
                    })
                    .with(OwnedBy { entity })
                    .build();
            })
            .with_assertion(|world| {
                let player_status = world.fetch::<PlayerStatus>();
                assert_eq!(ORIGINAL_MONEY, player_status.money);
            })
            .run()
    }

    #[test]
    fn contract_not_fulfilled_if_ship_does_not_have_required_goods() -> Result<()> {
        const PAYMENT: u32 = 30;
        const ORIGINAL_MONEY: i32 = 10;

        let goods_required: HashMap<ItemType, u32> = [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
            .iter()
            .cloned()
            .collect();

        AmethystApplication::blank()
            .with_system(FulfillContractSystem, "fulfill_contract", &[])
            .with_effect(move |world| {
                let port = world
                    .create_entity()
                    .with(Cargo {
                        items: HashMap::new(),
                    })
                    .with(Transform::default())
                    .build();

                world
                    .create_entity()
                    .with(Ship { base_speed: 1.0 })
                    .with(Cargo {
                        items: [(ItemType::Sugar, 10)].iter().cloned().collect(),
                    })
                    .with(Transform::default())
                    .build();

                world.insert(PlayerStatus {
                    money: ORIGINAL_MONEY,
                });

                world
                    .create_entity()
                    .with(Contract {
                        payment: PAYMENT,
                        destination: port,
                        goods_required: goods_required.clone(),
                    })
                    .build();
            })
            .with_assertion(|world| {
                let player_status = world.fetch::<PlayerStatus>();
                assert_eq!(ORIGINAL_MONEY, player_status.money);
            })
            .run()
    }

    #[test]
    fn contract_not_fulfilled_if_no_ship_nearby() -> Result<()> {
        const PAYMENT: u32 = 30;
        const ORIGINAL_MONEY: i32 = 10;

        let goods_required: HashMap<ItemType, u32> = [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
            .iter()
            .cloned()
            .collect();

        AmethystApplication::blank()
            .with_system(FulfillContractSystem, "fulfill_contract", &[])
            .with_effect(move |world| {
                let port = world
                    .create_entity()
                    .with(Cargo {
                        items: HashMap::new(),
                    })
                    .with(Transform::default())
                    .build();

                let mut ship_transform = Transform::default();
                ship_transform.set_translation_xyz(100.0, 0.0, 0.0);

                world
                    .create_entity()
                    .with(Ship { base_speed: 1.0 })
                    .with(Cargo {
                        items: goods_required.clone(),
                    })
                    .with(ship_transform)
                    .build();

                world.insert(PlayerStatus {
                    money: ORIGINAL_MONEY,
                });

                world
                    .create_entity()
                    .with(Contract {
                        payment: PAYMENT,
                        destination: port,
                        goods_required: goods_required.clone(),
                    })
                    .build();
            })
            .with_assertion(|world| {
                let player_status = world.fetch::<PlayerStatus>();
                assert_eq!(ORIGINAL_MONEY, player_status.money);
            })
            .run()
    }
}
