use std::{
    collections::{HashMap, VecDeque},
    ops::Add,
};

use crate::components::{
    Action, Affiliation, Ai, AiState, BoundingBox, Cargo, Contract, Controllable, Expiration,
    ItemType, OwnedBy, Patrol, Pirate, Port, Ship, StateQuery,
};
use amethyst::{
    assets::{AssetLoaderSystemData, AssetStorage, Handle, Loader},
    core::{
        math::{Point2, Point3, Vector3},
        transform::Transform,
        WithNamed,
    },
    ecs::Join,
    prelude::*,
    renderer::{
        palette::LinSrgba,
        rendy::{
            mesh::{Position, TexCoord},
            texture::palette::load_from_linear_rgba,
        },
        visibility::BoundingSphere,
        Camera, ImageFormat, Material, MaterialDefaults, Mesh, SpriteRender, SpriteSheet,
        SpriteSheetFormat, Texture,
    },
    ui::{FontAsset, TtfFormat, UiCreator},
    window::ScreenDimensions,
};
use chrono::{Duration, TimeZone, Utc};
use rand::{seq::SliceRandom, thread_rng, Rng};
use std::iter;

pub const WORLD_WIDTH: f32 = 400.0;
pub const WORLD_HEIGHT: f32 = 300.0;
pub const DISTANCE_THRESHOLD: f32 = 0.15;

pub type Notifications = VecDeque<String>;

pub struct MainState;

impl SimpleState for MainState {
    fn on_start(&mut self, data: StateData<'_, GameData<'_, '_>>) {
        let world = data.world;

        world.exec(|mut creator: UiCreator<'_>| {
            creator.create("ui/main.ron", ());
        });

        let font_handle = {
            let loader = world.read_resource::<Loader>();
            let font_storage = world.read_resource::<AssetStorage<FontAsset>>();
            loader.load("font/square.ttf", TtfFormat, (), &font_storage)
        };

        let texture_handle = {
            let loader = world.read_resource::<Loader>();
            let texture_storage = world.read_resource::<AssetStorage<Texture>>();
            loader.load(
                "texture/panel.png",
                ImageFormat::default(),
                (),
                &texture_storage,
            )
        };

        world.insert(UiAssets {
            font: font_handle,
            panel: texture_handle,
        });

        world.insert(Date::default());

        initialise_map(world);
        initialise_ports(world);
        initialise_contracts(world);
        initialise_player(world);
        initialise_pirates(world);
        initialise_camera(world);
    }
}

pub struct UiAssets {
    pub font: Handle<FontAsset>,
    pub panel: Handle<Texture>,
}

fn initialise_player(world: &mut World) -> () {
    world.insert(PlayerStatus { money: 0 });

    let sprite_render = SpriteRender::new(load_sprite_sheet(world), 1);

    let mut transform = Transform::default();
    transform.set_translation_xyz(200.0, 150.0, 0.0);

    world
        .create_entity()
        .with(Ship { base_speed: 10.0 })
        .named("Dolphin")
        .with(Affiliation {
            name: "You".to_string(),
        })
        .with(Controllable)
        .with(Cargo::default())
        .with(sprite_render.clone())
        .with(transform)
        .with(BoundingBox {
            width: 8.0,
            origin: Point2::new(0.0, 0.0),
        })
        .build();
}

fn initialise_pirates(world: &mut World) {
    let sprite_render = SpriteRender::new(load_sprite_sheet(world), 2);

    let mut transform = Transform::default();
    transform.set_translation_xyz(300.0, 180.0, 0.0);

    let chase_distance = 30;

    world.register::<Pirate>();

    world
        .create_entity()
        .with(Ship { base_speed: 9.0 })
        .named("Queen Anne's Revenge")
        .with(Affiliation {
            name: "Pirates".to_string(),
        })
        .with(Pirate)
        .with(Ai {
            states: vec![
                AiState {
                    transitions: [
                        (StateQuery::TargetNotNearby(chase_distance), 0),
                        (StateQuery::TargetNearby(chase_distance), 1),
                    ]
                    .iter()
                    .cloned()
                    .collect(),
                    action: Action::Patrol,
                },
                AiState {
                    transitions: [
                        (StateQuery::TargetNearby(chase_distance), 1),
                        (StateQuery::TargetNotNearby(chase_distance), 0),
                    ]
                    .iter()
                    .cloned()
                    .collect(),
                    action: Action::Chase,
                },
            ],
            current_state_index: 0,
            previous_state_index: 0,
        })
        .with(Patrol {
            waypoints: vec![Point2::new(250.0, 190.0), Point2::new(280.0, 160.0)],
            next_waypoint_index: 0,
        })
        .with(Cargo::default())
        .with(sprite_render.clone())
        .with(transform)
        .with(BoundingBox {
            width: 8.0,
            origin: Point2::new(0.0, 0.0),
        })
        .build();
}

fn initialise_ports(world: &mut World) {
    let sprite_render = SpriteRender::new(load_sprite_sheet(world), 0);

    let mut transform = Transform::default();
    transform.set_translation_xyz(120.0, 50.0, 0.0);

    world
        .create_entity()
        .with(Port)
        .named("Portsmouth")
        .with(Cargo::default())
        .with(sprite_render.clone())
        .with(transform)
        .with(BoundingBox {
            width: 10.0,
            origin: Point2::new(0.0, 0.0),
        })
        .build();

    let mut transform = Transform::default();
    transform.set_translation_xyz(275.0, 110.0, 0.0);
    world
        .create_entity()
        .with(Port)
        .named("London")
        .with(Cargo::default())
        .with(sprite_render.clone())
        .with(transform)
        .with(BoundingBox {
            width: 10.0,
            origin: Point2::new(0.0, 0.0),
        })
        .build();

    let mut transform = Transform::default();
    transform.set_translation_xyz(150.0, 275.0, 0.0);
    world
        .create_entity()
        .with(Port)
        .named("Liverpool")
        .with(Cargo::default())
        .with(sprite_render.clone())
        .with(transform)
        .with(BoundingBox {
            width: 10.0,
            origin: Point2::new(0.0, 0.0),
        })
        .build();
}

fn initialise_contracts(world: &mut World) {
    let port_entities = {
        let entities = world.entities();
        let ports = world.read_component::<Port>();
        (&entities, &ports)
            .join()
            .map(|(e, _)| e)
            .collect::<Vec<_>>()
    };
    let mut rng = thread_rng();

    for p in &port_entities {
        let number_of_initial_contracts = rng.gen_range(1..4);

        for _ in 0..number_of_initial_contracts {
            let mut goods_required = HashMap::new();

            let number_of_items = rng.gen_range(1..4);

            for _ in 0..number_of_items {
                let item_type = ItemType::choose();
                let amount = rng.gen_range(1..11);
                *goods_required.entry(item_type).or_insert(0) += amount;
            }

            let destination = loop {
                let choice = port_entities.choose(&mut rng).unwrap();
                if choice != p {
                    break *choice;
                }
            };

            let contract = world
                .create_entity()
                .with(Contract::new(
                    rng.gen_range(10..100) * 10,
                    destination,
                    goods_required,
                ))
                .with(OwnedBy { entity: *p });

            if rng.gen_bool(0.3) {
                let days_ahead = rng.gen_range(5..20);
                let expiration_date = Utc.ymd(1680, 1, 1).add(Duration::days(days_ahead));
                contract
                    .with(Expiration {
                        expiration_date,
                        expired: false,
                    })
                    .build();
            } else {
                contract.build();
            }
        }
    }
}

fn initialise_camera(world: &mut World) {
    let mut transform = Transform::default();
    transform.set_translation_xyz(WORLD_WIDTH * 0.5, WORLD_HEIGHT * 0.5, 10.0);

    world
        .create_entity()
        .with(Camera::standard_2d(WORLD_WIDTH, WORLD_HEIGHT))
        .with(transform)
        .build();
}

fn initialise_map(world: &mut World) {
    let map = Map {
        islands: vec![vec![
            Point2::new(150.0, 200.0),
            Point2::new(150.0, 150.0),
            Point2::new(180.0, 150.0),
            Point2::new(180.0, 150.0),
            Point2::new(150.0, 150.0),
            Point2::new(175.0, 100.0),
        ]],
    };
    println!("{}", map.on_land(Point2::new(160.0, 175.0)));

    let map_vertices = map.into_vertices();
    let num_map_vertices = map_vertices.len();

    let mesh = world.exec(|loader: AssetLoaderSystemData<Mesh>| {
        loader.load_from_data(
            amethyst::renderer::types::MeshData(
                (
                    map_vertices,
                    iter::repeat(TexCoord([0.0, 0.0]))
                        .take(num_map_vertices)
                        .collect::<Vec<_>>(),
                )
                    .into(),
            ),
            (),
        )
    });

    let default_mat = world.read_resource::<MaterialDefaults>().0.clone();

    let albedo = world.exec(|loader: AssetLoaderSystemData<Texture>| {
        loader.load_from_data(
            load_from_linear_rgba(LinSrgba::new(0.14, 0.6, 0.2, 1.0)).into(),
            (),
        )
    });

    let mat = world.exec(|loader: AssetLoaderSystemData<Material>| {
        loader.load_from_data(
            Material {
                albedo,
                ..default_mat.clone()
            },
            (),
        )
    });

    let transform = Transform::default();

    world
        .create_entity()
        .with(mat)
        .with(mesh)
        .with(BoundingSphere {
            center: Point3::origin(),
            radius: 1000.0,
        })
        .with(transform)
        .build();

    world.insert(map);
}

fn load_sprite_sheet(world: &mut World) -> Handle<SpriteSheet> {
    let texture_handle = {
        let loader = world.read_resource::<Loader>();
        let texture_storage = world.read_resource::<AssetStorage<Texture>>();
        loader.load(
            "sprite/port_spritesheet.png",
            ImageFormat::default(),
            (),
            &texture_storage,
        )
    };

    let loader = world.read_resource::<Loader>();
    let sprite_sheet_store = world.read_resource::<AssetStorage<SpriteSheet>>();
    loader.load(
        "sprite/port_spritesheet.ron",
        SpriteSheetFormat(texture_handle),
        (),
        &sprite_sheet_store,
    )
}

pub fn point_in_rect(point: Point2<f32>, left: f32, right: f32, top: f32, bottom: f32) -> bool {
    point.x >= left && point.x <= right && point.y <= top && point.y >= bottom
}

pub fn point_mouse_to_world(
    mouse_x: f32,
    mouse_y: f32,
    screen_dimensions: &ScreenDimensions,
    camera_position: &Vector3<f32>,
) -> Point2<f32> {
    Point2::new(
        WORLD_WIDTH * mouse_x / screen_dimensions.width() + (camera_position.x - WORLD_WIDTH / 2.0),
        WORLD_HEIGHT - WORLD_HEIGHT * mouse_y / screen_dimensions.height()
            + (camera_position.y - WORLD_HEIGHT / 2.0),
    )
}

#[derive(Default)]
pub struct Map {
    pub islands: Vec<Vec<Point2<f32>>>,
}

impl Map {
    fn into_vertices(&self) -> Vec<Position> {
        self.islands
            .iter()
            .flat_map(|island| island.iter().map(|&p| Position([p.x, p.y, 0.0])).clone())
            .collect::<Vec<_>>()
    }

    fn on_land(&self, point: Point2<f32>) -> bool {
        self.islands.iter().any(|island| {
            island.chunks(3).any(|triangle| {
                let a = triangle[0];
                let b = triangle[1];
                let c = triangle[2];
                let a_b = b - a;
                let b_c = c - b;
                let c_a = a - c;
                let a_p = point - a;
                let b_p = point - b;
                let c_p = point - c;

                let a_cross = a_b.x * a_p.y - a_b.y * a_p.x;
                let b_cross = b_c.x * b_p.y - b_c.y * b_p.x;
                let c_cross = c_a.x * c_p.y - c_a.y * c_p.x;

                a_cross.signum() == b_cross.signum() && a_cross.signum() == c_cross.signum()
            })
        })
    }
}

#[derive(Default)]
pub struct PlayerStatus {
    pub money: i32,
}

pub struct Date {
    pub time_elapsed: f64,
    pub current_speed: f32,
    pub paused: bool,
}

impl Default for Date {
    fn default() -> Self {
        Date {
            time_elapsed: 0.,
            current_speed: 1.,
            paused: false,
        }
    }
}

impl Date {
    pub fn current_date_string(&self) -> String {
        self.current_date().format("%e %B %Y").to_string()
    }

    pub fn current_date(&self) -> chrono::Date<Utc> {
        let utc = Utc.ymd(1680, 1, 1);

        utc.add(Duration::seconds(self.time_elapsed as i64))
    }

    pub fn game_speed(&self) -> f32 {
        if self.paused {
            0.
        } else {
            self.current_speed
        }
    }
}
