/*
 * Copyright (c) 2022 Contributors to the bevy-rrise project
 */

#![doc = include_str!("../README.MD")]

use bevy::prelude::*;
use rrise::{AkCallbackInfo, AkTransform};

pub mod emitter_listener;
pub mod plugin;
pub mod sound_engine;

#[derive(Deref, DerefMut)]
pub struct AkCallbackEvent(pub AkCallbackInfo);

pub trait ToAkTransform {
    /// Constructs a Wwise transform based on a game engine transform
    fn to_ak_transform(&self) -> AkTransform;
}

#[doc(hidden)]
// Wwise uses a left-handed, Y up coordinate system.
// See https://www.audiokinetic.com/library/2021.1.7_7796/?source=SDK&id=soundengine_3dpositions.html#soundengine_3dpositions_xyz
macro_rules! ak_transform_impls {
    ($($type:ty),+) => {
        $(
            impl ToAkTransform for $type {
                fn to_ak_transform(&self) -> AkTransform {
                    let mut pos = self.translation.to_array();
                    pos[2] = -pos[2];
                    let mut ak_tfm = AkTransform::from_position(pos);
                    let mut front = self.forward().to_array();
                    front[2] = -front[2];
                    ak_tfm.orientationFront = front.into();
                    ak_tfm.orientationTop = self.up().to_array().into();
                    ak_tfm
                }
            }
        )*
    };
}

ak_transform_impls![Transform, GlobalTransform];
