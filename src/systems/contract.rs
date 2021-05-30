use crate::{
    age_of_sail::{Notifications, PlayerStatus, DISTANCE_THRESHOLD},
    components::{Cargo, Contract, Expiration, OwnedBy, Ship},
    event::UiUpdateEvent,
};
use amethyst::{
    core::{alga::linear::EuclideanSpace, math::Point2, Named, Transform},
    ecs::{Entities, Join, Read, ReadStorage, System, SystemData, Write, WriteStorage},
    prelude::SystemDesc,
    shred::World,
    shrev::{EventChannel, ReaderId},
    ui::{UiEvent, UiEventType},
};
use itertools::Itertools;
use std::collections::HashSet;

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
        ReadStorage<'s, Expiration>,
        ReadStorage<'s, Named>,
        WriteStorage<'s, OwnedBy>,
        WriteStorage<'s, Cargo>,
        Read<'s, EventChannel<UiEvent>>,
        Write<'s, EventChannel<UiUpdateEvent>>,
        Write<'s, Notifications>,
    );

    fn run(
        &mut self,
        (
            contracts,
            expirations,
            names,
            mut owned_bys,
            mut cargos,
            channel,
            mut update_channel,
            mut notifications,
        ): Self::SystemData,
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
                        let port = owned_bys.get(associated_entity).unwrap().entity;
                        let port_cargo = cargos.get_mut(port).unwrap();

                        owned_bys.remove(associated_entity);

                        for (item_type, amount) in &contract.goods_required {
                            *port_cargo.items.entry(*item_type).or_insert(0) += amount;
                        }

                        let items_notification = contract
                            .goods_required
                            .iter()
                            .sorted_by_key(|(&item, _)| item)
                            .map(|(item, amount)| format!("{} tons of {}", amount, item))
                            .collect::<Vec<_>>()
                            .join(", ");

                        let contract_accepted_message = format!(
                            "Contract accepted. {} ready to be loaded at {}.",
                            items_notification,
                            names.get(port).unwrap().name.to_string(),
                        );

                        if let Some(expiration) = expirations.get(associated_entity) {
                            notifications.push_back(format!(
                                "{} It will expire on {}.",
                                contract_accepted_message,
                                expiration.expiration_date.format("%e %B %Y").to_string()
                            ));
                        } else {
                            notifications.push_back(contract_accepted_message);
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
        ReadStorage<'s, Named>,
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
            names,
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
                let items_notification = contract
                    .goods_required
                    .iter()
                    .sorted_by_key(|(&item, _)| item)
                    .map(|(item, amount)| format!("{} tons of {}", amount, item))
                    .collect::<Vec<_>>()
                    .join(", ");

                for (item, amount) in &contract.goods_required {
                    *cargo.items.get_mut(&item).unwrap() -= amount;
                }

                player_status.money += contract.payment as i32;
                channel.single_write(UiUpdateEvent::PlayerStatus);
                channel.single_write(UiUpdateEvent::Target(e));

                entities.delete(e).unwrap();
                notifications.push_back(format!(
                    "Completed contract for £{} at {}. {} removed from cargo.",
                    contract.payment,
                    names.get(contract.destination).unwrap().name.to_string(),
                    items_notification
                ));
            }
        }
    }
}

pub struct ExpireContractSystem;

impl<'s> System<'s> for ExpireContractSystem {
    type SystemData = (
        Entities<'s>,
        ReadStorage<'s, Contract>,
        ReadStorage<'s, OwnedBy>,
        ReadStorage<'s, Expiration>,
        Write<'s, Notifications>,
        Write<'s, EventChannel<UiUpdateEvent>>,
    );

    fn run(
        &mut self,
        (
            entities,
            contracts,
            owned_bys,
            expirations,
            mut notifications,
            mut channel,
        ): Self::SystemData,
    ) {
        let mut contracts_to_destroy = HashSet::new();

        for (e, _, expiration) in (&entities, &contracts, &expirations).join() {
            if expiration.expired {
                contracts_to_destroy.insert(e);
                match owned_bys.get(e) {
                    Some(_) => channel.single_write(UiUpdateEvent::Target(e)),
                    None => notifications
                        .push_back("A contract you have accepted has expired".to_string()),
                }
            }
        }

        for e in contracts_to_destroy {
            entities.delete(e).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Cargo, Contract, ItemType, OwnedBy};
    use amethyst::{core::WithNamed, ecs::Entity, prelude::*, Result};
    use amethyst_test::prelude::*;
    use chrono::{TimeZone, Utc};
    use std::collections::HashMap;

    #[test]
    fn accepting_contract_sends_ui_update_event() -> Result<()> {
        AmethystApplication::blank()
            .with_system_desc(AcceptContractSystemDesc, "accept_contract", &[])
            .with_effect(|world| {
                let port = world
                    .create_entity()
                    .named("Portsmouth")
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
                    .named("Portsmouth")
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
                    .named("Portsmouth")
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
    fn accepted_contract_sends_notification() -> Result<()> {
        const PORT: &str = "Portsmouth";

        let goods_required: HashMap<ItemType, u32> = [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
            .iter()
            .cloned()
            .collect();

        AmethystApplication::blank()
            .with_system_desc(AcceptContractSystemDesc, "accept_contract", &[])
            .with_effect(move |world| {
                let port = world
                    .create_entity()
                    .named(PORT)
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
                let notifications = world.read_resource::<Notifications>().clone();
                assert_eq!(1, notifications.len(), "Number of notifications");
                assert_eq!(
                    &format!(
                        "Contract accepted. 5 tons of Rum, 10 tons of Sugar ready to be loaded at {}.",
                        PORT
                    ),
                    notifications.front().unwrap(),
                    "Notification"
                );
            })
            .run()
    }

    #[test]
    fn accepted_contract_expiration_sends_notification() -> Result<()> {
        const PORT: &str = "Portsmouth";
        let expiration_date = Utc.ymd(1680, 1, 1);
        let expiration_date_cloned = expiration_date.clone();

        let goods_required: HashMap<ItemType, u32> = [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
            .iter()
            .cloned()
            .collect();

        AmethystApplication::blank()
            .with_system_desc(AcceptContractSystemDesc, "accept_contract", &[])
            .with_effect(move |world| {
                let port = world
                    .create_entity()
                    .named(PORT)
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
                    .with(Expiration { expired: false, expiration_date: expiration_date})
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
                let notifications = world.read_resource::<Notifications>().clone();
                assert_eq!(1, notifications.len(), "Number of notifications");
                assert_eq!(
                    &format!(
                        "Contract accepted. 5 tons of Rum, 10 tons of Sugar ready to be loaded at {}. It will expire on {}.",
                        PORT,
                        expiration_date_cloned.format("%e %B %Y")
                    ),
                    notifications.front().unwrap(),
                    "Notification"
                );
            })
            .run()
    }

    #[test]
    fn fulfilling_contract_sends_ui_update_event_for_contract() -> Result<()> {
        let goods_required: HashMap<ItemType, u32> = [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
            .iter()
            .cloned()
            .collect();

        AmethystApplication::blank()
            .with_system(FulfillContractSystem, "fulfill_contract", &[])
            .with_effect(move |world| {
                let port = world
                    .create_entity()
                    .named("Portsmouth")
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

                let contract = world
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

                world.insert(EffectReturn(contract));
            })
            .with_assertion(|world| {
                let contract_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();

                let ui_update_event_channel = world.fetch_mut::<EventChannel<UiUpdateEvent>>();
                let mut reader_id = world.fetch_mut::<ReaderId<UiUpdateEvent>>();
                let update_event = ui_update_event_channel.read(&mut reader_id).last().unwrap();
                assert_eq!(UiUpdateEvent::Target(contract_entity), *update_event);
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
                    .named("Portsmouth")
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
                    .named("Portsmouth")
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
                    .named("Portsmouth")
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
    fn fulfilling_contract_sends_notification() -> Result<()> {
        const PAYMENT: u32 = 100;
        const PORT: &str = "London";
        let goods_required: HashMap<ItemType, u32> = [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
            .iter()
            .cloned()
            .collect();

        AmethystApplication::blank()
            .with_system(FulfillContractSystem, "fulfill_contract", &[])
            .with_effect(move |world| {
                let port = world
                    .create_entity()
                    .named(PORT)
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
                    &format!(
                        "Completed contract for £{} at {}. 5 tons of Rum, 10 tons of Sugar removed from cargo.",
                        PAYMENT, PORT
                    ),
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

    #[test]
    fn expired_contract_is_deleted() -> Result<()> {
        AmethystApplication::blank()
            .with_system(ExpireContractSystem, "expire_contract", &[])
            .with_effect(|world| {
                let destination = world.create_entity().build();
                let contract = world
                    .create_entity()
                    .with(Contract {
                        payment: 0,
                        destination,
                        goods_required: [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
                            .iter()
                            .cloned()
                            .collect(),
                    })
                    .with(Expiration {
                        expiration_date: Utc.ymd(1680, 1, 1),
                        expired: true,
                    })
                    .build();

                world.insert(EffectReturn(contract));
            })
            .with_assertion(|world| {
                world.maintain();
                let contract_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();
                assert!(!world.entities().is_alive(contract_entity));
            })
            .run()
    }

    #[test]
    fn non_expired_contract_not_deleted() -> Result<()> {
        AmethystApplication::blank()
            .with_system(ExpireContractSystem, "expire_contract", &[])
            .with_effect(|world| {
                let destination = world.create_entity().build();
                let contract = world
                    .create_entity()
                    .with(Contract {
                        payment: 0,
                        destination,
                        goods_required: [(ItemType::Sugar, 10), (ItemType::Rum, 5)]
                            .iter()
                            .cloned()
                            .collect(),
                    })
                    .with(Expiration {
                        expiration_date: Utc.ymd(1680, 1, 1),
                        expired: false,
                    })
                    .build();

                world.insert(EffectReturn(contract));
            })
            .with_assertion(|world| {
                world.maintain();
                let contract_entity = world.read_resource::<EffectReturn<Entity>>().0.clone();
                assert!(world.entities().is_alive(contract_entity));
            })
            .run()
    }

    #[test]
    fn expired_contract_in_port_sends_ui_update_event() -> Result<()> {
        AmethystApplication::blank()
            .with_system(ExpireContractSystem, "expire_contract", &[])
            .with_effect(|world| {
                let port = world.create_entity().build();
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
                    .with(Expiration {
                        expiration_date: Utc.ymd(1680, 1, 1),
                        expired: true,
                    })
                    .build();

                world.insert(EffectReturn(contract));

                let reader_id = world
                    .fetch_mut::<EventChannel<UiUpdateEvent>>()
                    .register_reader();

                world.insert(reader_id);
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
    fn player_expired_contract_sends_notification() -> Result<()> {
        AmethystApplication::blank()
            .with_system(ExpireContractSystem, "expire_contract", &[])
            .with_effect(|world| {
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
                    .with(Expiration {
                        expiration_date: Utc.ymd(1680, 1, 1),
                        expired: true,
                    })
                    .build();

                world.insert(EffectReturn(contract));

                let reader_id = world
                    .fetch_mut::<EventChannel<UiUpdateEvent>>()
                    .register_reader();

                world.insert(reader_id);
            })
            .with_assertion(|world| {
                let notifications = world.read_resource::<Notifications>().clone();
                assert_eq!(1, notifications.len(), "Number of notifications");
                let notification = notifications.front().unwrap();
                assert_eq!(
                    "A contract you have accepted has expired", notification,
                    "Notification message"
                );
            })
            .run()
    }
}
