use bevy::prelude::*;
use crate::app::AppState;

// --- markers ---
#[derive(Component)] struct MenuRoot;
#[derive(Component)] struct PlayButton;
#[derive(Component)] struct ModePanel;      // container that holds the two mode buttons
#[derive(Component)] struct SingleButton;
#[derive(Component)] struct MultiButton;
#[derive(Component)] struct Disabled;       // simple "disabled" flag

pub struct MenuPlugin;
impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Menu), setup_menu)
            .add_systems(Update, button_logic.run_if(in_state(AppState::Menu)))
            .add_systems(OnExit(AppState::Menu), cleanup_menu);
    }
}

fn setup_menu(mut commands: Commands, assets: Res<AssetServer>) {
    // UI camera (remove if you already spawn one elsewhere for the menu)
    commands.spawn(Camera2d);

    let font: Handle<Font> = assets.load("fonts/arial.ttf");

    // Root
    commands
        .spawn((
            MenuRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(24.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.08, 0.10)),
        ))
        .with_children(|ui| {
            // Title
            ui.spawn((
                Text::new("PROJECT RTS"),
                TextFont { font: font.clone(), font_size: 56.0, ..default() },
                TextColor(Color::WHITE),
            ));

            // Play button (first screen)
            ui.spawn((
                PlayButton,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(28.0), Val::Px(16.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.95, 0.82, 0.10)),
                BorderColor::all(Color::BLACK),
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new("Play"),
                    TextFont { font: font.clone(), font_size: 26.0, ..default() },
                    TextColor(Color::BLACK),
                ));
            });

            // Modes panel (hidden until Play is pressed)
            ui.spawn((
                ModePanel,
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(16.0),
                    ..default()
                },
                Visibility::Hidden, // <-- show this after Play
            ))
            .with_children(|panel| {
                // Singleplayer (enabled)
                panel
                    .spawn((
                        SingleButton,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(24.0), Val::Px(14.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.95, 0.82, 0.10)),
                        BorderColor::all(Color::BLACK),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("Singleplayer"),
                            TextFont { font: font.clone(), font_size: 24.0, ..default() },
                            TextColor(Color::BLACK),
                        ));
                    });

                // Multiplayer (disabled + slightly transparent)
                panel
                    .spawn((
                        MultiButton,
                        Disabled, // mark as not clickable
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(24.0), Val::Px(14.0)),
                            ..default()
                        },
                        // semi-transparent mustard
                        BackgroundColor(Color::srgba(0.95, 0.82, 0.10, 0.45)),
                        BorderColor::all(Color::BLACK),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("Multiplayer (coming soon)"),
                            TextFont { font, font_size: 24.0, ..default() },
                            // dim the label too
                            TextColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                        ));
                    });
            });
        });
}

fn button_logic(
    mut next: ResMut<NextState<AppState>>,
    mut play_vis: Query<&mut Visibility, With<PlayButton>>,
    mut panel_vis: Query<&mut Visibility, (With<ModePanel>, Without<PlayButton>)>,
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor, Option<&Disabled>, Option<&SingleButton>, Entity),
        (Changed<Interaction>, With<Button>)
    >,
) {
    for (interaction, mut bg, disabled, is_single, entity) in &mut buttons {
        // hover tint for enabled buttons
        match (*interaction, disabled.is_some()) {
            (Interaction::Hovered, false) => *bg = BackgroundColor(Color::srgb(1.0, 0.9, 0.2)),
            (Interaction::None,    false) => *bg = BackgroundColor(Color::srgb(0.95, 0.82, 0.10)),
            _ => {}
        }

        if *interaction == Interaction::Pressed {
            if disabled.is_some() { continue; } // ignore disabled

            // Play button → reveal panel, hide Play
            if play_vis.get_mut(entity).is_ok() {
                if let Ok(mut v_panel) = panel_vis.single_mut() { *v_panel = Visibility::Visible; }
                if let Ok(mut v_play)  = play_vis.single_mut()  { *v_play  = Visibility::Hidden;  }
                continue;
            }

            // Singleplayer → enter game
            if is_single.is_some() {
                next.set(AppState::InGame);
            }
        }
    }
}

fn cleanup_menu(mut commands: Commands, q: Query<Entity, With<MenuRoot>>, cams: Query<Entity, With<Camera>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
    for cam in &cams {
        commands.entity(cam).despawn();
    }
}