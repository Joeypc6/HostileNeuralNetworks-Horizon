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
- Jade: the IO port's mode (HostileJadePlugin, "Data Center IO Port Mode" in Jade's options), and the machines'
  contents and energy as Jade shows them for any mod.

## With AE2

The machines don't push what they make, as on NeoForge: a pattern provider fills them, and an import bus on the
machine (or anything else that pulls) brings the result back to the network. The AE2 port's crafting status knows a
Simulation Chamber's run (240 ticks, 300 training) and a Loot Fabricator's (60), so a job's time left is right from
its first craft. The Data Center's time is learned as it works.

## What differs from the Java mod

- **The Deep Learner's screen** has the Java one's layout: the spinning mob, its name, trivia, tier, accuracy, the kills to the next tier and its statistics, with the arrows between models. A click moves a Data Model in or out (there is no carried stack); its text doesn't type itself out, a model's variants aren't cycled, experience is vanilla's 5 for a monster and 1-3 for others, and the Curios slot isn't there. Its HUD is simpler.
- **Mobs in item icons** stand on a Data Model's base as in the Java mod, drawn without the ghost's transparency. A block model shows the block.
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
guest opening a chamber, with 0 panics. Since 7.0.2 also: an IO port taking Enderman batches of 16 pearls without passing 16, an Iron Furnaces generator filling a chamber, the Deep Learner's screen putting models in and taking them out, the mobs on Data Model icons, and the W guide. **Not tested:** a dedicated server, the learner's screen for a LAN guest, the Fabrication Directive, and block models.

Also built on Horizon commit `fdc87ae` (Nightly 2026-11-08) as `native-fdc87ae-83d10460ef68`, with AE2, Jade, JEI (mod kit 14),
Elevator, Iron Furnaces, Tinkers' Construct, Ender IO, Powah and Apothic Spawners: all ten compile together, and the W guide opens in
game with its 3D scenes. The block and item models were compared with the 7.0.0 jar's: all 23 the same, none missing.
