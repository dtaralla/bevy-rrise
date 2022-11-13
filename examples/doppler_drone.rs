/*
 * Copyright (c) 2022 Contributors to the bevy-rrise project
 */

use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy_easings::{Ease, EaseMethod, EasingComponent, EasingType, EasingsPlugin};
use bevy_rrise::emitter_listener::{RrDynamicEmitterBundle, RrListener};
use bevy_rrise::plugin::RrisePlugin;
use rrise::game_syncs::SetRtpcValue;
use rrise::sound_engine::load_bank_by_name;
use rrise::{settings, AkGameObjectID, AkResult};
use std::path::PathBuf;

#[cfg(windows)]
use cc;

const SPEED_OF_SOUND: f32 = 340_f32;

// If you play with those, you might want to adapt the attenuation curve in the Wwise project
const TRAJECTORY_LENGTH: f32 = 90_f32; // expected to be positive
const TRAJECTORY_SPEED: f32 = 15_f32; // expected to be positive

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(LogPlugin {
            filter: "bevy_rrise=debug,wgpu=error".to_string(),
            ..default()
        }))
        .add_plugin(EasingsPlugin)
        .add_plugin(
            RrisePlugin::default().with_engine_settings(
                settings::AkInitSettings {
                    #[cfg(not(wwrelease))]
                    install_assert_hook: true,
                    ..default()
                }
                .with_plugin_dll_path(get_example_dll_path()),
            ),
        )
        .add_startup_system(setup_scene)
        .add_system(update.pipe(system_adapter::unwrap))
        .run();
}

/// Updates the Doppler effect RTPC of the drone & make the camera look at the drone
fn update(
    drone: Query<(&Children, &EasingComponent<Transform>, &GlobalTransform)>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
) -> Result<(), AkResult> {
    let mut rtpc = SetRtpcValue::new("Doppler", 0.0);

    let (children, easing, tfm) = drone.single();

    // Doppler effect computation: because the movement is 1D and the listener doesn't move,
    // computation is simplified greatly!
    let doppler_factor = if TRAJECTORY_SPEED >= SPEED_OF_SOUND {
        // corner case; max doppler effect when breaking sound barrier
        16_f32
    } else {
        SPEED_OF_SOUND
            / (SPEED_OF_SOUND
                - easing.direction() as i32 as f32
                    * TRAJECTORY_SPEED
                    * -tfm.translation().x.signum())
    };

    rtpc.with_value(doppler_factor);

    for c in children.iter() {
        rtpc.for_target(c.index() as AkGameObjectID).set()?;
    }

    // Make camera look at emitter
    let mut camera_tfm = camera.single_mut();
    camera_tfm.look_at(tfm.translation(), Vec3::Y);

    Ok(())
}

/// Setup scene and spawn drone emitter looping its position between a point and another
fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    default_listener: Query<Entity, With<RrListener>>,
) {
    // Setup cameras
    commands
        .spawn(Camera3dBundle {
            transform: Transform::from_xyz(0., 0., 15.).looking_at(Vec3::default(), Vec3::Y),
            ..default()
        })
        .add_child(default_listener.iter().next().unwrap());

    // Setup light
    commands.spawn(DirectionalLightBundle {
        directional_light: Default::default(),
        ..default()
    });

    // Setup path mesh
    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Box {
            min_x: -TRAJECTORY_LENGTH / 2.,
            max_x: TRAJECTORY_LENGTH / 2.,
            min_y: -0.1,
            max_y: 0.1,
            min_z: -2.1,
            max_z: -1.9,
        })),
        material: materials.add(Color::DARK_GRAY.into()),
        ..default()
    });

    if let Err(akr) = load_bank_by_name("TheBank.bnk") {
        panic!("Couldn't load TheBank: {}", akr);
    }

    // Setup mesh audio emitter
    commands
        .spawn((
            PbrBundle {
                mesh: meshes.add(Mesh::from(shape::Icosphere::default())),
                material: materials.add(Color::RED.into()),
                transform: Transform::from_xyz(-TRAJECTORY_LENGTH / 2., 0., -2.),
                ..default()
            },
            Transform::from_xyz(-TRAJECTORY_LENGTH / 2., 0., -2.).ease_to(
                Transform::from_xyz(TRAJECTORY_LENGTH / 2., 0., -2.),
                EaseMethod::Linear,
                EasingType::PingPong {
                    duration: std::time::Duration::from_secs_f32(
                        TRAJECTORY_LENGTH / TRAJECTORY_SPEED,
                    ),
                    pause: None,
                },
            ),
        ))
        .with_children(|parent| {
            // Attach dynamic emitter in the center of the parent
            parent.spawn(
                RrDynamicEmitterBundle::new(Vec3::default()).with_event("PlayDoppler", true),
            );
        });
}

fn get_example_dll_path() -> String {
    let wwise_sdk = PathBuf::from(std::env::var("WWISESDK").expect("env var WWISESDK not found"));

    let mut path;
    #[cfg(windows)]
    {
        let vs_version = cc::windows_registry::find_vs_version().expect("No MSVC install found");

        let wwise_vc = match vs_version {
            cc::windows_registry::VsVers::Vs14 => "x64_vc140",
            cc::windows_registry::VsVers::Vs15 => "x64_vc150",
            cc::windows_registry::VsVers::Vs16 => "x64_vc160",
            cc::windows_registry::VsVers::Vs17 => "x64_vc170",
            _ => panic!("Unsupported MSVC version: {:?}", vs_version),
        };
        path = wwise_sdk.join(wwise_vc);

        if !path.exists() {
            panic!(
                "Could not find {}.\n\
                You are using MSVC {:?} but the {} Wwise SDK target probably wasn't installed or \
                doesn't exist for this version of Wwise.\n\
                Note that Vs17 (Visual Studio 2022) is supported since Wwise 2021.1.10 only.",
                path.to_str().unwrap(),
                vs_version,
                wwise_vc
            )
        }
    }
    #[cfg(target_os = "linux")]
    {
        path = wwise_sdk.join("Linux_x64");
    }

    path = if cfg!(wwdebug) {
        path.join("Debug")
    } else if cfg!(wwrelease) {
        path.join("Release")
    } else {
        path.join("Profile")
    };

    // -- KNOWN ISSUE ON WINDOWS --
    // If WWISESDK contains spaces, the DLLs can't be discovered.
    // Help wanted!
    // Anyway, if you truly wanted to deploy something based on this crate with dynamic loading of
    // Wwise plugins, you would need to make sure to deploy any Wwise shared library (SO or DLL)
    // along your executable. You can't expect your players to have Wwise installed!
    // You can also just statically link everything, using this crate features. Enabling a feature
    // then forcing a rebuild will statically link the selected plugins instead of letting Wwise
    // look for their shared libraries at runtime.
    // Legal: Remember that Wwise is a licensed product, and you can't distribute their code,
    // statically linked or not, without a proper license.
    path.join("bin").into_os_string().into_string().unwrap()
}
