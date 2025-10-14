use bevy::prelude::*;
use crate::app::AppState;

const BG: Color = Color::srgb(0.06, 0.07, 0.10);
const BTN: Color = Color::srgb(0.16, 0.18, 0.26);
const BTN_HOVER: Color = Color::srgb(0.22, 0.24, 0.34);
const BTN_PRESSED: Color = Color::srgb(0.10, 0.50, 0.32);
const TXT: Color = Color::srgb(0.95, 0.96, 0.98);

#[derive(Component)] struct MenuRoot;
#[derive(Component)] enum ButtonAction { Start, Quit }

pub struct MenuPlugin;
impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Menu), setup_menu)
           .add_systems(Update, button_interactions.run_if(in_state(AppState::Menu)))
           .add_systems(OnExit(AppState::Menu), cleanup_menu);
    }
}

fn setup_menu(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);

    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(BG),
        MenuRoot,
    ))
    .with_children(|root| {
        root.spawn(Node {
            width: Val::Px(380.0),
            row_gap: Val::Px(16.0),
            padding: UiRect::all(Val::Px(24.0)),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                Text::new("PROJECT RTS"),
                TextFont { font: assets.load("fonts/arial.ttf"), font_size: 38.0, ..default() },
                TextColor(TXT),
            ));

            col.spawn((
                Button,
                Node {
                    width: Val::Px(240.0),
                    height: Val::Px(56.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(BTN),
                BorderRadius::all(Val::Px(10.0)),
                ButtonAction::Start,
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new("Start Game"),
                    TextFont { font: assets.load("fonts/arial.ttf"), font_size: 24.0, ..default() },
                    TextColor(TXT),
                ));
            });

            #[cfg(not(target_arch = "wasm32"))]
            col.spawn((
                Button,
                Node {
                    width: Val::Px(240.0),
                    height: Val::Px(56.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(BTN),
                BorderRadius::all(Val::Px(10.0)),
                ButtonAction::Quit,
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new("Quit"),
                    TextFont { font: assets.load("fonts/arial.ttf"), font_size: 24.0, ..default() },
                    TextColor(TXT),
                ));
            });
        });
    });
}

fn button_interactions(
    mut q: Query<(&Interaction, &mut BackgroundColor, &ButtonAction), (Changed<Interaction>, With<Button>)>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (interaction, mut bg, action) in &mut q {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(BTN_PRESSED);
                match action {
                    ButtonAction::Start => next_state.set(AppState::InGame),
                    ButtonAction::Quit => {
                        #[cfg(not(target_arch = "wasm32"))]
                        std::process::exit(0);
                    }
                }
            }
            Interaction::Hovered => *bg = BackgroundColor(BTN_HOVER),
            Interaction::None => *bg = BackgroundColor(BTN),
        }
    }
}

fn cleanup_menu(
    mut commands: Commands,
    roots: Query<Entity, With<MenuRoot>>,
    children_q: Query<&Children>,
) {
    for e in &roots {
        despawn_recursive(&mut commands, e, &children_q);
    }
}

// manual recursive despawn (no trait needed)
fn despawn_recursive(commands: &mut Commands, entity: Entity, children_q: &Query<&Children>) {
    if let Ok(children) = children_q.get(entity) {
        // Children::iter() yields Entity by value in Bevy 0.17
        for child in children.iter() {
            despawn_recursive(commands, child, children_q);
        }
    }
    commands.entity(entity).despawn();
}

