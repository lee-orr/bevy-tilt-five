mod bridge;

mod conversions;
#[cfg(target_family = "windows")]
mod dx_11_interface;
mod eye_clone_node;

use std::{
    f32::consts::PI,
    sync::mpsc::{channel, Receiver, Sender},
    time::Duration,
};

use bevy::{
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::{
        camera::RenderTarget,
        extract_resource::ExtractResource,
        main_graph::node::CAMERA_DRIVER,
        render_graph::RenderGraph,
        render_resource::{
            Buffer, Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        renderer::RenderDevice,
        RenderApp, RenderStage,
    },
    utils::HashMap,
};
use bridge::{
    ffi::{T5_Quat, T5_Vec3, T5_WandReport},
    *,
};

pub use bridge::T5GameboardType;
use wgpu::{BufferDescriptor, BufferUsages, MapMode};

use crate::conversions::transform_matrix_from_bevy_to_glasses_space;

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
                .add_system(setup_board_transformer)
                .add_system(setup_debug_gizmo)
                .add_system(communicate_with_client)
                .add_system(update_glasses_list)
                .add_system(connect_to_glasses)
                .add_system(disconnect_from_glasses)
                .add_system(setup_glasses_rendering)
                .add_system(set_glasses_position)
                .add_system(adjust_glasses_position);

            app.add_system(setup_debug_meshes);

            let render_app = app.sub_app_mut(RenderApp);
            render_app
                .insert_non_send_resource(render_app_client)
                .insert_resource(T5RenderGlassesList {
                    glasses: Default::default(),
                })
                .add_system_to_stage(RenderStage::Extract, get_glasses_pose)
                .add_system_to_stage(RenderStage::Extract, process_commands)
                .add_system_to_stage(RenderStage::Prepare, setup_buffers_for_frame)
                .add_system_to_stage(RenderStage::Cleanup, retrieve_textures_from_gpu);

            let mut graph = render_app.world.get_resource_mut::<RenderGraph>().unwrap();

            graph.add_node(
                eye_clone_node::EYE_CLONE_NODE_NAME,
                eye_clone_node::EyeCloneNode::default(),
            );
            graph
                .add_node_edge(CAMERA_DRIVER, eye_clone_node::EYE_CLONE_NODE_NAME)
                .unwrap();

            #[cfg(target_family = "windows")]
            {
                app.add_plugin(dx_11_interface::DX11Plugin);
                // app.add_plugin(ogl_interface::OGLPlugin);
            }
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

type GlassesMapData = (
    Glasses,
    Option<(Handle<Image>, Handle<Image>)>,
    Option<(Buffer, Buffer)>,
    Option<(T5_Vec3, T5_Vec3, T5_Quat)>,
);

#[derive(Resource)]
struct T5RenderGlassesList {
    glasses: HashMap<String, GlassesMapData>,
}

#[derive(Bundle, Default)]
pub struct BoardBundle {
    pub board: Board,
    pub spatial: SpatialBundle,
}

#[derive(Component)]
struct BoardTransformer;

type GlassesInfo = (Entity, Handle<Image>, Handle<Image>);

#[derive(Resource, Reflect, Debug, Default, Clone, ExtractResource)]
pub struct AvailableGlasses {
    pub glasses: HashMap<String, Option<GlassesInfo>>,
}

#[derive(Component)]
pub struct Board(f32);

impl Default for Board {
    fn default() -> Self {
        Self(1.)
    }
}

#[derive(Debug, Clone)]
pub enum TiltFiveClientEvent {
    GlassesFound(Vec<String>),
    GlassesConnected(String),
    GlassesDisconnected(String),
    GlassesPoseChanged(String, Transform, f32, Transform),
    WandConnected {
        glasses: String,
        wand_id: String,
    },
    WandDisconnected {
        glasses: String,
        wand_id: String,
    },
    WandDesync {
        glasses: String,
        wand_id: String,
    },
    WantReportUpdated {
        glasses: String,
        wand_id: String,
        report: T5_WandReport,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TiltFiveCommands {
    RefreshGlassesList,
    ConnectToGlasses(String),
    DisconnectFromGlasses(String),
    SetGlassesImages(String, Handle<Image>, Handle<Image>),
}

#[derive(Component)]
struct TiltFiveGlasses(Option<(String, Handle<Image>, Handle<Image>)>);

#[derive(Component)]
struct TiltFiveIPD(f32);

#[derive(Component)]
pub struct DebugGizmo(pub Color);

fn setup_board_transformer(mut commands: Commands, boards: Query<Entity, Added<Board>>) {
    for entity in boards.iter() {
        commands.entity(entity).with_children(|p| {
            p.spawn((BoardTransformer, SpatialBundle::default()))
                .with_children(|p| {
                    // p.spawn((
                    //     SpatialBundle {
                    //         transform: Transform::from_rotation(Quat::from_rotation_x(
                    //             -90f32.to_radians(),
                    //         )),
                    //         ..Default::default()
                    //     },
                    //     DebugGizmo(Color::BLACK),
                    // ));
                });
        });
    }
}

fn setup_debug_gizmo(
    mut commands: Commands,
    gizmos: Query<(Entity, &DebugGizmo), Added<DebugGizmo>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if gizmos.is_empty() {
        return;
    }

    let axis_mesh = meshes.add(shape::Box::new(0.05, 0.05, 2.).into());
    let ball_mesh = meshes.add(
        shape::Icosphere {
            radius: 0.1,
            subdivisions: 6,
        }
        .into(),
    );
    let box_mesh = meshes.add(shape::Cube::new(0.1).into());
    let axis_material = materials.add(Color::rgb(0.7, 0.7, 0.7).into());

    let x_mat = materials.add(Color::RED.into());
    let y_mat = materials.add(Color::GREEN.into());
    let z_mat = materials.add(Color::BLUE.into());

    let axis = [(Vec3::X, x_mat), (Vec3::Y, y_mat), (Vec3::Z, z_mat)];

    for (entity, gizmo) in gizmos.iter() {
        let my_mat = materials.add(gizmo.0.into());
        commands.entity(entity).with_children(|p| {
            p.spawn(PbrBundle {
                mesh: ball_mesh.clone(),
                material: my_mat,
                ..Default::default()
            });

            for (axis, color) in axis.iter() {
                p.spawn(PbrBundle {
                    mesh: axis_mesh.clone(),
                    material: axis_material.clone(),
                    transform: Transform::IDENTITY
                        .looking_at(-1. * *axis, axis.any_orthogonal_vector()),
                    ..Default::default()
                });
                p.spawn(PbrBundle {
                    mesh: ball_mesh.clone(),
                    material: color.clone(),
                    transform: Transform::from_translation(*axis),
                    ..default()
                });
                p.spawn(PbrBundle {
                    mesh: box_mesh.clone(),
                    material: color.clone(),
                    transform: Transform::from_translation(-1. * *axis),
                    ..default()
                });
            }
        });
    }
}

fn communicate_with_client(
    client: NonSendMut<T5ClientMainApp>,
    mut commands: EventReader<TiltFiveCommands>,
    mut events: EventWriter<TiltFiveClientEvent>,
) {
    for command in commands.iter() {
        let _ = client.sender.send(command.clone());
    }

    while let Ok(event) = client.receiver.try_recv() {
        events.send(event);
    }
}

fn process_commands(
    mut client: NonSendMut<T5ClientRenderApp>,
    mut list: ResMut<T5RenderGlassesList>,
    _device: Res<RenderDevice>,
) {
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
                        list.glasses
                            .insert(glasses_id.clone(), (glasses, None, None, None));
                        let _ = client
                            .sender
                            .send(TiltFiveClientEvent::GlassesConnected(glasses_id));
                    }
                }
            }
            TiltFiveCommands::DisconnectFromGlasses(glasses_id) => {
                if let Some((glasses, _, _, _)) = list.glasses.remove(&glasses_id) {
                    let _ = client.client.release_glasses(glasses);
                    let _ = client
                        .sender
                        .send(TiltFiveClientEvent::GlassesDisconnected(glasses_id));
                }
            }
            TiltFiveCommands::SetGlassesImages(id, left, right) => {
                if let Some(value) = list.glasses.get_mut(&id) {
                    value.1 = Some((left, right));
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

pub const TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

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
                            format: TEXTURE_FORMAT,
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
                            format: TEXTURE_FORMAT,
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

fn setup_glasses_rendering(
    mut commands: Commands,
    query: Query<(Entity, &TiltFiveGlasses), Added<TiltFiveGlasses>>,
    boards: Query<Entity, With<BoardTransformer>>,
    mut t5_commands: EventWriter<TiltFiveCommands>,
) {
    if let Ok(board) = boards.get_single() {
        for (entity, glasses) in query.iter() {
            if let Some((id, left, right)) = &glasses.0 {
                commands.entity(entity).with_children(|parent| {
                    parent.spawn((
                        Camera3dBundle {
                            transform: Transform::from_xyz(-0.1, 0., 0.)
                                .with_rotation(Quat::from_euler(EulerRot::XYZ, PI, 0., 0.)),
                            camera: Camera {
                                priority: -2,
                                target: RenderTarget::Image(left.clone()),
                                hdr: false,
                                ..Default::default()
                            },
                            tonemapping: Tonemapping::Enabled {
                                deband_dither: true,
                            },
                            projection: Projection::Perspective(PerspectiveProjection {
                                fov: DEFAULT_GLASSES_FOV.to_radians(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                        Visibility::default(),
                        ComputedVisibility::default(),
                    ));
                    parent.spawn((
                        Camera3dBundle {
                            transform: Transform::from_xyz(0.1, 0., 0.)
                                .with_rotation(Quat::from_euler(EulerRot::XYZ, PI, 0., 0.)),
                            camera: Camera {
                                priority: -1,
                                target: RenderTarget::Image(right.clone()),
                                hdr: false,
                                ..Default::default()
                            },
                            projection: Projection::Perspective(PerspectiveProjection {
                                fov: DEFAULT_GLASSES_FOV.to_radians(),
                                ..Default::default()
                            }),
                            tonemapping: Tonemapping::Enabled {
                                deband_dither: true,
                            },
                            ..Default::default()
                        },
                        Visibility::default(),
                        ComputedVisibility::default(),
                    ));
                    t5_commands.send(TiltFiveCommands::SetGlassesImages(
                        id.clone(),
                        left.clone(),
                        right.clone(),
                    ))
                });
                commands.entity(board).add_child(entity);
            }
        }
    }
}

fn adjust_glasses_position(
    mut cameras: Query<(&Camera3d, &mut Transform)>,
    parents: Query<(&TiltFiveIPD, &Children), Changed<TiltFiveIPD>>,
) {
    for (ipd, children) in parents.iter() {
        let ipd = ipd.0 / 2.;
        for child in children.iter() {
            if let Ok((_, mut transform)) = cameras.get_mut(*child) {
                transform.translation = transform.translation.normalize() * ipd;
            }
        }
    }
}

fn get_glasses_pose(
    mut client: NonSendMut<T5ClientRenderApp>,
    mut list: ResMut<T5RenderGlassesList>,
) {
    for (id, mut value) in list.glasses.iter_mut() {
        match (
            client.client.get_glasses_pose(&value.0),
            client.client.get_ipd(&value.0),
        ) {
            (Ok(pose), Ok(ipd)) => {
                let (transform, org) = transform_matrix_from_bevy_to_glasses_space(&pose);

                let ipd = ipd * 0.001;
                let lpos = org.left() * ipd + org.translation;
                let rpos = org.right() * ipd + org.translation;

                let _ = client.sender.send(TiltFiveClientEvent::GlassesPoseChanged(
                    id.clone(),
                    transform,
                    ipd,
                    org,
                ));

                let lpos = T5_Vec3 {
                    x: lpos.x,
                    y: lpos.y,
                    z: lpos.z,
                };
                let rpos = T5_Vec3 {
                    x: rpos.x,
                    y: rpos.y,
                    z: rpos.z,
                };

                value.3 = Some((lpos, rpos, pose.rotToGLS_GBD));
            }
            _ => bevy::log::error!("Couldn't get pose"),
        }
    }
}

fn set_glasses_position(
    mut commands: Commands,
    list: Res<AvailableGlasses>,
    mut events: EventReader<TiltFiveClientEvent>,
) {
    for event in events.iter() {
        if let TiltFiveClientEvent::GlassesPoseChanged(id, transform, ipd, _) = event {
            if let Some(Some((entity, _, _))) = list.glasses.get(id) {
                commands
                    .entity(*entity)
                    .insert((*transform, TiltFiveIPD(*ipd)));
            }
        }
    }
}

fn setup_debug_meshes(mut commands: Commands, query: Query<Entity, Added<TiltFiveGlasses>>) {
    // for entity in query.iter() {
    //     commands.entity(entity).with_children(|p| {
    //         p.spawn((
    //             SpatialBundle::from_transform(Transform::from_scale(Vec3::ONE * 0.2)),
    //             DebugGizmo(Color::YELLOW),
    //         ));
    //     });
    // }
}

pub type GlassesBufferInfo = (Glasses, Vec<u8>, Vec<u8>, T5_Vec3, T5_Vec3, T5_Quat);
struct BufferSender {
    pub sender: Sender<GlassesBufferInfo>,
}

fn setup_buffers_for_frame(mut glasses: ResMut<T5RenderGlassesList>, device: Res<RenderDevice>) {
    let fmt = TEXTURE_FORMAT.describe();
    let bytes_per_row =
        DEFAULT_GLASSES_WIDTH * (fmt.block_dimensions.0 as u32) * (fmt.block_size as u32);
    let padded_bytes_total: u64 = bytes_per_row as u64 * (DEFAULT_GLASSES_HEIGHT as u64);
    for (_, mut val) in glasses.glasses.iter_mut() {
        if val.1.is_some() {
            let left_buffer = device.create_buffer(&BufferDescriptor {
                label: Some("Left Eye Buffer"),
                size: padded_bytes_total,
                usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let right_buffer = device.create_buffer(&BufferDescriptor {
                label: Some("Right Eye Buffer"),
                size: padded_bytes_total,
                usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            val.2 = Some((left_buffer, right_buffer));
        }
    }
}

const FRAME_DURATION: Duration = Duration::from_millis(16);

fn retrieve_textures_from_gpu(
    glasses: Res<T5RenderGlassesList>,
    device: Res<RenderDevice>,
    buffer_sender: NonSendMut<BufferSender>,
) {
    for (_, (glasses, _images, buffers, transform)) in glasses.glasses.iter() {
        if let (Some((lb, rb)), Some((lpos, rpos, rot))) = (buffers, transform) {
            let ls = lb.slice(..);
            let rs = rb.slice(..);

            let (ready_sender, ready_receiver) = channel();

            let l_sender = ready_sender.clone();
            let r_seder = ready_sender.clone();

            device.map_buffer(&ls, MapMode::Read, move |_| {
                let _ = l_sender.send(true);
            });
            device.map_buffer(&rs, MapMode::Read, move |_| {
                let _ = r_seder.send(true);
            });

            device.poll(wgpu::Maintain::Wait);

            if ready_receiver.recv_timeout(FRAME_DURATION).is_ok()
                && ready_receiver.recv_timeout(FRAME_DURATION).is_ok()
            {
                let _ = buffer_sender.sender.send((
                    glasses.clone(),
                    ls.get_mapped_range().to_vec(),
                    rs.get_mapped_range().to_vec(),
                    *lpos,
                    *rpos,
                    *rot,
                ));
            }
        }
    }
}
