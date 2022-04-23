/*
 * Copyright (c) 2022 Contributors to the bevy-rrise project
 */

use crate::emitter_listener::{
    despawn_silent_emitters, init_new_rr_objects, stop_destroyed_emitters,
    update_emitters_position, RrListenerBundle,
};
use crate::AkCallbackEvent;
use bevy::app::AppExit;
use bevy::asset::{AssetServerSettings, FileAssetIo};
use bevy::prelude::*;
use crossbeam_channel::{Receiver, Sender};
use rrise::settings::*;
use rrise::AkResult::AK_Fail;
use rrise::*;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[macro_export]
/// Shorthand for creating `Rrise*Settings` [`Res<T>`] types.
macro_rules! rrise_setting {
    ($setting:expr) => {
        Arc::new(RwLock::new($setting))
    };
}

pub type RriseMemSettings = Arc<RwLock<AkMemSettings>>;
pub type RriseStreamMgrSettings = Arc<RwLock<AkStreamMgrSettings>>;
pub type RriseDeviceSettings = Arc<RwLock<AkDeviceSettings>>;
pub type RriseSoundEngineSettings = Arc<RwLock<AkInitSettings>>;
pub type RriseSoundEnginePlatformSettings = Arc<RwLock<AkPlatformInitSettings>>;
pub type RriseMusicSettings = Arc<RwLock<AkMusicSettings>>;
#[cfg(not(wwrelease))]
pub type RriseCommSettings = Arc<RwLock<AkCommSettings>>;

#[derive(Default)]
pub struct RrisePlugin;

#[derive(Debug, Clone)]
/// Plugin settings
pub struct RrisePluginSettings {
    /// One of the languages supported by your Wwise project in Project > Languages.
    ///
    /// Defaults to English(US).
    pub init_language: String,

    /// Generated soundbanks location, relative to the Bevy asset server folder.
    /// If the path given is absolute, overrides the asset server folder by that one.
    ///
    /// Don't add the platform folder; just the root where to expect to find the Windows or Linux
    /// folder containing the banks.
    pub banks_location: PathBuf,

    /// Whether to create a default listener automatically.
    ///
    /// If this is `true`, it is available after [RriseLabel::RriseReady].
    ///
    /// You can query it with `Query<&RrListener, Added<RrListener>>` if you want to attach it to
    /// your camera or avatar for instance.
    /// ```rust
    /// use bevy::prelude::*;
    /// use bevy::render::camera::Camera3d;
    /// use bevy_rrise::emitter_listener::RrListener;
    /// fn attach_default_listeners_to_camera(
    ///     mut cmds: Commands,
    ///     listeners: Query<(Entity, &RrListener), Added<RrListener>>,
    ///     main_camera: Query<Entity, With<Camera3d>>,
    /// ) {
    ///     let main_camera = main_camera.single();
    ///     for (entity, listener) in listeners.iter() {
    ///         cmds.entity(main_camera).add_child(entity);
    ///     }
    /// }
    /// ```
    pub spawn_default_listener: bool,
}

impl Default for RrisePluginSettings {
    /// Sets `default_language` to `English(US)` and `banks_location` to `soundbanks`, expecting
    /// soundbanks files to be in `[cargo dir OR exe directory]/assets/soundbanks/[Platform]`.
    fn default() -> Self {
        Self {
            init_language: "English(US)".to_string(),
            banks_location: PathBuf::from("soundbanks"),
            spawn_default_listener: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemLabel)]
pub enum RriseLabel {
    /// After this in [StartupStage::PreStartup], it is safe to call bevy-rrise APIs and Rrise raw
    /// APIs until Bevy's [AppExit] event is emitted.
    SoundEngineInitialized,

    /// After this in [StartupStage::PreStartup], you can consider the Init.bnk loaded and a possible
    /// default [RrListenerBundle] spawned until Bevy's [AppExit] event is emitted.
    ///
    /// *See also* [RrisePluginSettings::spawn_default_listener]
    RriseReady,

    /// After this in [CoreStage::PreUpdate], the EventReader<AkCallbackEvent> systems that run this
    /// frame will be populated with Rrise callbacks that occurred since the last execution of this label.
    RriseCallbackEventsPopulated,

    /// This marks the moment in the frame's [CoreStage::PostUpdate] where the sound engine gets
    /// terminated if an [AppExit] event occurred. It is not safe to call bevy-rrise APIs and Rrise
    /// raw APIs from now on.
    RriseMightBeTerminated,
}

impl Plugin for RrisePlugin {
    fn build(&self, app: &mut App) {
        app.world
            .get_resource_or_insert_with(RriseMemSettings::default);
        app.world
            .get_resource_or_insert_with(RriseStreamMgrSettings::default);
        app.world
            .get_resource_or_insert_with(RriseDeviceSettings::default);
        app.world.get_resource_or_insert_with(|| {
            rrise_setting![AkInitSettings {
                install_assert_hook: true,
                ..default()
            }]
        });
        app.world
            .get_resource_or_insert_with(RriseSoundEnginePlatformSettings::default);
        app.world
            .get_resource_or_insert_with(RriseMusicSettings::default);
        app.world
            .get_resource_or_insert_with(RrisePluginSettings::default);
        #[cfg(not(wwrelease))]
        app.world
            .get_resource_or_insert_with(RriseCommSettings::default);

        app.add_event::<AkCallbackEvent>()
            .insert_resource(CallbackChannel::new())
            .add_startup_system_to_stage(
                StartupStage::PreStartup,
                init_sound_engine
                    .chain(error_handler)
                    .label(RriseLabel::SoundEngineInitialized),
            )
            .add_startup_system_to_stage(
                StartupStage::PreStartup,
                setup_audio
                    .chain(error_handler)
                    .after(RriseLabel::SoundEngineInitialized)
                    .label(RriseLabel::RriseReady),
            )
            .add_system_to_stage(
                CoreStage::PreUpdate,
                init_new_rr_objects
                    .chain(error_handler)
                    .before(RriseLabel::RriseCallbackEventsPopulated),
            )
            .add_system_to_stage(
                CoreStage::PreUpdate,
                process_callbacks.label(RriseLabel::RriseCallbackEventsPopulated),
            )
            .add_system_to_stage(
                CoreStage::PostUpdate,
                stop_destroyed_emitters
                    .chain(error_handler)
                    .before("Rrise_despawn_silent_emitters"), // No need to stop silent emitters despawned this frame
            )
            .add_system_to_stage(
                CoreStage::PostUpdate,
                despawn_silent_emitters
                    .chain(error_handler)
                    .label("Rrise_despawn_silent_emitters"),
            )
            .add_system_to_stage(
                CoreStage::PostUpdate,
                update_emitters_position
                    .chain(error_handler)
                    .after("Rrise_despawn_silent_emitters"), // No need to stop silent emitters despawned this frame,
            )
            .add_system_to_stage(
                CoreStage::Last,
                audio_rendering
                    .chain(error_handler)
                    .label(RriseLabel::RriseMightBeTerminated),
            );
    }
}

#[derive(Clone)]
/// Resource to query in systems where you want to post callback-enabled events.
///
/// *See also* [RrEmitter::post_associated_event()](crate::emitter_listener::RrEmitter::post_associated_event())
pub struct CallbackChannel {
    pub(crate) sender: Sender<AkCallbackInfo>,
    receiver: Receiver<AkCallbackInfo>,
}

impl CallbackChannel {
    fn new() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self { sender, receiver }
    }
}

fn error_handler(In(result): In<Result<(), AkResult>>) {
    if let Err(akr) = result {
        error!("Unexpected Wwise error: {}", akr);
    }
}

// This system must be called late enough to maximize the chances to catch the AppExit event.
// See https://docs.rs/bevy/latest/bevy/app/struct.AppExit.html
fn audio_rendering(exits: EventReader<AppExit>) -> Result<(), AkResult> {
    if !sound_engine::is_initialized() {
        Ok(())
    } else if !exits.is_empty() {
        term_sound_engine()
    } else {
        const ALLOW_SYNC_RENDER: bool = true;
        sound_engine::render_audio(ALLOW_SYNC_RENDER)
    }
}

fn process_callbacks(callback_channel: Res<CallbackChannel>, mut ew: EventWriter<AkCallbackEvent>) {
    while let Ok(cb_info) = callback_channel.receiver.try_recv() {
        ew.send(AkCallbackEvent(cb_info));
    }
}

fn setup_audio(mut commands: Commands) -> Result<(), AkResult> {
    // Load Init.bnk - always required!
    if let Err(akr) = sound_engine::load_bank_by_name("Init.bnk") {
        error!("Init.bnk could not be loaded; there will be no audio. Make sure you generate all soundbanks before running");
        return Err(akr);
    }

    // Setup default listener
    let mut entity_cmds = commands.spawn_bundle(RrListenerBundle::default());
    #[cfg(not(wwrelease))]
    entity_cmds.insert(Name::new("RrMainDefaultListener"));

    Ok(())
}

#[cfg_attr(target_os = "linux", allow(unused_variables))]
#[allow(clippy::too_many_arguments)]
#[tracing::instrument(level = "debug", skip_all)]
fn init_sound_engine(
    mem_settings: Res<Arc<RwLock<AkMemSettings>>>,
    stream_settings: Res<Arc<RwLock<AkStreamMgrSettings>>>,
    device_settings: Res<Arc<RwLock<AkDeviceSettings>>>,
    sound_engine_settings: Res<Arc<RwLock<AkInitSettings>>>,
    sound_engine_platform_settings: Res<Arc<RwLock<AkPlatformInitSettings>>>,
    music_settings: Res<Arc<RwLock<AkMusicSettings>>>,
    #[cfg(not(wwrelease))] comms_settings: Res<Arc<RwLock<AkCommSettings>>>,
    plugin_settings: Res<RrisePluginSettings>,
    asset_server_settings: Res<AssetServerSettings>,
    windows: Res<Windows>,
) -> Result<(), AkResult> {
    // init memorymgr
    memory_mgr::init(&mut mem_settings.write().unwrap())?;
    assert!(memory_mgr::is_initialized());
    debug!("Memory manager initialized");

    // init streamingmgr
    #[cfg(target_os = "windows")]
    let platform = "Windows";
    #[cfg(target_os = "linux")]
    let platform = "Linux";
    let mut gen_banks_folder = plugin_settings.banks_location.join(platform);
    if gen_banks_folder.is_relative() {
        gen_banks_folder = FileAssetIo::get_root_path()
            .join(&asset_server_settings.asset_folder)
            .join(gen_banks_folder);
    }
    stream_mgr::init_default_stream_mgr(
        &stream_settings.read().unwrap(),
        &mut device_settings.write().unwrap(),
        gen_banks_folder.as_os_str().to_str().unwrap(),
    )?;
    debug!("Default streaming manager initialized");

    stream_mgr::set_current_language(&plugin_settings.init_language)?;
    debug!("Current language set");

    // init soundengine
    {
        #[cfg(windows)]
        // Find the Bevy window and register it as owner of the sound engine
        if let Some(w) = windows.iter().next() {
            use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

            sound_engine_platform_settings.write().unwrap().h_wnd.store(
                match unsafe { w.raw_window_handle().get_handle().raw_window_handle() } {
                    #[cfg(windows)]
                    RawWindowHandle::Win32(h) => h.hwnd,
                    other => {
                        panic!("Unexpected window handle: {:?}", other)
                    }
                },
                std::sync::atomic::Ordering::SeqCst,
            );
        }

        sound_engine::init(
            &mut sound_engine_settings.write().unwrap(),
            &mut sound_engine_platform_settings.write().unwrap(),
        )?;
    }
    debug!("Internal sound engine initialized");

    // init musicengine
    music_engine::init(&mut music_settings.write().unwrap())?;
    debug!("Internal music engine initialized");

    // init comms
    #[cfg(not(wwrelease))]
    {
        communication::init(&comms_settings.read().unwrap())?;
        debug!("Profiling (comms) initialized");
    }

    if !sound_engine::is_initialized() {
        error!("Unknown error: the sound engine didn't initialize properly");
        Err(AK_Fail)
    } else {
        Ok(())
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn term_sound_engine() -> Result<(), AkResult> {
    sound_engine::stop_all(None);
    sound_engine::unregister_all_game_obj()?;
    debug!("All objects stopped and unregistered");

    // term comms
    #[cfg(not(wwrelease))]
    {
        communication::term();
        debug!("Profiling (comms) terminated");
    }

    // term spatial

    // term music
    music_engine::term();
    debug!("Internal music engine terminated");

    // term soundengine
    sound_engine::term();
    debug!("Internal sound engine terminated");

    // term streamingmgr
    stream_mgr::term_default_stream_mgr();
    debug!("Streaming manager terminated");

    // term memorymgr
    memory_mgr::term();
    debug!("Memory manager terminated");

    Ok(())
}
