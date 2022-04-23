/*
 * Copyright (c) 2022 Contributors to the bevy-rrise project
 */

use bevy::prelude::*;
use rrise::sound_engine::{
    register_game_obj, set_position, unregister_game_obj, PostEvent as RPostEvent,
};

use crate::plugin::CallbackChannel;
use crate::ToAkTransform;
use rrise::AkTransform;
pub use rrise::{AkCallbackInfo, AkCallbackType, AkGameObjectID, AkID, AkPlayingID, AkResult};
use tracing::{debug, error};

pub struct SoundEngine {}

/// Helper struct to post events in a fire & forget fashion
pub struct PostEventAtLocation<'a> {
    inner: RPostEvent<'a>,
    has_flags: bool,
    tmp_id: AkGameObjectID,
    at: AkTransform,
}

impl<'a> PostEventAtLocation<'a> {
    /// Selects an event by name or by ID, to play at a given location
    pub fn new<T: Into<AkID<'a>>, U: ToAkTransform>(event_id: T, at: U) -> Self {
        // Trick found in the Wwise Unreal Integration... it's worth what it's worth!
        let tmp_id = (&event_id as *const T) as AkGameObjectID;

        Self {
            inner: RPostEvent::new(tmp_id, event_id),
            has_flags: false,
            tmp_id,
            at: at.to_ak_transform(),
        }
    }

    /// Add flags before posting. Bitmask: see [AkCallbackType].
    pub fn add_flags(&mut self, flags: AkCallbackType) -> &mut Self {
        self.has_flags = flags.0 > AkCallbackType(0).0;
        self.inner.add_flags(flags);
        self
    }

    /// Set flags before posting. Bitmask: see [AkCallbackType]
    pub fn flags(&mut self, flags: AkCallbackType) -> &mut Self {
        self.has_flags = flags.0 > AkCallbackType(0).0;
        self.inner.flags(flags);
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    /// Posts the event to the sound engine.
    ///
    /// Provide a clone of the [`Res<CallbackChannel>`] resource if you want to receive callbacks
    /// from Wwise (see [Self::flags()], [Self::add_flags()]).
    pub fn post(&mut self, cb_channel: Option<CallbackChannel>) -> Result<AkPlayingID, AkResult> {
        register_game_obj(self.tmp_id)?;
        set_position(self.tmp_id, self.at)?;
        debug!("Registered tmp Wwise emitter {}", self.tmp_id);

        let post_result = match (self.has_flags, cb_channel) {
            (false, _) => self.inner.post(),
            (true, None) => {
                if self.has_flags {
                    warn!(
                        "Event {:?} wants callbacks but didn't pass a World; you won't receive bevy events for it",
                        self.inner,
                    )
                }
                self.inner.post()
            }
            (true, Some(cb_channel)) => {
                // self.inner.add_flags(AkCallbackType::AK_EndOfEvent);
                self.inner.post_with_callback(move |cb_info| {
                    if cb_channel.sender.try_send(cb_info.clone()).is_err() {
                        warn!("Could not send {:?}", cb_info);
                    }

                    // Clean static maps of playing IDs?
                    // if let AkCallbackInfo::Event {
                    //     callback_type: AkCallbackType::AK_EndOfEvent,
                    //     ..
                    // } = cb_info
                    // {
                    //     // clean...
                    // }
                })
            }
        };
        if let Err(akr) = unregister_game_obj(self.tmp_id) {
            error!(
                "Couldn't unregister Wwise emitter {}; this might be a leak - {}",
                self.tmp_id, akr
            );
        } else {
            debug!("Unregistered tmp Wwise emitter {}", self.tmp_id);
        }
        post_result
    }
}
