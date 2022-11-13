/*
 * Copyright (c) 2022 Contributors to the bevy-rrise project
 */

use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy_rrise::plugin::{CallbackChannel, RriseLabel, RrisePlugin};
use bevy_rrise::sound_engine::PostEventAtLocation;
use bevy_rrise::AkCallbackEvent;
use rrise::query_params::{get_rtpc_value, RtpcValueType};
use rrise::settings;
use rrise::sound_engine::load_bank_by_name;
use rrise::{AkAuxBusID, AkCallbackInfo, AkCallbackType, AkResult, AkRtpcValue};
use rrise_headers::rr;
use std::path::PathBuf;

#[cfg(windows)]
use cc;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(LogPlugin {
            filter: "bevy_rrise=debug,wgpu=error".to_string(),
            ..default()
        }))
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
        .insert_resource(Meters {
            meters: [
                (rr::xbus::Meters_00, 0.),
                (rr::xbus::Meters_01, 0.),
                (rr::xbus::Meters_02, 0.),
                (rr::xbus::Meters_03, 0.),
                (rr::xbus::Meters_04, 0.),
                (rr::xbus::Meters_05, 0.),
                (rr::xbus::Meters_06, 0.),
                (rr::xbus::Meters_07, 0.),
                (rr::xbus::Meters_08, 0.),
                (rr::xbus::Meters_09, 0.),
                (rr::xbus::Meters_10, 0.),
            ],
        })
        .add_startup_system(setup_scene)
        .add_startup_system(start_music.pipe(error_handler))
        .add_system_to_stage(
            CoreStage::PreUpdate,
            audio_metering
                .pipe(error_handler)
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

type Meter = (AkAuxBusID, AkRtpcValue);

#[derive(Resource)]
struct Meters {
    meters: [Meter; 11],
}

fn audio_metering(mut meters: ResMut<Meters>) -> Result<(), AkResult> {
    for meter in &mut meters.meters {
        let rtpc_value = get_rtpc_value(meter.0, None, None, RtpcValueType::Global(0.))?;
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
            } => beat_bar_text.sections[1].value = format!("{:.1}s", time.elapsed_seconds_f64()),
            AkCallbackInfo::MusicSync {
                music_sync_type: AkCallbackType::AK_MusicSyncBeat,
                ..
            } => beat_bar_text.sections[4].value = format!("{:.1}s", time.elapsed_seconds_f64()),
            _ => (),
        };
    }
}

// Start music by spawning a static sound emitter
#[tracing::instrument(level = "debug", skip_all)]
fn start_music(cb_channel: Res<CallbackChannel>) -> Result<(), AkResult> {
    if let Err(akr) = load_bank_by_name(rr::bnk::TheBank) {
        error!("Couldn't load TheBank: {}", akr);
        return Err(akr);
    }

    PostEventAtLocation::new(rr::ev::PlayMeteredMusic, Transform::default())
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
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0., 0., 15.).looking_at(Vec3::default(), Vec3::Y),
        ..default()
    });

    // Setup light
    commands.spawn(DirectionalLightBundle {
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
        .spawn(TextBundle {
            style: Style {
                align_self: AlignSelf::FlexEnd,
                position_type: PositionType::Absolute,
                position: UiRect {
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
    commands.spawn((
        PbrBundle {
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
        },
        BandMeter(index),
    ));
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
