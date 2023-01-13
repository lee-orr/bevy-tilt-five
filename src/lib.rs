mod bridge;

use bevy::{prelude::*, utils::HashMap, render::{render_resource::{Extent3d, TextureDimension, TextureFormat, TextureDescriptor, TextureUsages}, camera::RenderTarget}};
use bridge::*;

pub use bridge::T5GameboardType;

pub struct TiltFivePlugin;

impl Plugin for TiltFivePlugin {
    fn build(&self, app: &mut App) {
        app
            .add_event::<TiltFiveClientEvent>()
            .add_event::<TiltFiveCommands>()
            .init_resource::<AvailableGlasses>()
            .register_type::<AvailableGlasses>();

        if let Ok(client) = T5Client::new("my-app", "1") {
            println!("Setting up T5 Client");
            app
            
                .insert_non_send_resource(client)
                .add_system(check_glasses_list)
                .add_system(connect_to_glasses)
                .add_system(disconnect_from_glasses)
                .add_system(setup_glasses_rendering)
                .add_system(set_glasses_position)
                .add_system(send_glasses_frames)
                .add_system(setup_debug_meshes);
        }
    }
}

#[derive(Bundle, Default)]
pub struct BoardBundle {
    board: Board,
    spatial: SpatialBundle
}

#[derive(Resource, Reflect, Debug, Default)]
pub struct AvailableGlasses {
    pub glasses: HashMap<String, Option<(Entity, Handle<Image>, Handle<Image>)>>,
}

#[derive(Component, Default)]
pub struct Board;

#[derive(Debug, Clone)]
pub enum TiltFiveClientEvent {
    GlassesFound(String),
    GlassesConnected(String),
    GlassesDisconnected(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TiltFiveCommands {
    RefreshGlassesList,
    ConnectToGlasses(String),
    DisconnectFromGlasses(String)
}

#[derive(Component)]
struct TiltFiveGlasses(Option<(Glasses, Handle<Image>, Handle<Image>)>);

pub const GLASSES_TEXTURE_SIZE: Extent3d = Extent3d { width: DEFAULT_GLASSES_WIDTH, height: DEFAULT_GLASSES_HEIGHT, depth_or_array_layers: 1 };

fn check_glasses_list(mut client: NonSendMut<T5Client>, mut list: ResMut<AvailableGlasses>, mut events: EventWriter<TiltFiveClientEvent>, mut reader: EventReader<TiltFiveCommands>) {
    for evt in reader.iter() {
        if evt == &TiltFiveCommands::RefreshGlassesList {
            if let Ok(new_list) = client.list_glasses() {
                for glasses in new_list.iter() {
                    if !list.glasses.contains_key(glasses) {
                        list.glasses.insert(glasses.clone(), None);
                        events.send(TiltFiveClientEvent::GlassesFound(glasses.clone()));
                    }
                }
            }
            break;
        }
    }
}

fn connect_to_glasses(mut client: NonSendMut<T5Client>, mut list: ResMut<AvailableGlasses>, mut events: EventWriter<TiltFiveClientEvent>, mut reader: EventReader<TiltFiveCommands>, mut commands: Commands, mut assets: ResMut<Assets<Image>>) {
    for evt in reader.iter() {
        if let TiltFiveCommands::ConnectToGlasses(glasses_id) = evt {
            if let Ok(glasses) = client.create_glasses(glasses_id) {
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
                        let entity = commands.spawn((SpatialBundle::default(), TiltFiveGlasses(Some((glasses, left.clone(), right.clone()))))).id();
                        list.glasses.insert(glasses_id.clone(), Some((entity, left, right)));
                        events.send(TiltFiveClientEvent::GlassesConnected(glasses_id.clone()));
                    }
                }
            }
        }
    }
}

fn disconnect_from_glasses(mut client: NonSendMut<T5Client>, mut list: ResMut<AvailableGlasses>, mut events: EventWriter<TiltFiveClientEvent>, mut reader: EventReader<TiltFiveCommands>, mut commands: Commands, mut query: Query<&mut TiltFiveGlasses>) {
    for evt in reader.iter() {
        if let TiltFiveCommands::DisconnectFromGlasses(glasses_id) = evt {
            if let Some(Some((entity, _, _))) = list.glasses.get(glasses_id) {
                if let Ok(mut g) = query.get_mut(*entity) {
                    if let Some((g,_,_)) = g.0.take() {
                        let _ = client.release_glasses(g);
                    }
                }
                commands.entity(*entity).despawn_recursive();
            }
            list.glasses.insert(glasses_id.clone(), None);
            events.send(TiltFiveClientEvent::GlassesConnected(glasses_id.clone()));            
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

fn set_glasses_position(mut glasses: Query<(&mut Transform, &TiltFiveGlasses)>, mut client: NonSendMut<T5Client>) {
    for (mut transform, glasses) in glasses.iter_mut() {
        if let Some((glasses,_,_)) = &glasses.0 {
            match client.get_glasses_pose(glasses) {
                Ok(pose) => {
                    bevy::log::info!("Got pose!");
                    transform.translation = Vec3::new(pose.posGLS_GBD.x, pose.posGLS_GBD.z, -pose.posGLS_GBD.y);
                    transform.rotation = Quat::from_xyzw(pose.rotToGLS_GBD.x, pose.rotToGLS_GBD.z, -pose.rotToGLS_GBD.y, pose.rotToGLS_GBD.w);
                }
                Err(e) => bevy::log::error!("Couldn't get pose {e:?}"),
            }
        }
    }
}

fn send_glasses_frames(glasses: Query<&TiltFiveGlasses>, mut client: NonSendMut<T5Client>, images: Res<Assets<Image>>) {
    for (glasses) in glasses.iter() {
        if let Some((glasses,left,right)) = &glasses.0 {
            if let Some(left) = images.get(left) {
                bevy::log::info!("Image...");
            }
        }
    }
}

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