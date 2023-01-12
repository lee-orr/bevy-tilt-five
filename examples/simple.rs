use bevy::{prelude::*};
use bevy_egui::{EguiPlugin, EguiContext};
use bevy_inspector_egui::{quick::{WorldInspectorPlugin, ResourceInspectorPlugin}, egui};
use bevy_tilt_five::{TiltFivePlugin, BoardBundle, AvailableGlasses, TiltFiveCommands};

fn main() {
    App::new()
    .add_plugins(DefaultPlugins)
    .add_plugin(EguiPlugin)
    .add_plugin(TiltFivePlugin)
    .add_startup_system(setup)
    .add_system(connect_glasses)
    .add_plugin(WorldInspectorPlugin)
    .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // plane
    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Plane { size: 5.0 })),
        material: materials.add(Color::rgb(0.3, 0.5, 0.3).into()),
        ..default()
    });
    // cube
    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Cube { size: 1.0 })),
        material: materials.add(Color::rgb(0.8, 0.7, 0.6).into()),
        transform: Transform::from_xyz(0.0, 0.5, 0.0),
        ..default()
    });
    // light
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });
    // camera
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    commands.spawn(BoardBundle::default());
}

fn connect_glasses(mut egui_context: ResMut<EguiContext>, mut events: EventWriter<TiltFiveCommands>, glasses: Res<AvailableGlasses>) {
    let connected_glasses = glasses.glasses.iter().filter_map(|(name, val)| {
        if let Some((_, left, right)) = val {
            if let (Some(left), Some(right)) = (egui_context.image_id(left), egui_context.image_id(right)) {
                Some((name, Some((left, right))))
            } else {
                Some((name, None))
            }
        } else {
            None
        }
    }).collect::<Vec<_>>();
    let ctx = egui_context.ctx_mut();
    egui::Window::new("T5 Status").show(ctx, |ui| {
        ui.label("Available Glasses:");
        for (key, val) in glasses.glasses.iter() {
            if val.is_none() {
                if ui.button(key).clicked() {
                    events.send(TiltFiveCommands::ConnectToGlasses(key.clone()));
                }
            }
        }
        ui.label("Connected Glasses:");
        for (key, images) in connected_glasses.iter() {
            if let Some((left, right)) = images {
                ui.label("left");
                ui.image(*left, [300.0, 300.0]);
                ui.label("right");
                ui.image(*right, [300.0, 300.0]);
            } else {
                ui.label("Couldn't get image");
            }
            if ui.button(*key).clicked() {
                events.send(TiltFiveCommands::DisconnectFromGlasses(key.to_string()));
            }
        }
        if ui.button("Refresh List").clicked() {
            events.send(TiltFiveCommands::RefreshGlassesList);
        }
    });
}