use bevy::{
    prelude::*,
    render::settings::{Backends, WgpuSettings},
};
use bevy_egui::{EguiContext, EguiPlugin};
use bevy_inspector_egui::{
    egui::{self, Ui},
    quick::{ResourceInspectorPlugin, WorldInspectorPlugin},
};
use bevy_tilt_five::{
    AvailableGlasses, BoardBundle, DebugGizmo, TiltFiveClientEvent, TiltFiveCommands,
    TiltFivePlugin, GLS_TO_FINAL,
};

fn main() {
    App::new()
        .insert_resource(WgpuSettings {
            backends: Some(Backends::DX12),
            ..Default::default()
        })
        .add_plugins(DefaultPlugins)
        .add_plugin(EguiPlugin)
        .add_plugin(TiltFivePlugin)
        .add_startup_system(setup)
        .add_system(connect_glasses)
        .add_plugin(WorldInspectorPlugin)
        .add_plugin(ResourceInspectorPlugin::<GLS_TO_FINAL>::default())
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

    // Gizmo
    commands.spawn((
        SpatialBundle::from_transform(Transform::from_xyz(2.2, 0., 0.)),
        DebugGizmo(Color::RED),
    ));

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
        transform: Transform::from_xyz(-2.0, 2.5, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    commands.spawn(BoardBundle {
        ..Default::default()
    });
}

fn connect_glasses(
    mut egui_context: ResMut<EguiContext>,
    mut events: EventWriter<TiltFiveCommands>,
    mut read_events: EventReader<TiltFiveClientEvent>,
    glasses: Res<AvailableGlasses>,
) {
    let connected_glasses = glasses
        .glasses
        .iter()
        .filter_map(|(name, val)| {
            if let Some((_, left, right)) = val {
                Some((
                    name,
                    (
                        egui_context.add_image(left.clone_weak()),
                        egui_context.add_image(right.clone_weak()),
                    ),
                ))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let ctx = egui_context.ctx_mut();
    egui::Window::new("T5 Status").show(ctx, |ui| {
        ui.label("Available Glasses:");
        for (key, val) in glasses.glasses.iter() {
            if val.is_none() && ui.button(key).clicked() {
                events.send(TiltFiveCommands::ConnectToGlasses(key.clone()));
            }
        }
        ui.label("Connected Glasses:");
        for (key, images) in connected_glasses.iter() {
            let (left, right) = images;

            ui.horizontal(|ui| {
                ui.label("left");
                ui.image(*left, [121.6, 76.8]);
                ui.label("right");
                ui.image(*right, [121.6, 76.8]);
            });

            if ui.button(*key).clicked() {
                events.send(TiltFiveCommands::DisconnectFromGlasses(key.to_string()));
            }
        }
        if ui.button("Refresh List").clicked() {
            events.send(TiltFiveCommands::RefreshGlassesList);
        }

        for evt in read_events.iter() {
            match evt {
                TiltFiveClientEvent::GlassesPoseChanged(_, t, _, o) => {
                    let t_p = t.translation;
                    let t_r = t.rotation;
                    let o_p = o.translation;
                    let o_r = o.rotation;
                    ui.label("Current Position:");
                    display_vec3(ui, &t_p, 1.);
                    ui.label("Org Position:");
                    display_vec3(ui, &o_p, 1.);

                    ui.label("Current Rotation:");
                    display_rotation(ui, &t_r);
                    ui.label("Org Rotation:");
                    display_rotation(ui, &o_r);
                }
                _ => {}
            }
        }
    });
}

fn display_rotation(ui: &mut Ui, rotation: &Quat) {
    let rot = rotation.to_euler(EulerRot::XYZ);
    display_vec3(ui, &Vec3::new(rot.0, rot.1, rot.2), 1f32.to_degrees());
}

fn display_vec3(ui: &mut Ui, vec: &Vec3, scale_by: f32) {
    ui.horizontal(|ui| {
        ui.label("X: ");
        ui.label(format!("{:05.2}", vec.x * scale_by));
        ui.label(" Y: ");
        ui.label(format!("{:05.2}", vec.y * scale_by));
        ui.label("Z: ");
        ui.label(format!("{:05.2}", vec.z * scale_by));
    });
}
