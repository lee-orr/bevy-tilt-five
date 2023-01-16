use std::num::NonZeroU32;

use bevy::{
    prelude::Image,
    render::{
        render_asset::{RenderAssets},
        render_graph::Node,
    },
};
use wgpu::{ImageCopyBuffer, ImageDataLayout};

use crate::{
    bridge::{DEFAULT_GLASSES_HEIGHT, DEFAULT_GLASSES_WIDTH},
    T5RenderGlassesList, GLASSES_TEXTURE_SIZE, TEXTURE_FORMAT,
};

pub const EYE_CLONE_NODE_NAME: &str = "eye_clone_node";

#[derive(Default)]
pub struct EyeCloneNode;

impl Node for EyeCloneNode {
    fn run(
        &self,
        _graph: &mut bevy::render::render_graph::RenderGraphContext,
        render_context: &mut bevy::render::renderer::RenderContext,
        world: &bevy::prelude::World,
    ) -> Result<(), bevy::render::render_graph::NodeRunError> {
        let list = world.resource::<T5RenderGlassesList>();
        let format = TEXTURE_FORMAT;
        let fmt = format.describe();
        let bytes_per_row =
            DEFAULT_GLASSES_WIDTH * (fmt.block_dimensions.0 as u32) * (fmt.block_size as u32);

        for (_, (_, images, buffers, _)) in list.glasses.iter() {
            if let (Some((left, right)), Some((lb, rb))) = (&images, &buffers) {
                if let Some(image) = world.resource::<RenderAssets<Image>>().get(left) {
                    render_context.command_encoder.copy_texture_to_buffer(
                        image.texture.as_image_copy(),
                        ImageCopyBuffer {
                            buffer: lb,
                            layout: ImageDataLayout {
                                offset: 0,
                                bytes_per_row: Some(NonZeroU32::new(bytes_per_row).unwrap()),
                                rows_per_image: Some(
                                    NonZeroU32::new(DEFAULT_GLASSES_HEIGHT).unwrap(),
                                ),
                            },
                        },
                        GLASSES_TEXTURE_SIZE,
                    );
                }
                if let Some(image) = world.resource::<RenderAssets<Image>>().get(right) {
                    render_context.command_encoder.copy_texture_to_buffer(
                        image.texture.as_image_copy(),
                        ImageCopyBuffer {
                            buffer: rb,
                            layout: ImageDataLayout {
                                offset: 0,
                                bytes_per_row: Some(NonZeroU32::new(bytes_per_row).unwrap()),
                                rows_per_image: Some(
                                    NonZeroU32::new(DEFAULT_GLASSES_HEIGHT).unwrap(),
                                ),
                            },
                        },
                        GLASSES_TEXTURE_SIZE,
                    );
                }
            }
        }

        Ok(())
    }
}
