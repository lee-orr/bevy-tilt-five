use bevy::prelude::*;
use std::f32::consts::PI;

use crate::bridge::ffi::T5_GlassesPose;

// GBD - gameboard space - +x right +y forward +z up
// GLS - glasses space - +x right +y up +z backward
// BGBD - bevy gamboard - +x right +y up +z backward

pub fn transform_matrix_from_bevy_to_glasses_space(
    glasses_from_gameboard: &T5_GlassesPose,
) -> (Transform, Transform) {
    let glasses_from_gameboard_rotation: Quat = glasses_from_gameboard.rotToGLS_GBD.into();
    let glasses_from_gameboard_position: Vec3 = glasses_from_gameboard.posGLS_GBD.into();

    let transform_from_gameboard_to_glasses =
        Transform::from_translation(glasses_from_gameboard_position)
            .with_rotation(glasses_from_gameboard_rotation);

    let transform_from_world_to_gameboard =
        Transform::from_rotation(Quat::from_rotation_x(-PI / 2.));

    let conversion = Quat::from_rotation_x(PI);
    let rotation = conversion * glasses_from_gameboard_rotation;
    let rotation = rotation.conjugate();
    let transform = transform_from_world_to_gameboard
        * Transform::from_rotation(rotation).with_translation(glasses_from_gameboard_position);

    (transform, transform_from_gameboard_to_glasses)
}
