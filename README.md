# Hostile Neural Networks for Horizon

An unofficial port of [Hostile Neural Networks](https://github.com/Shadows-of-Fire/Hostile-Neural-Networks) 7.0.0 (NeoForge, MIT, by
Shadows_of_Fire) to [Horizon](https://github.com/horizon-rust/horizon), as one native mod. The Java mod was rewritten in Rust for Horizon's
hooks, so no original code runs. The textures, models, recipes and data models are the original mod's.

- `mixins/hostilenetworks`: the mod. `hostile.rs` is the logic, `data/hostilenetworks-content.hmod` its blocks, items, recipes and textures.
- `tools/make_content.py`: makes the content from the original jar. `tools/make_bundle.py` writes the bundle's `horizon-mixin.toml` and helpers.

## What's there

- Data models for every vanilla mob and block the mod lists (those for another mod's mobs wait on that mod, as on NeoForge), attuned with a Model Framework on a mob or block.
- The Deep Learner: four models, data from kills and mined blocks, and its HUD while in hand.
- The Simulation Chamber (inference and training, redstone modes, the failure messages).
- The Loot Fabricator with fixed and queued outputs, and the Fabrication Directive.
- The Data Center: the 7 by 7 by 7 shell check, 25 models, shared inputs and outputs, and IO ports by mode.
- `hostilenetworks.toml` in the config folder holds the Java mod's power and model options. The W guide has pages written for this port.

## What differs from the Java mod

- **No mobs in the screens.** The Java mod draws a turning ghost of each mob in its data model items, screens, the Deep Learner and the Data Center. Here a model's item is its frame alone, and its name is in the tooltip.
- **The Deep Learner has no screen.** Hold a model in the off hand and use the learner to put it in (up to four); sneak and use it to take them all out. Its book-style pages of stats and trivia aren't there, its HUD is simpler, and the Curios slot isn't there.
- **The Data Center's screen** is simpler: right click a model to step through what it makes (Java opens a page for it). Queued outputs work in the machine but can't be set up from this screen.
- **Kills count only from a player's own blows**, as Horizon's hook says; arrows and other indirect kills don't yet.
- **Block models** give data for any break of the block, without the Java mod's loot-table check (Silk Touch and so on).
- **`/hostilenetworks` commands and JEI's own pages** aren't there.

## Build

This is a Horizon native mod bundle: Rust mixins compiled into the game. Native mods are unsandboxed code that runs with your rights, so
read what you build.

```bash
horizon-launcher native-build "path/to/folder/with/the/bundle" --trust-native-code
```

## Licence

MIT, as the original. See `LICENSE`.

## Tested

On Horizon commit `6560641`, on Windows: attuning a framework on a zombie, a kill raising a model in the learner, putting models in and
out of the learner, a Simulation Chamber cycle, a Loot Fabricator choice and its drops, a Data Center shell, run and outputs, and a LAN
guest opening a chamber, with 0 panics. **Not tested:** a dedicated server, IO ports by mode, the Fabrication Directive, and block models.
