use bevy::prelude::*;

use crate::conf::ui::ui_colors;
use crate::game_ui::login::LoginPhase;
use crate::game_ui::{
    DialogButton, DialogButtonId, DialogButtonPressed, GameUiAssets, ModalDialog, ModalOrder,
};
use crate::network::version::{CLIENT_VERSION, VersionCheck};

#[derive(Component)]
pub(super) struct UpdateRequiredDialog;

pub(super) fn route_to_update_required(
    check: Res<VersionCheck>,
    phase: Res<State<LoginPhase>>,
    mut commands: Commands,
) {
    if matches!(*check, VersionCheck::Outdated { .. }) && *phase.get() != LoginPhase::UpdateRequired
    {
        commands.set_state(LoginPhase::UpdateRequired);
    }
}

pub(super) fn spawn_update_required(
    mut commands: Commands,
    ui_assets: Res<GameUiAssets>,
    mut order: ResMut<ModalOrder>,
    check: Res<VersionCheck>,
) {
    let VersionCheck::Outdated {
        required,
        download_url,
    } = &*check
    else {
        return;
    };
    let handle = ModalDialog::new("Update Required")
        .with_buttons([DialogButton::custom("download", "Open download page")])
        .spawn(&mut commands, &ui_assets, &mut order);
    commands.entity(handle.root).insert(UpdateRequiredDialog);
    commands.entity(handle.content).with_child((
        Text::new(format!(
            "Rustibia {required} is required; this is {CLIENT_VERSION}.\nDownload it from {download_url}"
        )),
        TextFont {
            font: ui_assets.font.clone(),
            font_size: 11.0,
            ..default()
        },
        TextColor(ui_colors::FONT_COLOR_CONTENT.into()),
    ));
}

pub(super) fn despawn_update_required(
    mut commands: Commands,
    roots: Query<Entity, With<UpdateRequiredDialog>>,
) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

pub(super) fn on_update_required_button(
    event: On<DialogButtonPressed>,
    dialogs: Query<(), With<UpdateRequiredDialog>>,
    check: Res<VersionCheck>,
) {
    if dialogs.get(event.dialog).is_err() || event.button != DialogButtonId::Custom("download") {
        return;
    }
    let VersionCheck::Outdated { download_url, .. } = &*check else {
        return;
    };
    if let Err(e) = webbrowser::open(download_url) {
        error!("could not open {download_url} in a browser: {e}");
    }
}
