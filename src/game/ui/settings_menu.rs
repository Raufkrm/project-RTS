use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::app::AppState;
use crate::game::ui::pause_menu::PauseMenuCommand;

#[derive(Message)]
pub enum SettingsMenuCommand {
    Show,
    Hide,
    Toggle,
}

#[derive(Resource, Debug)]
pub struct SettingsMenuState {
    pub visible: bool,
    pub active_tab: SettingsTab,
}

impl Default for SettingsMenuState {
    fn default() -> Self {
        Self {
            visible: false,
            active_tab: SettingsTab::Graphics,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsTab {
    Graphics,
    Controls,
    Sound,
}

#[derive(Component)]
struct SettingsMenuRoot;

#[derive(Component)]
struct SettingsTabButton {
    tab: SettingsTab,
}

#[derive(Component)]
struct SettingsContentText;

#[derive(Component)]
struct CloseSettingsButton;

#[derive(Component)]
struct SettingsTipOne;

#[derive(Component)]
struct SettingsTipTwo;

pub struct SettingsMenuPlugin;
impl Plugin for SettingsMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SettingsMenuCommand>()
            .init_resource::<SettingsMenuState>()
            .add_systems(OnEnter(AppState::InGame), spawn_settings_menu)
            .add_systems(OnExit(AppState::InGame), cleanup_settings_menu)
            .add_systems(
                Update,
                (
                    handle_settings_commands,
                    settings_menu_keyboard,
                    tab_button_logic,
                    close_button_logic,
                    update_tab_visuals,
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

fn spawn_settings_menu(mut commands: Commands, assets: Res<AssetServer>) {
    let font: Handle<Font> = assets.load("fonts/arial.ttf");

    commands
        .spawn((
            SettingsMenuRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                position_type: PositionType::Absolute,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
            Visibility::Hidden,
            FocusPolicy::Block,
            ZIndex(120),
            Name::new("SettingsMenu"),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(620.0),
                    height: Val::Px(520.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Stretch,
                    row_gap: Val::Px(18.0),
                    padding: UiRect::all(Val::Px(24.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.09, 0.09, 0.14, 0.96)),
                BorderColor::all(Color::srgba(0.35, 0.35, 0.45, 1.0)),
                Name::new("SettingsMenuPanel"),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("SETTINGS"),
                    TextFont {
                        font: font.clone(),
                        font_size: 30.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 1.0, 1.0)),
                ));

                panel
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(12.0),
                            ..default()
                        },
                        Name::new("SettingsTabs"),
                    ))
                    .with_children(|tabs| {
                        tabs.spawn((
                            SettingsTabButton {
                                tab: SettingsTab::Graphics,
                            },
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(20.0), Val::Px(12.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.35, 0.38, 0.50)),
                            BorderColor::all(Color::srgb(0.05, 0.05, 0.05)),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new("Graphics"),
                                TextFont {
                                    font: font.clone(),
                                    font_size: 18.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.08, 0.08, 0.1)),
                            ));
                        });

                        tabs.spawn((
                            SettingsTabButton {
                                tab: SettingsTab::Controls,
                            },
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(20.0), Val::Px(12.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.35, 0.38, 0.50)),
                            BorderColor::all(Color::srgb(0.05, 0.05, 0.05)),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new("Controls"),
                                TextFont {
                                    font: font.clone(),
                                    font_size: 18.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.08, 0.08, 0.1)),
                            ));
                        });

                        tabs.spawn((
                            SettingsTabButton {
                                tab: SettingsTab::Sound,
                            },
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(20.0), Val::Px(12.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.35, 0.38, 0.50)),
                            BorderColor::all(Color::srgb(0.05, 0.05, 0.05)),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new("Sound"),
                                TextFont {
                                    font: font.clone(),
                                    font_size: 18.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.08, 0.08, 0.1)),
                            ));
                        });
                    });

                panel.spawn((
                    Node {
                        height: Val::Px(2.0),
                        width: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.3, 0.3, 0.45, 0.6)),
                ));

                panel
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(18.0),
                            flex_grow: 1.0,
                            padding: UiRect::all(Val::Px(8.0)),
                            ..default()
                        },
                        Name::new("SettingsContent"),
                    ))
                    .with_children(|content| {
                        content.spawn((
                            SettingsContentText,
                            Text::new("Adjust visual quality, resolution, and rendering features here."),
                            TextFont {
                                font: font.clone(),
                                font_size: 18.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.9, 0.9, 0.95)),
                        ));
                        content.spawn((
                            SettingsTipOne,
                            Text::new("• Placeholder slider: Resolution scale"),
                            TextFont {
                                font: font.clone(),
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.75, 0.8, 0.95)),
                        ));
                        content.spawn((
                            SettingsTipTwo,
                            Text::new("• Placeholder toggle: VSync"),
                            TextFont {
                                font: font.clone(),
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.75, 0.8, 0.95)),
                        ));
                    });

                panel.spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::FlexEnd,
                        ..default()
                    },
                    Name::new("SettingsFooter"),
                ))
                .with_children(|footer| {
                    footer
                        .spawn((
                            CloseSettingsButton,
                            Button,
                            Node {
                                padding: UiRect::new(
                                    Val::Px(18.0),
                                    Val::Px(18.0),
                                    Val::Px(10.0),
                                    Val::Px(10.0),
                                ),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.95, 0.82, 0.10)),
                            BorderColor::all(Color::srgb(0.05, 0.05, 0.05)),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new("Back"),
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
        });
}

fn cleanup_settings_menu(
    mut commands: Commands,
    mut state: ResMut<SettingsMenuState>,
    roots: Query<Entity, With<SettingsMenuRoot>>,
    children: Query<&Children>,
) {
    state.visible = false;
    state.active_tab = SettingsTab::Graphics;
    for entity in &roots {
        despawn_recursive(&mut commands, entity, &children);
    }
}

fn settings_menu_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<SettingsMenuState>,
    mut menu_root: Query<&mut Visibility, With<SettingsMenuRoot>>,
    mut pause_cmd: MessageWriter<PauseMenuCommand>,
) {
    if !state.visible {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        set_settings_menu_visibility(&mut state, &mut menu_root, false);
        pause_cmd.write(PauseMenuCommand::Show);
    }
}

fn handle_settings_commands(
    mut events: MessageReader<SettingsMenuCommand>,
    mut state: ResMut<SettingsMenuState>,
    mut menu_root: Query<&mut Visibility, With<SettingsMenuRoot>>,
    mut pause_cmd: MessageWriter<PauseMenuCommand>,
) {
    for command in events.read() {
        match command {
            SettingsMenuCommand::Show => {
                set_settings_menu_visibility(&mut state, &mut menu_root, true);
                pause_cmd.write(PauseMenuCommand::Hide);
            }
            SettingsMenuCommand::Hide => {
                set_settings_menu_visibility(&mut state, &mut menu_root, false);
            }
            SettingsMenuCommand::Toggle => {
                let target = !state.visible;
                set_settings_menu_visibility(&mut state, &mut menu_root, target);
                if target {
                    pause_cmd.write(PauseMenuCommand::Hide);
                }
            }
        }
    }
}

fn tab_button_logic(
    mut state: ResMut<SettingsMenuState>,
    mut buttons: Query<(&SettingsTabButton, &Interaction), (Changed<Interaction>, With<Button>)>,
) {
    if !state.visible {
        return;
    }

    for (button, interaction) in &mut buttons {
        if *interaction == Interaction::Pressed {
            state.active_tab = button.tab;
        }
    }
}

fn close_button_logic(
    mut state: ResMut<SettingsMenuState>,
    mut menu_root: Query<&mut Visibility, With<SettingsMenuRoot>>,
    mut pause_cmd: MessageWriter<PauseMenuCommand>,
    mut buttons: Query<&Interaction, (Changed<Interaction>, With<CloseSettingsButton>)>,
) {
    if !state.visible {
        return;
    }
    for interaction in &mut buttons {
        if *interaction == Interaction::Pressed {
            set_settings_menu_visibility(&mut state, &mut menu_root, false);
            pause_cmd.write(PauseMenuCommand::Show);
        }
    }
}

fn update_tab_visuals(
    state: Res<SettingsMenuState>,
    mut buttons: Query<(&SettingsTabButton, &mut BackgroundColor)>,
    mut text_sets: ParamSet<(
        Query<&mut Text, With<SettingsContentText>>,
        Query<&mut Text, With<SettingsTipOne>>,
        Query<&mut Text, With<SettingsTipTwo>>,
    )>,
) {
    if !state.is_changed() {
        return;
    }

    let selected_color = BackgroundColor(Color::srgb(0.95, 0.82, 0.20));
    let normal_color = BackgroundColor(Color::srgb(0.35, 0.38, 0.50));

    for (button, mut bg) in &mut buttons {
        *bg = if button.tab == state.active_tab {
            selected_color
        } else {
            normal_color
        };
    }

    if let Some(mut text) = text_sets.p0().iter_mut().next() {
        text.0 = match state.active_tab {
            SettingsTab::Graphics => "Adjust visual quality, resolution, and rendering features here.".to_string(),
            SettingsTab::Controls => "Configure key bindings, mouse sensitivity, and accessibility shortcuts here.".to_string(),
            SettingsTab::Sound => "Set master volume, music balance, and sound effect levels here.".to_string(),
        };
    }

    if let Some(mut text) = text_sets.p1().iter_mut().next() {
        text.0 = match state.active_tab {
            SettingsTab::Graphics => "• Placeholder slider: Resolution scale".to_string(),
            SettingsTab::Controls => "• Placeholder slider: Mouse sensitivity".to_string(),
            SettingsTab::Sound => "• Placeholder slider: Master volume".to_string(),
        };
    }

    if let Some(mut text) = text_sets.p2().iter_mut().next() {
        text.0 = match state.active_tab {
            SettingsTab::Graphics => "• Placeholder toggle: VSync".to_string(),
            SettingsTab::Controls => "• Placeholder button: Rebind keys".to_string(),
            SettingsTab::Sound => "• Placeholder button: Mute music".to_string(),
        };
    }
}

fn set_settings_menu_visibility(
    state: &mut SettingsMenuState,
    menu_root: &mut Query<&mut Visibility, With<SettingsMenuRoot>>,
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

fn despawn_recursive(
    commands: &mut Commands,
    entity: Entity,
    children_q: &Query<&Children>,
) {
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            despawn_recursive(commands, child, children_q);
        }
    }
    commands.entity(entity).despawn();
}

pub fn settings_menu_hidden(state: Option<Res<SettingsMenuState>>) -> bool {
    state.map_or(true, |s| !s.visible)
}

pub fn settings_menu_visible(state: Option<Res<SettingsMenuState>>) -> bool {
    state.map_or(false, |s| s.visible)
}
