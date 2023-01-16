use bevy::prelude::*;
use std::f32::consts::PI;

use crate::{bridge::ffi::T5_GlassesPose, GLS_TO_FINAL};

pub fn position_t5_gameboard_to_bevy_gameboard(pos_t5_gameboard: Vec3) -> Vec3 {
    Vec3::new(
        pos_t5_gameboard.x,
        pos_t5_gameboard.z,
        -1. * pos_t5_gameboard.y,
    )
}

// GBD - gameboard space - +x right +y forward +z up
// GLS - glasses space - +x right +y up +z backward
// BGBD - bevy gamboard - +x right +y up +z backward

pub fn transform_matrix_from_bevy_to_glasses_space(
    glasses_from_gameboard: &T5_GlassesPose,
    _matrix_changer: &GLS_TO_FINAL,
) -> (Transform, Transform) {
    let glasses_from_gameboard_rotation: Quat = glasses_from_gameboard.rotToGLS_GBD.into();
    let glasses_from_gameboard_position: Vec3 = glasses_from_gameboard.posGLS_GBD.into();

    let transform_from_gameboard_to_glasses =
        Transform::from_translation(glasses_from_gameboard_position)
            .with_rotation(glasses_from_gameboard_rotation);

    let conversion = Quat::from_rotation_x(PI);
    let rotation = conversion * glasses_from_gameboard_rotation;
    let rotation = rotation.conjugate();
    let transform =
        Transform::from_rotation(rotation).with_translation(glasses_from_gameboard_position);

    (transform, transform_from_gameboard_to_glasses)
}
