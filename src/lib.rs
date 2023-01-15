mod bridge;

#[cfg(target_family = "windows")]
mod dx_11_interface;
mod eye_clone_node;

use std::{sync::mpsc::{channel, Receiver, Sender}, time::Duration};

use bevy::{
    prelude::*,
    render::{
        camera::RenderTarget,
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, Buffer,
        },
        RenderApp, RenderStage, extract_component::ExtractComponent, extract_resource::ExtractResource, render_asset::RenderAssets, renderer::RenderDevice, main_graph::node::CAMERA_DRIVER, render_graph::RenderGraph,
    },
    utils::HashMap,
};
use bridge::{*, ffi::{T5_Quat, T5_Vec3}};

pub use bridge::T5GameboardType;
use wgpu::{BufferUsages, BufferDescriptor, MapMode};

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
                .add_system_to_stage(RenderStage::Extract, process_commands)
                .add_system_to_stage(RenderStage::Prepare, setup_buffers_for_frame)
                .add_system_to_stage(RenderStage::Cleanup, retrieve_textures_from_gpu);

            let mut graph = render_app.world.get_resource_mut::<RenderGraph>().unwrap();

            graph.add_node(eye_clone_node::EYE_CLONE_NODE_NAME, eye_clone_node::EyeCloneNode::default());
            graph.add_node_edge(CAMERA_DRIVER, eye_clone_node::EYE_CLONE_NODE_NAME).unwrap();

            #[cfg(target_family = "windows")]
            {
                app.add_plugin(dx_11_interface::DX11Plugin);
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

#[derive(Resource)]
struct T5RenderGlassesList {
    glasses: HashMap<String, (Glasses, Option<(Handle<Image>, Handle<Image>)>, Option<(Buffer, Buffer)>, Option<(T5_Vec3, T5_Vec3, T5_Quat)>)>,
}

#[derive(Bundle, Default)]
pub struct BoardBundle {
    board: Board,
    spatial: SpatialBundle,
}

#[derive(Resource, Reflect, Debug, Default, Clone, ExtractResource)]
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
    SetGlassesImages(String, Handle<Image>, Handle<Image>)
}

#[derive(Component)]
struct TiltFiveGlasses(Option<(String, Handle<Image>, Handle<Image>)>);

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


fn process_commands(mut client: NonSendMut<T5ClientRenderApp>,  mut list: ResMut<T5RenderGlassesList>, device: Res<RenderDevice>) {
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
                        list.glasses.insert(glasses_id.clone(), (glasses, None, None, None));
                        let _ = client
                            .sender
                            .send(TiltFiveClientEvent::GlassesConnected(glasses_id));
                    }
                }
            }
            TiltFiveCommands::DisconnectFromGlasses(glasses_id) => {
                if let Some((glasses,_, _, _)) = list.glasses.remove(&glasses_id) {
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
            },
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

fn setup_glasses_rendering(mut commands: Commands, query: Query<(Entity, &TiltFiveGlasses), Added<TiltFiveGlasses>>, boards: Query<Entity, With<Board>>, mut t5_commands: EventWriter<TiltFiveCommands>) {
    if let Ok(board) = boards.get_single() {
        for (entity, glasses ) in query.iter() {
            if let Some((id, left, right)) = &glasses.0 {
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
                    t5_commands.send(TiltFiveCommands::SetGlassesImages(id.clone(), left.clone(), right.clone()))
                });
                commands.entity(board).add_child(entity);
            }
        }
    }
}

fn get_glasses_pose(mut client: NonSendMut<T5ClientRenderApp>, mut list: ResMut<T5RenderGlassesList>) {
    for (id, mut value) in list.glasses.iter_mut() {
        match client.client.get_glasses_pose(&value.0) {
            Ok(pose) => {
                bevy::log::info!("Got pose!");
                let pos = Vec3::new(pose.posGLS_GBD.x, pose.posGLS_GBD.z, -pose.posGLS_GBD.y);
                let rotation = Quat::from_xyzw(pose.rotToGLS_GBD.x, pose.rotToGLS_GBD.z, -pose.rotToGLS_GBD.y, pose.rotToGLS_GBD.w);
                let transform = Transform::from_translation(pos).with_rotation(rotation);

                let lpos = transform.left() * 0.1 + pos;
                let rpos = transform.right() * 0.1 + pos;

                let _ = client.sender.send(TiltFiveClientEvent::GlassesPoseChanged(id.clone(), transform));

                let lpos = T5_Vec3 { x: lpos.x, y: -lpos.z, z: lpos.y };
                let rpos = T5_Vec3 { x: rpos.x, y: -rpos.z, z: rpos.y };

                value.3 = Some((lpos,rpos, pose.rotToGLS_GBD.clone()));
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

struct BufferSender {
    pub sender: Sender<(Glasses, Vec<u8>, Vec<u8>, T5_Vec3, T5_Vec3, T5_Quat)>
}

fn setup_buffers_for_frame(mut glasses: ResMut<T5RenderGlassesList>, device: Res<RenderDevice>) {
    let padded_bytes_per_row : u64 = (RenderDevice::align_copy_bytes_per_row(DEFAULT_GLASSES_WIDTH as usize) * 4) as u64;
    let padded_bytes_total: u64 = padded_bytes_per_row * (DEFAULT_GLASSES_HEIGHT as u64);
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
    buffer_sender: NonSendMut<BufferSender>
) {
    for (_, (glasses, images, buffers, transform)) in glasses.glasses.iter() {
        if let (Some((lb, rb)), Some((lpos, rpos, rot))) = (buffers, transform) {
            info!("Sending info for glasses...");
            let ls = lb.slice(..);
            let rs = rb.slice(..);

            let (ready_sender, ready_receiver) = channel();

            let l_sender = ready_sender.clone();
            let r_seder = ready_sender.clone();

            device.map_buffer(&ls, MapMode::Read,  move |_| {
                let _ = l_sender.clone().send(true);
            });
            device.map_buffer(&rs, MapMode::Read,  move|_| {
                let _ = r_seder.clone().send(true);
            });

            device.poll(wgpu::Maintain::Wait);

            if let Ok(_) = ready_receiver.recv_timeout(FRAME_DURATION) {
                if let Ok(_) = ready_receiver.recv_timeout(FRAME_DURATION)  {
                    let _= buffer_sender.sender.send((glasses.clone(), ls.get_mapped_range().to_vec(), rs.get_mapped_range().to_vec(), lpos.clone(), rpos.clone(), rot.clone()));
                }
            }
        }
    }
}