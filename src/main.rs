use amethyst::{
    core::transform::TransformBundle,
    input::{InputBundle, StringBindings},
    prelude::*,
    renderer::{
        plugins::{RenderFlat2D, RenderToWindow},
        types::DefaultBackend,
        RenderingBundle,
    },
    ui::{RenderUi, UiBundle},
    utils::application_root_dir,
};
use systems::{
    AiSystem, ChaseSystem, CollisionSystem, DestroySystemDesc, DockingSystem, ExpirationSystem,
    ExpireContractSystem, FulfillContractSystem, GameSpeedSystemDesc, NotificationSystem,
    PatrolSystem, PlayerStatusSystemDesc, PlotCourseSystem, PortPanelSystemDesc, SelectPortSystem,
    UpdateTimeSystem,
};

mod age_of_sail;
mod components;
mod event;
mod systems;

use crate::age_of_sail::MainState;
use crate::systems::{AcceptContractSystemDesc, MoveShipsSystem, SelectSystem};

fn main() -> amethyst::Result<()> {
    amethyst::start_logger(Default::default());

    let app_root = application_root_dir()?;

    let resources = app_root.join("assets");
    let display_config = app_root.join("config/display_config.ron");
    let key_bindings_path = app_root.join("config/input.ron");

    let game_data = GameDataBuilder::default()
        .with_bundle(TransformBundle::new())?
        .with_bundle(
            InputBundle::<StringBindings>::new().with_bindings_from_file(&key_bindings_path)?,
        )?
        .with_bundle(UiBundle::<StringBindings>::new())?
        .with_bundle(
            RenderingBundle::<DefaultBackend>::new()
                .with_plugin(
                    RenderToWindow::from_config_path(display_config)?
                        .with_clear([0.0, 0.0, 0.5, 1.0]),
                )
                .with_plugin(RenderUi::default())
                .with_plugin(RenderFlat2D::default()),
        )?
        .with(UpdateTimeSystem, "time", &[])
        .with(ExpirationSystem, "expiration", &[])
        .with(ExpireContractSystem, "expired_contract", &[])
        .with(AiSystem, "ai", &[])
        .with(PatrolSystem, "patrol", &[])
        .with(ChaseSystem, "chase", &[])
        .with(MoveShipsSystem, "move_ships", &[])
        .with(PlotCourseSystem, "plot_course", &[])
        .with(DockingSystem, "docking", &[])
        .with(SelectSystem::default(), "select", &[])
        .with(SelectPortSystem, "select_port", &[])
        .with(CollisionSystem, "collision", &[])
        .with_system_desc(PlayerStatusSystemDesc::default(), "ui_player_status", &[])
        .with_system_desc(GameSpeedSystemDesc::default(), "ui_game_speed", &[])
        .with(NotificationSystem::default(), "ui_notification_system", &[])
        .with_system_desc(AcceptContractSystemDesc::default(), "accept_contract", &[])
        .with_system_desc(DestroySystemDesc::default(), "destroy", &[])
        .with(FulfillContractSystem, "fulfill_contract", &[])
        .with_thread_local_desc(PortPanelSystemDesc::default());

    let mut game = Application::new(resources, MainState, game_data)?;
    game.run();

    Ok(())
}
