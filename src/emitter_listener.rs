/*
 * Copyright (c) 2022 Contributors to the bevy-rrise project
 */

use crate::plugin::CallbackChannel;
use crate::ToAkTransform;
use bevy::math::Affine3A;
use bevy::prelude::*;
#[cfg(wwrelease)]
use rrise::sound_engine::register_game_obj;
#[cfg(not(wwrelease))]
use rrise::sound_engine::register_named_game_obj;
use rrise::sound_engine::{add_default_listener, set_position, stop_all, PostEvent};
use rrise::{
    AkCallbackInfo, AkCallbackType, AkGameObjectID, AkID, AkPlayingID, AkResult,
    AK_INVALID_PLAYING_ID,
};
use std::sync::{Arc, RwLock};
use tracing;

#[derive(Component)]
/// Marker for emitters that are registered in Wwise.
///
/// A [RrEmitter] sitting on the same entity than this is guaranteed to be registered.
pub struct RrRegistered;

#[derive(Debug, Component)]
/// Sound emitter configuration.
///
/// If its entity gets destroyed or this component gets removed, the events posted with it will be
/// stopped.
pub struct RrEmitter {
    /// The event to pre-set on this emitter.
    /// Defaults to no event (ie, `""`).
    ///
    /// See [`auto_post`](RrEmitter::auto_post)
    pub event_id: AkID<'static>,

    /// Mask describing which callbacks you want to subscribe to.
    /// Defaults to none (ie, `AkCallbackType(0)`).
    pub flags: AkCallbackType,

    /// Whether to auto post the associated event when this emitter gets registered.
    ///
    /// See [`event_id`](RrEmitter::event_id)
    pub auto_post: bool,

    /// Whether to automatically despawn the entity bearing this emitter when it is done playing.
    ///
    /// *Remark* "Done playing" = no more events are playing on it - work if several events got posted
    /// simultaneously with it.
    pub despawn_on_silent: bool,
    // pub stop_on_destroy: bool, // TODO
    pub(crate) playing_ids: Arc<RwLock<Vec<AkPlayingID>>>,
    pub(crate) entity: Option<Entity>,
}

#[derive(Bundle, Default)]
/// Static sound emitter. More optimized if you know it won't move.
///
/// If you attach a Transform to an entity created with RrEmitterBundle, it will behave like a
/// [RrDynamicEmitterBundle].
pub struct RrEmitterBundle {
    pub rr: RrEmitter,
    pub global_tfm: GlobalTransform,
}

#[derive(Bundle, Default)]
/// Dynamic sound emitter.
///
/// If you know that it will never move, or that it is not attached to a parent transform, use
/// [RrEmitterBundle] instead.
pub struct RrDynamicEmitterBundle {
    #[bundle]
    emitter: RrEmitterBundle,
    tfm: Transform,
}

#[derive(Debug, Component)]
/// Sound listener marker.
pub struct RrListener {
    is_default: bool,
    pub(crate) entity: Option<Entity>,
}

impl RrListener {
    pub fn new(is_default: bool) -> Self {
        Self {
            is_default,
            ..default()
        }
    }

    pub fn is_default(&self) -> bool {
        self.is_default
    }
}

impl Default for RrListener {
    fn default() -> Self {
        Self {
            is_default: true,
            entity: None,
        }
    }
}

#[derive(Bundle, Default)]
/// Sound listener.
///
/// You should attach this to a camera or your player avatar.
///
/// ### Example
/// You can create a system that attaches any newly created listeners to your main 3D camera:
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
pub struct RrListenerBundle {
    #[bundle]
    pub tfm: TransformBundle,
    pub listener: RrListener,
}

impl RrListenerBundle {
    /// Sets whether this listener is a default listener or not.
    ///
    /// Emitters that have not explicitly overridden their listener set are associated to the
    /// default listeners set.
    ///
    /// Defaults to `true`.
    pub fn with_is_default(mut self, is_default: bool) -> Self {
        self.listener.is_default = is_default;
        self
    }
}

impl Default for RrEmitter {
    /// Creates a pure emitter (no transform) that can later be used to post events on.
    ///
    /// Defaults to no event nor auto post, no callback flags, no despawn on silent.
    fn default() -> Self {
        Self {
            event_id: AkID::Name(""),
            flags: AkCallbackType::default(),
            auto_post: false,
            despawn_on_silent: false,
            // stop_on_destroy: true, // TODO
            playing_ids: Arc::new(RwLock::new(vec![])),
            entity: None,
        }
    }
}

impl RrEmitterBundle {
    /// Creates an emitter at `position`.
    pub fn new(position: Vec3) -> Self {
        Self {
            global_tfm: GlobalTransform::from_translation(position),
            ..default()
        }
    }

    /// Sets the rotation of this emitter.
    pub fn with_rotation(mut self, rotation: Quat) -> Self {
        self.global_tfm = GlobalTransform::from(Affine3A::from_rotation_translation(
            rotation,
            self.global_tfm.translation(),
        ));
        self
    }

    /// Sets the event to associate with this emitter and registers it for auto play.
    ///
    /// If `despawn_on_silent` is `true`, despawn this emitter once it has finished playing all its
    /// events.
    pub fn with_event<T: Into<AkID<'static>>>(mut self, event: T, despawn_on_silent: bool) -> Self {
        self.rr.event_id = event.into();
        self.rr.auto_post = true;
        self.rr.despawn_on_silent = despawn_on_silent;
        self
    }

    /// Sets the callback flags to associate with this emitter.
    pub fn with_flags(mut self, flags: AkCallbackType) -> Self {
        self.rr.flags = flags;
        self
    }

    // TODO
    // /// Sets whether to automatically stop the sounds emitted by this emitter when it gets destroyed.
    // ///
    // /// Defaults to `true`.
    // pub fn stop_on_destroy(mut self, stop_on_destroy: bool) -> Self {
    //     self.rr.stop_on_destroy = stop_on_destroy;
    //     self
    // }
}

impl RrDynamicEmitterBundle {
    /// Creates an emitter at `position`.
    pub fn new(position: Vec3) -> Self {
        Self {
            tfm: Transform::from_translation(position),
            // emitter.global_tfm will get updated by Bevy
            ..default()
        }
    }

    /// Creates an emitter from a [`Transform`].
    pub fn from_transform(at: Transform) -> Self {
        Self {
            tfm: at,
            // emitter.global_tfm will get updated by Bevy
            ..default()
        }
    }

    /// Sets the initial rotation of this emitter.
    pub fn with_rotation(mut self, rotation: Quat) -> Self {
        self.tfm = self.tfm.with_rotation(rotation);
        // self.emitter.global_tfm will get updated by Bevy
        self
    }

    /// Sets the event to associate to this emitter and registers it for auto play.
    pub fn with_event<T: Into<AkID<'static>>>(mut self, event: T, despawn_on_silent: bool) -> Self {
        self.emitter.rr.event_id = event.into();
        self.emitter.rr.auto_post = true;
        self.emitter.rr.despawn_on_silent = despawn_on_silent;
        self
    }

    /// Sets the callback flags to associate with this emitter.
    pub fn with_flags(mut self, flags: AkCallbackType) -> Self {
        self.emitter.rr.flags = flags;
        self
    }

    // TODO
    // /// Sets whether to automatically stop the sounds emitted by this emitter when it gets destroyed.
    // ///
    // /// Defaults to `true`.
    // pub fn stop_on_destroy(mut self, stop_on_destroy: bool) -> Self {
    //     self.emitter.rr.stop_on_destroy = stop_on_destroy;
    //     self
    // }
}

impl RrListenerBundle {
    /// Creates a listener at `position`.
    pub fn new(position: Vec3) -> Self {
        Self {
            tfm: TransformBundle {
                local: Transform::from_translation(position),
                // self.tfm.global will get updated by Bevy
                ..default()
            },
            ..default()
        }
    }

    /// Sets the rotation of this listener.
    pub fn with_rotation(mut self, rotation: Quat) -> Self {
        self.tfm.local = self.tfm.local.with_rotation(rotation);
        // self.tfm.global will get updated by Bevy
        self
    }
}

#[doc(hidden)]
macro_rules! post_event_internal {
    ($event_id:ident on $entity:ident with $flags:expr; store in $safe_playing_ids:ident; react with $cb_info:ident then { $($then:stmt)* }) => {
        PostEvent::new($entity.id() as AkGameObjectID, $event_id)
            .flags($flags | AkCallbackType::AK_EndOfEvent)
            .post_with_callback(move |$cb_info| {
                {
                    $($then)*
                }

                if let AkCallbackInfo::Event {
                    playing_id,
                    callback_type: AkCallbackType::AK_EndOfEvent,
                    ..
                } = $cb_info
                {
                    let mut lock = $safe_playing_ids.write().unwrap();
                    (*lock).retain(|&p_id| p_id != playing_id);
                };
            })
    };
    ($event_id:ident on $entity:ident with $flags:expr; store in $safe_playing_ids:ident) => {
        post_event_internal![$event_id on $entity with $flags; store in $safe_playing_ids; react with cb_info then {}]
    };
}

impl RrEmitter {
    /// Whether any events are playing on this emitter
    pub fn is_playing(&self) -> bool {
        !self.playing_ids.read().unwrap().is_empty()
    }

    /// Whether this component appears to be registered in Wwise.
    ///
    /// You can make sure of this by also querying for the [`RrRegistered`] component on your entities.
    pub fn is_registered(&self) -> bool {
        self.entity.is_some()
    }

    /// Stops all events currently playing on this emitter.
    pub fn stop(&self) {
        if let Some(entity) = self.entity {
            stop_all(Some(entity.id() as u64));
        }
    }

    /// Posts the event `self.event_id` using flags `self.flags`.
    ///
    /// If you pass [`None`] for `cb_channel`, you won't receive any [`AkCallbackEvent`](crate::AkCallbackEvent)
    /// in your [`EventReader`]s, even if you had some flags set in `self.flags`.
    ///
    /// See [`CallbackChannel`]
    pub fn post_associated_event(&mut self, cb_channel: Option<CallbackChannel>) -> AkPlayingID {
        self.post_event(self.event_id, self.flags, cb_channel)
    }

    /// Posts `event` using `flags` (**this method ignores `self.flags`**).
    ///
    /// If you pass [`None`] for `cb_channel`, you won't receive any [`AkCallbackEvent`](crate::AkCallbackEvent)
    /// in your [`EventReader`]s, even if you had some `flags`.
    ///
    /// See [`CallbackChannel`]
    pub fn post_event<'b, T: Into<AkID<'b>>>(
        &mut self,
        event: T,
        flags: AkCallbackType,
        cb_channel: Option<CallbackChannel>,
    ) -> AkPlayingID {
        if let Some(entity) = self.entity {
            let has_flags = flags.0 > AkCallbackType(0).0;
            let event = event.into();
            let safe_playing_ids = self.playing_ids.clone();
            let post_result = match (has_flags, cb_channel) {
                (false, _) => {
                    post_event_internal![
                        event on entity with flags;
                        store in safe_playing_ids]
                }
                (true, None) => {
                    warn!(
                        "Event {} on {:?} wants callbacks {} but didn't pass a CallbackChannel; you won't receive bevy events for it",
                        event,
                        self.entity,
                        flags,
                    );
                    post_event_internal![
                        event on entity with AkCallbackType(0);
                        store in safe_playing_ids]
                }
                (true, Some(cb_channel)) => {
                    post_event_internal![
                    event on entity with flags;
                    store in safe_playing_ids;
                    react with cb_info then {
                        if cb_channel.sender.try_send(cb_info.clone()).is_err() {
                            warn!("Could not send {:?}", cb_info);
                        }
                    }]
                }
            };

            match post_result {
                Ok(playing_id) => {
                    self.playing_ids.write().unwrap().push(playing_id);
                    playing_id
                }
                Err(akr) => {
                    error!("Couldn't post '{}' on {:?} - {}", event, self.entity, akr);
                    AK_INVALID_PLAYING_ID
                }
            }
        } else {
            error!("RrComponent is not yet registered: {:?}", self);
            AK_INVALID_PLAYING_ID
        }
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub(crate) fn init_new_rr_objects(
    mut commands: Commands,
    mut listeners: Query<
        (Entity, Option<&Name>, &mut RrListener, &GlobalTransform),
        Added<RrListener>,
    >,
    mut emitters: Query<
        (Entity, Option<&Name>, &mut RrEmitter, &GlobalTransform),
        Added<RrEmitter>,
    >,
    cb_channel: Res<CallbackChannel>,
) -> Result<(), AkResult> {
    // Always register listeners first
    // Otherwise, if the first listener was created in the same frame than an emitter with auto-post,
    // this emitter would have no listener and fail to post on the Wwise side.
    for (e, name, mut rr_l, &tfm) in listeners.iter_mut() {
        rr_l.entity = Some(e);
        let id = e.id() as AkGameObjectID;

        #[cfg(not(wwrelease))]
        {
            if let Err(akr) = register_named_game_obj(
                id,
                name.map(|n| n.as_str())
                    .unwrap_or(format!("RrListener_{}", e.id()).as_str()),
            ) {
                error!("Couldn't register listener {} - {}", id, akr);
                continue;
            }
        }

        #[cfg(wwrelease)]
        if let Err(akr) = register_game_obj(id) {
            error!("Couldn't register listener {:?} - {}", e, akr);
            continue;
        }

        if rr_l.is_default {
            if let Err(akr) = add_default_listener(id) {
                error!("Couldn't add default listener {:?} - {}", e, akr);
                continue;
            }
        }

        if let Err(akr) = set_position(id, tfm.to_ak_transform()) {
            error!("Couldn't set listener {:?} position - {}", e, akr);
            continue;
        }

        commands.entity(e).insert(RrRegistered);

        debug!("Listener {} now registered", id);
    }

    for (e, name, mut rr_e, &tfm) in emitters.iter_mut() {
        rr_e.entity = Some(e);
        let id = e.id() as AkGameObjectID;

        #[cfg(not(wwrelease))]
        {
            if let Err(akr) = register_named_game_obj(
                id,
                name.map(|n| n.as_str())
                    .unwrap_or(format!("RrEmitter_{}", e.id()).as_str()),
            ) {
                error!("Couldn't register emitter {} - {}", id, akr);
                continue;
            }
        }

        #[cfg(wwrelease)]
        if let Err(akr) = register_game_obj(id) {
            error!("Couldn't register emitter {:?} - {}", e, akr);
            continue;
        }

        if let Err(akr) = set_position(id, tfm.to_ak_transform()) {
            error!("Couldn't set emitter {:?} position - {}", e, akr);
            continue;
        }

        if rr_e.auto_post {
            rr_e.post_associated_event(Some(cb_channel.clone()));
        }

        commands.entity(e).insert(RrRegistered);

        debug!("Emitter {} now registered", id);
    }

    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub(crate) fn stop_destroyed_emitters(
    destroyed_emitters: RemovedComponents<RrEmitter>,
) -> Result<(), AkResult> {
    for e in destroyed_emitters.iter() {
        stop_all(Some(e.id() as AkGameObjectID));
        debug!("Stopped emitter {} because it got despawned", e.id());
    }

    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub(crate) fn despawn_silent_emitters(
    mut commands: Commands,
    emitters: Query<&RrEmitter, With<RrRegistered>>,
) -> Result<(), AkResult> {
    for rr in emitters.iter() {
        if rr.despawn_on_silent && rr.playing_ids.read().unwrap().is_empty() {
            commands.entity(rr.entity.unwrap()).despawn();
            debug!(
                "Despawned emitter {} because it became silent",
                rr.entity.unwrap().id()
            );
        }
    }

    Ok(())
}

#[allow(clippy::type_complexity)]
pub(crate) fn update_rr_position(
    mut emitters: Query<
        (&mut RrEmitter, &GlobalTransform),
        (With<RrRegistered>, Changed<GlobalTransform>),
    >,
    mut listeners: Query<
        (&mut RrListener, &GlobalTransform),
        (With<RrRegistered>, Changed<GlobalTransform>),
    >,
) -> Result<(), AkResult> {
    for (rr, &tfm) in emitters.iter_mut() {
        set_position(
            rr.entity.unwrap().id() as AkGameObjectID,
            tfm.to_ak_transform(),
        )?;
    }
    for (rr, &tfm) in listeners.iter_mut() {
        set_position(
            rr.entity.unwrap().id() as AkGameObjectID,
            tfm.to_ak_transform(),
        )?;
    }

    Ok(())
}
