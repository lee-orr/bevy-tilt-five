mod bridge;

use bevy::{prelude::*, utils::HashMap};
use bridge::*;

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
                .add_system(disconnect_from_glasses);
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
    pub glasses: HashMap<String, Option<Entity>>,
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
struct TiltFiveGlasses(Option<Glasses>);

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

fn connect_to_glasses(mut client: NonSendMut<T5Client>, mut list: ResMut<AvailableGlasses>, mut events: EventWriter<TiltFiveClientEvent>, mut reader: EventReader<TiltFiveCommands>, mut commands: Commands) {
    for evt in reader.iter() {
        if let TiltFiveCommands::ConnectToGlasses(glasses_id) = evt {
            if let Ok(glasses) = client.create_glasses(&glasses_id) {
                if let Some(value) = list.glasses.get(glasses_id) {
                    if value.is_none() {
                        let entity = commands.spawn((SpatialBundle::default(), TiltFiveGlasses(Some(glasses)))).id();
                        list.glasses.insert(glasses_id.clone(), Some(entity));
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
            if let Some(Some(entity)) = list.glasses.get(glasses_id) {
                if let Ok(mut g) = query.get_mut(*entity) {
                    if let Some(g) = g.0.take() {
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