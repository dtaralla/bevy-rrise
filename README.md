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

Just add the plugin settings as resources to your Bevy app, then add the plugin itself. That's it, you can now spawn 
`RrEmitter` components, `RrEmitterBundle`s or `RrDynamicEmitterBundle`s!

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Use Rrise with default settings 
        .add_plugin(RrisePlugin)
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
        .spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Icosphere::default())),
            material: materials.add(Color::RED.into()),
            ..default()
        })
        .with_children(|parent| {
            // Attach dynamic emitter in the center of the parent
            parent.spawn_bundle(
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

Don't hesitate to enable logging at the debug level for bevy-rrise to get an idea of what's happening under the hood!
It can also help diagnose why your sounds might not work.

## Bevy Compat Table

| Bevy | rrise | bevy-rrise |
|:----:|:-----:|:----------:|
| 0.7  |  0.2  |    0.1     |

### Legal stuff
Wwise and the Wwise logo are trademarks of Audiokinetic Inc., registered in the U.S. and other countries.

This project is in no way affiliated to Audiokinetic.

You still need a licensed version of Wwise installed to compile and run this project. You need a valid Wwise license
to distribute any project based on this crate.