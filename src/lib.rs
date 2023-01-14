mod bridge;

use std::sync::mpsc::{channel, Receiver, Sender};

use bevy::{
    prelude::*,
    render::{
        camera::RenderTarget,
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        RenderApp, RenderStage,
    },
    utils::HashMap,
};
use bridge::*;

pub use bridge::T5GameboardType;

pub struct TiltFivePlugin;

impl Plugin for TiltFivePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<TiltFiveClientEvent>()
            .add_event::<TiltFiveCommands>()
            .init_resource::<AvailableGlasses>()
            .register_type::<AvailableGlasses>();

        if let Ok(client) = T5Client::new("my-app", "1") {
            println!("Setting up T5 Client");
            let (command_sender, command_receiver) = channel();
            let (event_sender, event_receiver) = channel();

            let main_app_client = T5ClientMainApp {
                sender: command_sender,
                receiver: event_receiver,
            };

            let render_app_client = T5ClientRenderApp {
                client,
                sender: event_sender,
                receiver: command_receiver,
            };

            app.insert_non_send_resource(main_app_client)
                .add_system(communicate_with_client)
                .add_system(update_glasses_list)
                .add_system(connect_to_glasses)
                .add_system(disconnect_from_glasses)
                .add_system(setup_glasses_rendering)
                .add_system(set_glasses_position);

            app
                .add_system(setup_debug_meshes);

            let render_app = app.sub_app_mut(RenderApp);
            render_app
                .insert_non_send_resource(render_app_client)
                .insert_resource(T5RenderGlassesList { glasses: Default::default()})
                .add_system_to_stage(RenderStage::Extract, get_glasses_pose)
                .add_system_to_stage(RenderStage::Extract, process_commands);
        }
    }
}

struct T5ClientMainApp {
    pub sender: Sender<TiltFiveCommands>,
    pub receiver: Receiver<TiltFiveClientEvent>,
}

struct T5ClientRenderApp {
    client: T5Client,
    sender: Sender<TiltFiveClientEvent>,
    receiver: Receiver<TiltFiveCommands>,
}

#[derive(Resource)]
struct T5RenderGlassesList {
    glasses: HashMap<String, (Glasses, Option<(Handle<Image>, Handle<Image>)>)>,
}

#[derive(Bundle, Default)]
pub struct BoardBundle {
    board: Board,
    spatial: SpatialBundle,
}

#[derive(Resource, Reflect, Debug, Default)]
pub struct AvailableGlasses {
    pub glasses: HashMap<String, Option<(Entity, Handle<Image>, Handle<Image>)>>,
}

#[derive(Component, Default)]
pub struct Board;

#[derive(Debug, Clone)]
pub enum TiltFiveClientEvent {
    GlassesFound(Vec<String>),
    GlassesConnected(String),
    GlassesDisconnected(String),
    GlassesPoseChanged(String, Transform),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TiltFiveCommands {
    RefreshGlassesList,
    ConnectToGlasses(String),
    DisconnectFromGlasses(String),
}

#[derive(Component)]
struct TiltFiveGlasses(Option<(String, Handle<Image>, Handle<Image>)>);

fn communicate_with_client(
    mut client: NonSendMut<T5ClientMainApp>,
    mut commands: EventReader<TiltFiveCommands>,
    mut events: EventWriter<TiltFiveClientEvent>,
) {
    for command in commands.iter() {
        client.sender.send(command.clone());
    }

    while let Ok(event) = client.receiver.try_recv() {
        events.send(event);
    }
}

fn process_commands(mut client: NonSendMut<T5ClientRenderApp>, mut list: ResMut<T5RenderGlassesList>) {
    while let Ok(command) = client.receiver.try_recv() {
        match command {
            TiltFiveCommands::RefreshGlassesList => {
                if let Ok(new_list) = client.client.list_glasses() {
                    let _ = client
                        .sender
                        .send(TiltFiveClientEvent::GlassesFound(new_list));
                }
            }
            TiltFiveCommands::ConnectToGlasses(glasses_id) => {
                if !list.glasses.contains_key(&glasses_id) {
                    if let Ok(glasses) = client.client.create_glasses(&glasses_id) {
                        list.glasses.insert(glasses_id.clone(), (glasses, None));
                        let _ = client
                            .sender
                            .send(TiltFiveClientEvent::GlassesConnected(glasses_id));
                    }
                }
            }
            TiltFiveCommands::DisconnectFromGlasses(glasses_id) => {
                if let Some((glasses, _)) = list.glasses.remove(&glasses_id) {
                    let _ = client.client.release_glasses(glasses);
                    let _ = client
                        .sender
                        .send(TiltFiveClientEvent::GlassesDisconnected(glasses_id));
                }
            }
        }
    }
}

pub const GLASSES_TEXTURE_SIZE: Extent3d = Extent3d {
    width: DEFAULT_GLASSES_WIDTH,
    height: DEFAULT_GLASSES_HEIGHT,
    depth_or_array_layers: 1,
};

fn update_glasses_list(
    mut list: ResMut<AvailableGlasses>,
    mut reader: EventReader<TiltFiveClientEvent>,
) {
    for evt in reader.iter() {
        if let TiltFiveClientEvent::GlassesFound(new_list) = evt {
            for glasses in new_list.iter() {
                if !list.glasses.contains_key(glasses) {
                    list.glasses.insert(glasses.clone(), None);
                }
            }
            break;
        }
    }
}

fn connect_to_glasses(
    mut list: ResMut<AvailableGlasses>,
    mut events: EventReader<TiltFiveClientEvent>,
    mut commands: Commands,
    mut assets: ResMut<Assets<Image>>,
) {
    for evt in events.iter() {
        if let TiltFiveClientEvent::GlassesConnected(glasses_id) = evt {
            if let Some(value) = list.glasses.get(glasses_id) {
                if value.is_none() {
                    let mut left = Image {
                        texture_descriptor: TextureDescriptor {
                            label: None,
                            size: GLASSES_TEXTURE_SIZE,
                            dimension: TextureDimension::D2,
                            format: TextureFormat::Rgba8Unorm,
                            mip_level_count: 1,
                            sample_count: 1,
                            usage: TextureUsages::TEXTURE_BINDING
                                | TextureUsages::COPY_DST
                                | TextureUsages::RENDER_ATTACHMENT
                                | TextureUsages::COPY_SRC,
                        },
                        ..default()
                    };
                    left.resize(GLASSES_TEXTURE_SIZE);
                    let mut right = Image {
                        texture_descriptor: TextureDescriptor {
                            label: None,
                            size: GLASSES_TEXTURE_SIZE,
                            dimension: TextureDimension::D2,
                            format: TextureFormat::Rgba8Unorm,
                            mip_level_count: 1,
                            sample_count: 1,
                            usage: TextureUsages::TEXTURE_BINDING
                                | TextureUsages::COPY_DST
                                | TextureUsages::RENDER_ATTACHMENT
                                | TextureUsages::COPY_SRC,
                        },
                        ..default()
                    };
                    right.resize(GLASSES_TEXTURE_SIZE);

                    let left = assets.add(left);
                    let right = assets.add(right);
                    let entity = commands
                        .spawn((
                            SpatialBundle::default(),
                            TiltFiveGlasses(Some((
                                glasses_id.clone(),
                                left.clone(),
                                right.clone(),
                            ))),
                        ))
                        .id();
                    list.glasses
                        .insert(glasses_id.clone(), Some((entity, left, right)));
                }
            }
        }
    }
}

fn disconnect_from_glasses(
    mut list: ResMut<AvailableGlasses>,
    mut events: EventReader<TiltFiveClientEvent>,
    mut commands: Commands,
) {
    for evt in events.iter() {
        if let TiltFiveClientEvent::GlassesDisconnected(glasses_id) = evt {
            if let Some(Some((entity, _, _))) = list.glasses.get(glasses_id) {
                commands.entity(*entity).despawn_recursive();
            }
            list.glasses.insert(glasses_id.clone(), None);
        }
    }
}

fn setup_glasses_rendering(mut commands: Commands, query: Query<(Entity, &TiltFiveGlasses), Added<TiltFiveGlasses>>, boards: Query<Entity, With<Board>>) {
    if let Ok(board) = boards.get_single() {
        for (entity, glasses ) in query.iter() {
            if let Some((_, left, right)) = &glasses.0 {
                commands.entity(entity).with_children(|parent| {
                    parent.spawn(Camera3dBundle {
                        transform: Transform::from_xyz(-0.1, 0., 0.),
                        camera: Camera {
                            priority: -2,
                            target: RenderTarget::Image(left.clone()),
                            ..Default::default()
                        },
                        projection: Projection::Perspective(PerspectiveProjection { fov: DEFAULT_GLASSES_FOV, ..Default::default() }),
                        ..Default::default()
                    });
                    parent.spawn(Camera3dBundle {
                        transform: Transform::from_xyz(0.1, 0., 0.),
                        camera: Camera {
                            priority: -1,
                            target: RenderTarget::Image(right.clone()),
                            ..Default::default()
                        },
                        projection: Projection::Perspective(PerspectiveProjection { fov: DEFAULT_GLASSES_FOV, ..Default::default() }),
                        ..Default::default()
                    });
                });
                commands.entity(board).add_child(entity);
            }
        }
    }
}

fn get_glasses_pose(mut client: NonSendMut<T5ClientRenderApp>, list: Res<T5RenderGlassesList>) {
    for (id, (glasses, _)) in list.glasses.iter() {
        match client.client.get_glasses_pose(glasses) {
            Ok(pose) => {
                bevy::log::info!("Got pose!");
                let pos = Vec3::new(pose.posGLS_GBD.x, pose.posGLS_GBD.z, -pose.posGLS_GBD.y);
                let rotation = Quat::from_xyzw(pose.rotToGLS_GBD.x, pose.rotToGLS_GBD.z, -pose.rotToGLS_GBD.y, pose.rotToGLS_GBD.w);
                client.sender.send(TiltFiveClientEvent::GlassesPoseChanged(id.clone(), Transform::from_translation(pos).with_rotation(rotation)));
            }
            Err(e) => bevy::log::error!("Couldn't get pose {e:?}"),
        }
    }
}

fn set_glasses_position(mut commands: Commands, list: Res<AvailableGlasses>, mut events: EventReader<TiltFiveClientEvent>) {
    for event in events.iter() {
        match event {
            TiltFiveClientEvent::GlassesPoseChanged(id, transform) => {
                if let Some(Some((entity, _, _))) = list.glasses.get(id) {
                    commands.entity(*entity).insert(transform.clone());
                }
            }
         _ => {}
        }
    }
}

/*

fn send_glasses_frames(glasses: Query<&TiltFiveGlasses>, mut client: NonSendMut<T5Client>, images: Res<Assets<Image>>) {
    for (glasses) in glasses.iter() {
        if let Some((glasses,left,right)) = &glasses.0 {
            if let Some(left) = images.get(left) {
                bevy::log::info!("Image...");
            }
        }
    }
} */

fn setup_debug_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
   query: Query<Entity, Added<TiltFiveGlasses>>
) {
    for entity in query.iter() {
        commands.entity(entity).with_children(|p| {
            p.spawn(PbrBundle {
                mesh: meshes.add(Mesh::from(shape::Cube{ size: 0.2})),
                material: materials.add(Color::rgb(0.8, 0.1, 0.2).into()),
                ..default()});
        });
    }
}
