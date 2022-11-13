# bevy-rrise

[![Crates.io](https://img.shields.io/crates/v/bevy-rrise.svg)](https://crates.io/crates/bevy-rrise)
[![MIT/Apache 2.0](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](./LICENSE)
[![Crates.io](https://img.shields.io/crates/d/bevy-rrise.svg)](https://crates.io/crates/bevy-rrise)

## What is bevy-rrise?
It's a plugin for the [Bevy](https://bevyengine.org/) engine that integrates the 
[Wwise](https://www.audiokinetic.com/en/products/wwise) sound engine.

It relies on my [Rrise](https://github.com/dtaralla/rrise) crate for the Rust bindings of Wwise.

**PRs welcomed!**

## Usage
**First, take a look at the system requirements for [Rrise](https://github.com/dtaralla/rrise)**: they are the same 
for bevy-rrise!

Definitely take a look at the [examples](/examples) for the best way to learn how this crate works.
To be able to compile and run the examples, you should generate the example Wwise project soundbanks first 
(located in [examples/WwiseProject](/examples/WwiseProject)).

Examples also show how you can use the `rrise_headers::rr` auto-generated module to get your events, busses
etc defined as Rust constants (generated from the soundbank definition files). More info
[here](https://github.com/dtaralla/rrise/tree/main/rrise-headers).

To start using the plugin, just add it to your Bevy app. That's it, you can now spawn 
`RrEmitter` components, `RrEmitterBundle`s or `RrDynamicEmitterBundle`s!

```rust
// ... 'use' directives...

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Use Rrise with default settings 
        .add_plugin(RrisePlugin::default())
        .add_startup_system(setup_scene)
        .add_system(update)
        .run();
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Load soundbank containing our PlayHelloWorld event structure and media
    if let Err(akr) = load_bank_by_name("TheBank.bnk") {
        panic!("Couldn't load TheBank: {}", akr);
    }

    // Setup mesh audio emitter
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Icosphere::default())),
            material: materials.add(Color::RED.into()),
            ..default()
        })
        .with_children(|parent| {
            // Attach dynamic emitter in the center of the parent
            parent.spawn(
                RrDynamicEmitterBundle::new(Vec3::default())
                    .with_event("PlayHelloWorld", true),
            );
        });

    // ... setup rest of scene
}

fn update(/* ... */) {
    // ... update scene
}
```

`RrDynamicEmitterBundle` (or `RrEmitter` components sitting on entities with a `TransformBundle`) will get their 
transform updates forwarded to Wwise automatically.

If they have a `bevy:core::Name` component, emitters will send their entity's name to Wwise for easy monitoring.

Before you can hear anything, make sure you generate your soundbanks from the Wwise authoring tool and place the 
resulting assets where bevy-rrise can find them. Again, look at how the provided examples are configured to get 
started 😉

Don't hesitate to enable logging at the debug level for bevy-rrise to get an idea of what's happening under the hood!
It can also help diagnose why your sounds might not be working.

### Plugin Configuration
Starting in v0.2.1 (and Bevy 0.9), configuration is more ergonomic than ever!
Start from `RrisePlugin::default()`, then use a chain of any of the `with_*_settings(...)` functions to customize the
sound engine & plugin behaviors:
- `with_plugin_settings(RriseBasicSettings)`
- `with_mem_settings(AkMemSettings)`
- `with_music_settings(AkMusicSettings)`
- `with_engine_settings(AkInitSettings)`
- `with_stream_settings(AkStreamMgrSettings)`
- `with_dev_settings(AkDeviceSettings)`
- `with_platform_settings(AkPlatformInitSettings)`
- `with_comms_settings(AkCommSettings)`

All `*Settings` classes have detailed documentation for all their members and implement the `Default` trait, for 
maximum ergonomics when you want to override just a handful of values.

## Bevy Compat Table

| Bevy | rrise | bevy-rrise |
|:----:|:-----:|:----------:|
| 0.7  |  0.2  |    0.1     |
| 0.8  |  0.2  |    0.2     |
| 0.9  |  0.2  |   0.2.1    |

### Legal stuff
Wwise and the Wwise logo are trademarks of Audiokinetic Inc., registered in the U.S. and other countries.

This project is in no way affiliated to Audiokinetic.

You still need a licensed version of Wwise installed to compile and run this project. You need a valid Wwise license
to distribute any project based on this crate.
