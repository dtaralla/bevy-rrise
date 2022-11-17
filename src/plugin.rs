/*
 * Copyright (c) 2022 Contributors to the bevy-rrise project
 */

use crate::emitter_listener::{
    despawn_silent_emitters, init_new_rr_objects, stop_destroyed_emitters, update_rr_position,
    RrListenerBundle,
};
use crate::AkCallbackEvent;
use bevy::app::AppExit;
use bevy::asset::FileAssetIo;
use bevy::prelude::*;
use crossbeam_channel::{Receiver, Sender};
use rrise::settings::*;
use rrise::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

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

#[derive(Debug, Clone)]
/// Plugin basic settings
pub struct RriseBasicSettings {
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

impl Default for RriseBasicSettings {
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

struct PluginSettingsInternal {
    bevy_asset_folder: String,
    plugin: RriseBasicSettings,
    mem: AkMemSettings,
    stream: RefCell<AkStreamMgrSettings>,
    dev: RefCell<AkDeviceSettings>,
    engine: RefCell<AkInitSettings>,
    pltfm: RefCell<AkPlatformInitSettings>,
    music: AkMusicSettings,
    #[cfg(not(wwrelease))]
    comms: AkCommSettings,
}

impl Default for PluginSettingsInternal {
    fn default() -> Self {
        Self {
            bevy_asset_folder: default(),
            plugin: default(),
            mem: default(),
            stream: default(),
            dev: default(),
            engine: RefCell::new(AkInitSettings {
                install_assert_hook: true,
                ..default()
            }),
            pltfm: default(),
            music: default(),
            #[cfg(not(wwrelease))]
            comms: default(),
        }
    }
}

// SAFETY
// PluginSettingsInternal is not meant to be accessed by any systems other than init_sound_engine().
// Plus, it's an init system, so there is no way this structure is going to be accessed from
// more than 1 thread at a time.
unsafe impl Sync for PluginSettingsInternal {}

#[derive(Deref, Resource)]
struct PluginSettingsResource(Arc<RwLock<PluginSettingsInternal>>);

pub struct RrisePlugin(Arc<RwLock<PluginSettingsInternal>>);

impl Default for RrisePlugin {
    fn default() -> Self {
        Self(Arc::new(RwLock::new(PluginSettingsInternal::default())))
    }
}

impl RrisePlugin {
    pub fn new() -> Self {
        default()
    }

    #[allow(unused_mut)]
    pub fn with_plugin_settings(mut self, settings: RriseBasicSettings) -> Self {
        self.0.write().unwrap().plugin = settings;
        self
    }

    #[allow(unused_mut)]
    pub fn with_mem_settings(mut self, settings: AkMemSettings) -> Self {
        self.0.write().unwrap().mem = settings;
        self
    }

    #[allow(unused_mut)]
    pub fn with_music_settings(mut self, settings: AkMusicSettings) -> Self {
        self.0.write().unwrap().music = settings;
        self
    }

    #[allow(unused_mut)]
    pub fn with_engine_settings(mut self, settings: AkInitSettings) -> Self {
        self.0.write().unwrap().engine = RefCell::new(settings);
        self
    }

    #[allow(unused_mut)]
    pub fn with_stream_settings(mut self, settings: AkStreamMgrSettings) -> Self {
        self.0.write().unwrap().stream = RefCell::new(settings);
        self
    }

    #[allow(unused_mut)]
    pub fn with_dev_settings(mut self, settings: AkDeviceSettings) -> Self {
        self.0.write().unwrap().dev = RefCell::new(settings);
        self
    }

    #[allow(unused_mut)]
    pub fn with_platform_settings(mut self, settings: AkPlatformInitSettings) -> Self {
        self.0.write().unwrap().pltfm = RefCell::new(settings);
        self
    }

    #[cfg(not(wwrelease))]
    #[allow(unused_mut)]
    pub fn with_comms_settings(mut self, settings: AkCommSettings) -> Self {
        self.0.write().unwrap().comms = settings;
        self
    }
}

impl Plugin for RrisePlugin {
    fn build(&self, app: &mut App) {
        let plugin_settings = PluginSettingsResource(self.0.clone());

        if plugin_settings
            .read()
            .unwrap()
            .plugin
            .banks_location
            .is_relative()
        {
            let asset_folder = &app
                .get_added_plugins::<AssetPlugin>()
                .first()
                .expect("AssetPlugin must be inserted before Rrise if banks_location setting is a relative path")
                .asset_folder;
            plugin_settings.write().unwrap().bevy_asset_folder = asset_folder.clone();
        }

        app.add_event::<AkCallbackEvent>()
            .insert_resource(plugin_settings)
            .insert_resource(CallbackChannel::new())
            .add_startup_system_to_stage(
                StartupStage::PreStartup,
                init_sound_engine
                    .pipe(error_handler)
                    .label(RriseLabel::SoundEngineInitialized),
            )
            .add_startup_system_to_stage(
                StartupStage::PreStartup,
                setup_audio
                    .pipe(error_handler)
                    .after(RriseLabel::SoundEngineInitialized)
                    .label(RriseLabel::RriseReady),
            )
            .add_system_to_stage(
                CoreStage::PreUpdate,
                init_new_rr_objects
                    .pipe(error_handler)
                    .before(RriseLabel::RriseCallbackEventsPopulated),
            )
            .add_system_to_stage(
                CoreStage::PreUpdate,
                process_callbacks.label(RriseLabel::RriseCallbackEventsPopulated),
            )
            .add_system_to_stage(
                CoreStage::PostUpdate,
                stop_destroyed_emitters
                    .pipe(error_handler)
                    .before("Rrise_despawn_silent_emitters"), // No need to stop silent emitters despawned this frame
            )
            .add_system_to_stage(
                CoreStage::PostUpdate,
                despawn_silent_emitters
                    .pipe(error_handler)
                    .label("Rrise_despawn_silent_emitters"),
            )
            .add_system_to_stage(
                CoreStage::PostUpdate,
                update_rr_position
                    .pipe(error_handler)
                    .after("Rrise_despawn_silent_emitters"), // No need to stop silent emitters despawned this frame,
            )
            .add_system_to_stage(
                CoreStage::Last,
                audio_rendering
                    .pipe(error_handler)
                    .label(RriseLabel::RriseMightBeTerminated),
            );
    }
}

#[derive(Clone, Resource)]
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

fn setup_audio(
    mut commands: Commands,
    settings: Res<PluginSettingsResource>,
) -> Result<(), AkResult> {
    // Load Init.bnk - always required!
    if let Err(akr) = sound_engine::load_bank_by_name("Init.bnk") {
        error!("Init.bnk could not be loaded; there will be no audio. Make sure you generate all soundbanks before running");
        return Err(akr);
    }

    // Setup default listener
    if settings.read().unwrap().plugin.spawn_default_listener {
        let mut entity_cmds = commands.spawn(RrListenerBundle::default());
        #[cfg(not(wwrelease))]
        entity_cmds.insert(Name::new("RrMainDefaultListener"));
    }

    Ok(())
}

#[cfg_attr(target_os = "linux", allow(unused_variables))]
#[allow(clippy::too_many_arguments)]
#[tracing::instrument(level = "debug", skip_all)]
fn init_sound_engine(
    plugin_settings: ResMut<PluginSettingsResource>,
    windows: Res<Windows>,
) -> Result<(), AkResult> {
    let mut settings = plugin_settings.write().unwrap();

    // init memorymgr
    memory_mgr::init(&mut settings.mem)?;
    assert!(memory_mgr::is_initialized());
    debug!("Memory manager initialized");

    // init streamingmgr
    #[cfg(target_os = "windows")]
    let platform = "Windows";
    #[cfg(target_os = "linux")]
    let platform = "Linux";
    let mut gen_banks_folder = settings.plugin.banks_location.join(platform);
    if gen_banks_folder.is_relative() {
        gen_banks_folder = FileAssetIo::get_base_path()
            .join(&settings.bevy_asset_folder)
            .join(gen_banks_folder);
    }

    debug!("Banks will be discovered from: {:?}", gen_banks_folder);

    stream_mgr::init_default_stream_mgr(
        &settings.stream.borrow(),
        &mut settings.dev.borrow_mut(),
        gen_banks_folder.as_os_str().to_str().unwrap(),
    )?;
    debug!("Default streaming manager initialized");

    stream_mgr::set_current_language(&settings.plugin.init_language)?;
    debug!("Current language set");

    // init soundengine

    #[cfg(windows)]
    // Find the Bevy window and register it as owner of the sound engine
    if let Some(w) = windows.iter().next() {
        use raw_window_handle::RawWindowHandle;

        settings.pltfm.get_mut().h_wnd.store(
            match w.raw_handle().unwrap().window_handle {
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
        &mut settings.engine.borrow_mut(),
        &mut settings.pltfm.borrow_mut(),
    )?;
    debug!("Internal sound engine initialized");

    // init musicengine
    music_engine::init(&mut settings.music)?;
    debug!("Internal music engine initialized");

    // init comms
    #[cfg(not(wwrelease))]
    {
        communication::init(&settings.comms)?;
        debug!("Profiling (comms) initialized");
    }

    if !sound_engine::is_initialized() {
        error!("Unknown error: the sound engine didn't initialize properly");
        Err(AkResult::AK_Fail)
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
