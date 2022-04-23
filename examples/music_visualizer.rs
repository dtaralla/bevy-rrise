/*
 * Copyright (c) 2022 Contributors to the bevy-rrise project
 */

use bevy::log::LogSettings;
use bevy::prelude::*;
use bevy_rrise::plugin::{CallbackChannel, RriseLabel, RrisePlugin};
use bevy_rrise::sound_engine::PostEventAtLocation;
use bevy_rrise::{rrise_setting, AkCallbackEvent};
use rrise::query_params::{get_rtpc_value, RtpcValueType};
use rrise::settings::AkInitSettings;
use rrise::sound_engine::load_bank_by_name;
use rrise::{AkCallbackInfo, AkCallbackType, AkResult, AkRtpcValue};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

fn main() {
    App::new()
        .insert_resource(LogSettings {
            filter: "bevy_rrise=debug,wgpu=error".to_string(),
            ..default()
        })
        .add_plugins(DefaultPlugins)
        .insert_resource(rrise_setting![AkInitSettings {
            #[cfg(not(wwrelease))]
            install_assert_hook: true,
            ..default()
        }
        .with_plugin_dll_path(get_example_dll_path())])
        .add_plugin(RrisePlugin)
        .insert_resource(Meters {
            meters: [
                (String::from("Meters_00"), 0.),
                (String::from("Meters_01"), 0.),
                (String::from("Meters_02"), 0.),
                (String::from("Meters_03"), 0.),
                (String::from("Meters_04"), 0.),
                (String::from("Meters_05"), 0.),
                (String::from("Meters_06"), 0.),
                (String::from("Meters_07"), 0.),
                (String::from("Meters_08"), 0.),
                (String::from("Meters_09"), 0.),
                (String::from("Meters_10"), 0.),
            ],
        })
        .add_startup_system(setup_scene)
        .add_startup_system(start_music.chain(error_handler))
        .add_system_to_stage(
            CoreStage::PreUpdate,
            audio_metering
                .chain(error_handler)
                .after(RriseLabel::RriseCallbackEventsPopulated),
        )
        .add_system(visualize_music)
        .add_system(update_beat_bar_times)
        .run();
}

fn error_handler(In(result): In<Result<(), AkResult>>) {
    if let Err(akr) = result {
        panic!("Unexpected error in system: {}", akr);
    }
}

#[derive(Component)]
struct BandMeter(usize);

#[derive(Component)]
struct BeatBarText;

type Meter = (String, AkRtpcValue);
struct Meters {
    meters: [Meter; 11],
}

fn audio_metering(mut meters: ResMut<Meters>) -> Result<(), AkResult> {
    for meter in &mut meters.meters {
        let rtpc_value = get_rtpc_value(meter.0.as_str(), None, None, RtpcValueType::Global(0.))?;
        meter.1 = match rtpc_value {
            RtpcValueType::Global(v) => v,
            _ => 0.,
        };
    }

    Ok(())
}

fn visualize_music(mut visual_meters: Query<(&mut Transform, &BandMeter)>, meters: Res<Meters>) {
    for (mut tfm, meter_index) in visual_meters.iter_mut() {
        let y_scale = 10. * (meters.meters[meter_index.0].1 + 48.) / 54.;
        tfm.scale = Vec3::new(1., y_scale, 1.);
    }
}

fn update_beat_bar_times(
    mut beat_bar_text: Query<&mut Text, With<BeatBarText>>,
    time: Res<Time>,
    mut wwise_events: EventReader<AkCallbackEvent>,
) {
    let mut beat_bar_text = beat_bar_text.single_mut();
    for AkCallbackEvent(cb_info) in wwise_events.iter() {
        match cb_info {
            AkCallbackInfo::MusicSync {
                music_sync_type: AkCallbackType::AK_MusicSyncBar,
                ..
            } => beat_bar_text.sections[1].value = format!("{:.1}s", time.seconds_since_startup()),
            AkCallbackInfo::MusicSync {
                music_sync_type: AkCallbackType::AK_MusicSyncBeat,
                ..
            } => beat_bar_text.sections[4].value = format!("{:.1}s", time.seconds_since_startup()),
            _ => (),
        };
    }
}

// Start music by spawning a static sound emitter
#[tracing::instrument(level = "debug", skip_all)]
fn start_music(cb_channel: Res<CallbackChannel>) -> Result<(), AkResult> {
    if let Err(akr) = load_bank_by_name("TheBank.bnk") {
        error!("Couldn't load TheBank: {}", akr);
        return Err(akr);
    }

    PostEventAtLocation::new("PlayMeteredMusic", Transform::default())
        .flags(AkCallbackType::AK_MusicSyncBeat | AkCallbackType::AK_MusicSyncBar)
        .post(Some(cb_channel.clone()))?;

    Ok(())
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // Setup cameras
    commands.spawn_bundle(PerspectiveCameraBundle {
        transform: Transform::from_xyz(0., 0., 15.).looking_at(Vec3::default(), Vec3::Y),
        ..default()
    });
    commands.spawn_bundle(UiCameraBundle::default());

    // Setup light
    commands.spawn_bundle(DirectionalLightBundle {
        directional_light: Default::default(),
        ..default()
    });

    // Setup audio band visualizers
    spawn_meter(0, "2a4858", &mut commands, &mut meshes, &mut materials);
    spawn_meter(1, "225b6c", &mut commands, &mut meshes, &mut materials);
    spawn_meter(2, "106e7c", &mut commands, &mut meshes, &mut materials);
    spawn_meter(3, "008288", &mut commands, &mut meshes, &mut materials);
    spawn_meter(4, "00968e", &mut commands, &mut meshes, &mut materials);
    spawn_meter(5, "23aa8f", &mut commands, &mut meshes, &mut materials);
    spawn_meter(6, "4abd8c", &mut commands, &mut meshes, &mut materials);
    spawn_meter(7, "72cf85", &mut commands, &mut meshes, &mut materials);
    spawn_meter(8, "9cdf7c", &mut commands, &mut meshes, &mut materials);
    spawn_meter(9, "c9ee73", &mut commands, &mut meshes, &mut materials);
    spawn_meter(10, "fafa6e", &mut commands, &mut meshes, &mut materials);

    // Setup beat/bar displays
    let font = asset_server.load("fonts/FiraMono-Medium.ttf");
    commands
        .spawn_bundle(TextBundle {
            style: Style {
                align_self: AlignSelf::FlexEnd,
                position_type: PositionType::Absolute,
                position: Rect {
                    bottom: Val::Px(8.0),
                    right: Val::Px(8.0),
                    ..default()
                },
                ..default()
            },
            text: Text {
                alignment: TextAlignment {
                    horizontal: HorizontalAlign::Right,
                    ..default()
                },
                sections: vec![
                    TextSection {
                        value: "Last beat: ".to_string(),
                        style: TextStyle {
                            font: font.clone(),
                            ..default()
                        },
                    },
                    TextSection {
                        value: "0.0s".to_string(),
                        style: TextStyle {
                            font: font.clone(),
                            ..default()
                        },
                    },
                    TextSection {
                        value: "\n".to_string(),
                        style: TextStyle {
                            font: font.clone(),
                            ..default()
                        },
                    },
                    TextSection {
                        value: "Last bar: ".to_string(),
                        style: TextStyle {
                            font: font.clone(),
                            ..default()
                        },
                    },
                    TextSection {
                        value: "0.0s".to_string(),
                        style: TextStyle {
                            font: font.clone(),
                            ..default()
                        },
                    },
                ],
                ..default()
            },
            ..default()
        })
        .insert(BeatBarText);
}

fn spawn_meter(
    index: usize,
    hex: &str,
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    commands
        .spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Box {
                min_x: 0.0,
                min_y: 0.0,
                min_z: 0.0,
                max_x: 0.25,
                max_y: 1.0,
                max_z: 0.25,
            })),
            material: materials.add(Color::hex(hex).unwrap().into()),
            transform: Transform::from_xyz(-5. + (index as f32), -5., 0.),
            ..default()
        })
        .insert(BandMeter(index));
}

fn get_example_dll_path() -> String {
    let wwise_sdk = PathBuf::from(std::env::var("WWISESDK").expect("env var WWISESDK not found"));

    let mut path;
    #[cfg(windows)]
    {
        path = wwise_sdk.join("x64_vc160");
        if !path.is_dir() {
            path = wwise_sdk.join("x64_vc150");
            if !path.is_dir() {
                path = wwise_sdk.join("x64_vc140");
            }
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
