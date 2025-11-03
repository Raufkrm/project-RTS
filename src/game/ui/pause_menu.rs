use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::app::AppState;
use crate::game::ui::settings_menu::{SettingsMenuCommand, SettingsMenuState};
use crate::game::world::planet::{PlanetDebugConfig, PlanetDebugMode};

#[derive(Message)]
pub enum PauseMenuCommand {
    Show,
    Hide,
    Toggle,
}

#[derive(Resource, Default)]
pub struct PauseMenuState {
    pub visible: bool,
}

#[derive(Component)]
struct PauseMenuRoot;

#[derive(Component)]
struct PauseMenuButton;

#[derive(Component)]
struct SettingsButton;

#[derive(Component)]
struct LeaveGameButton;

#[derive(Component)]
struct ExitButton;

#[derive(Component)]
struct DisableDebugButton;

pub struct PauseMenuPlugin;
impl Plugin for PauseMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PauseMenuCommand>()
            .init_resource::<PauseMenuState>()
            .add_systems(OnEnter(AppState::InGame), spawn_pause_menu)
            .add_systems(OnExit(AppState::InGame), cleanup_pause_menu)
            .add_systems(
                Update,
                (
                    toggle_pause_menu,
                    pause_menu_button_logic,
                    handle_pause_menu_commands,
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

fn spawn_pause_menu(mut commands: Commands, assets: Res<AssetServer>) {
    let font: Handle<Font> = assets.load("fonts/arial.ttf");

    commands
        .spawn((
            PauseMenuRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                position_type: PositionType::Absolute,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            Visibility::Hidden,
            FocusPolicy::Block,
            ZIndex(100),
            Name::new("PauseMenu"),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(320.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Stretch,
                    row_gap: Val::Px(14.0),
                    padding: UiRect::all(Val::Px(20.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.08, 0.12, 0.92)),
                BorderColor::all(Color::srgba(0.35, 0.35, 0.45, 1.0)),
                Name::new("PauseMenuPanel"),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("PAUSED"),
                    TextFont {
                        font: font.clone(),
                        font_size: 28.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 1.0, 1.0)),
                ));

                panel
                    .spawn((
                        PauseMenuButton,
                        SettingsButton,
                        Button,
                        Node {
                            height: Val::Px(44.0),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.95, 0.82, 0.10)),
                        BorderColor::all(Color::srgb(0.05, 0.05, 0.05)),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("Settings"),
                            TextFont {
                                font: font.clone(),
                                font_size: 20.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.08, 0.08, 0.1)),
                        ));
                    });

                panel
                    .spawn((
                        PauseMenuButton,
                        LeaveGameButton,
                        Button,
                        Node {
                            height: Val::Px(44.0),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.95, 0.82, 0.10)),
                        BorderColor::all(Color::srgb(0.05, 0.05, 0.05)),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("Leave Game"),
                            TextFont {
                                font: font.clone(),
                                font_size: 20.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.08, 0.08, 0.1)),
                        ));
                    });

                panel
                    .spawn((
                        PauseMenuButton,
                        ExitButton,
                        Button,
                        Node {
                            height: Val::Px(44.0),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.95, 0.82, 0.10)),
                        BorderColor::all(Color::srgb(0.05, 0.05, 0.05)),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("Exit to OS"),
                            TextFont {
                                font: font.clone(),
                                font_size: 20.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.08, 0.08, 0.1)),
                        ));
                    });

                panel
                    .spawn((
                        PauseMenuButton,
                        DisableDebugButton,
                        Button,
                        Node {
                            height: Val::Px(44.0),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.95, 0.82, 0.10)),
                        BorderColor::all(Color::srgb(0.05, 0.05, 0.05)),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("Disable Debug"),
                            TextFont {
                                font,
                                font_size: 20.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.08, 0.08, 0.1)),
                        ));
                    });
            });
        });
}

fn cleanup_pause_menu(
    mut commands: Commands,
    mut state: ResMut<PauseMenuState>,
    roots: Query<Entity, With<PauseMenuRoot>>,
    children: Query<&Children>,
) {
    state.visible = false;
    for entity in &roots {
        despawn_recursive(&mut commands, entity, &children);
    }
}

fn toggle_pause_menu(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<PauseMenuState>,
    mut menu_root: Query<&mut Visibility, With<PauseMenuRoot>>,
    settings_state: Option<Res<SettingsMenuState>>,
    mut settings_cmd: MessageWriter<SettingsMenuCommand>,
) {
    let open_request = !state.visible && keys.just_pressed(KeyCode::Escape);
    let close_request = state.visible && keys.just_pressed(KeyCode::Escape);

    if let Some(settings_state) = settings_state {
        if settings_state.visible && keys.just_pressed(KeyCode::Escape) {
            settings_cmd.write(SettingsMenuCommand::Hide);
        }
    }

    if open_request || close_request {
        let new_visibility = !state.visible;
        set_menu_visibility(&mut state, &mut menu_root, new_visibility);
    }
}

#[allow(clippy::too_many_arguments)]
fn pause_menu_button_logic(
    mut state: ResMut<PauseMenuState>,
    mut menu_root: Query<&mut Visibility, With<PauseMenuRoot>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut exit_writer: MessageWriter<AppExit>,
    mut debug_config: ResMut<PlanetDebugConfig>,
    mut settings_cmd: MessageWriter<SettingsMenuCommand>,
    mut buttons: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            Option<&SettingsButton>,
            Option<&LeaveGameButton>,
            Option<&ExitButton>,
            Option<&DisableDebugButton>,
        ),
        (Changed<Interaction>, With<Button>, With<PauseMenuButton>),
    >,
) {
    for (interaction, mut bg, is_settings, is_leave, is_exit, is_disable_debug) in &mut buttons {
        if *interaction == Interaction::Pressed {
            if is_settings.is_some() {
                settings_cmd.write(SettingsMenuCommand::Show);
                set_menu_visibility(&mut state, &mut menu_root, false);
            } else if is_leave.is_some() {
                set_menu_visibility(&mut state, &mut menu_root, false);
                next_state.set(AppState::Menu);
            } else if is_exit.is_some() {
                exit_writer.write(AppExit::default());
            } else if is_disable_debug.is_some() {
                if debug_config.mode != PlanetDebugMode::None {
                    debug_config.mode = PlanetDebugMode::None;
                }
                set_menu_visibility(&mut state, &mut menu_root, false);
            }
        }

        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(Color::srgb(0.95, 0.8, 0.25));
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(Color::srgb(0.95, 0.82, 0.35));
            }
            Interaction::None => {
                *bg = BackgroundColor(Color::srgb(0.95, 0.82, 0.10));
            }
        }
    }
}

fn set_menu_visibility(
    state: &mut PauseMenuState,
    menu_root: &mut Query<&mut Visibility, With<PauseMenuRoot>>,
    visible: bool,
) {
    if state.visible == visible {
        return;
    }
    state.visible = visible;
    if let Ok(mut visibility) = menu_root.single_mut() {
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn despawn_recursive(commands: &mut Commands, entity: Entity, children_q: &Query<&Children>) {
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            despawn_recursive(commands, child, children_q);
        }
    }
    commands.entity(entity).despawn();
}

fn handle_pause_menu_commands(
    mut events: MessageReader<PauseMenuCommand>,
    mut state: ResMut<PauseMenuState>,
    mut menu_root: Query<&mut Visibility, With<PauseMenuRoot>>,
) {
    for command in events.read() {
        match command {
            PauseMenuCommand::Show => set_menu_visibility(&mut state, &mut menu_root, true),
            PauseMenuCommand::Hide => set_menu_visibility(&mut state, &mut menu_root, false),
            PauseMenuCommand::Toggle => {
                let target = !state.visible;
                set_menu_visibility(&mut state, &mut menu_root, target);
            }
        }
    }
}

pub fn pause_menu_hidden(state: Option<Res<PauseMenuState>>) -> bool {
    state.map_or(true, |s| !s.visible)
}

pub fn pause_menu_visible(state: Option<Res<PauseMenuState>>) -> bool {
    state.map_or(false, |s| s.visible)
}
