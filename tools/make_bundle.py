"""Writes the Hostile Neural Networks bundle's horizon-mixin.toml and its one-line hook helpers, and packs the content.

Usage: python make_bundle.py
"""

import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BUNDLE = ROOT / "mixins" / "hostilenetworks"
BUILD = ROOT.parent / "native-build" / "main" / "hostilenetworks"
JAR = ROOT.parent / "Ported" / "HostileNeuralNetworks-26.1.2-7.0.0.jar"
if not JAR.exists():
    JAR = ROOT.parent / "Ported" / JAR.name
HORIZON = "6560641ba51a452f6bc2407baa4eb1b3be517ca8"
MIXINS = "5a64af938ec86b00d53d16a408aabc0038fb3e57"
VERSION = "7.0.1"
M = "crate::game::hostilenetworks"

GAME_MUT = ("game", "&mut crate::game::Game<'_>", "&mut *self")
GAME = ("game", "&crate::game::Game<'_>", "&*self")
ENTITY = ("entity", "&mut horizon_client::containers::ModBlockEntity", "&mut *entity")

# (id, hook, bindings, body)
HOOKS = [
    ("be", "block_entity", [("block", "&str", "block"), ("entity", "&mut Option<(String, horizon_world::nbt::Compound)>", "&mut *entity")], "block_entity(block, entity)"),
    ("be-ticks", "block_entity_ticks", [("block", "&str", "block"), ("ticks", "&mut bool", "&mut *ticks")], "block_entity_ticks(block, ticks)"),
    ("be-container", "block_entity_container", [("block", "&str", "block"), ("container", "&mut Option<horizon_client::containers::ModContainer>", "&mut *container")], "block_entity_container(block, container)"),
    ("be-energy", "block_entity_energy", [("block", "&str", "block"), ("energy", "&mut Option<horizon_client::containers::ModEnergyBuffer>", "&mut *energy")], "block_entity_energy(block, energy)"),
    ("be-tick", "block_entity_tick", [GAME_MUT, ("pos", "[i32; 3]", "pos"), ENTITY], "block_entity_tick(game, pos, entity)"),
    ("accepts", "target:horizon_client::game::native_hooks::Game<'_>::native_container_accepts", [GAME, ("entity", "&horizon_client::containers::ModBlockEntity", "&*entity"), ("slot", "usize", "slot"), ("stack", "&horizon_client::items::ItemStack", "stack"), ("cancel", "&mut bool", "&mut *cancel")], "container_accepts(game, entity, slot, stack, cancel)"),
    ("menu-button", "menu_button_v2", [GAME, ENTITY, ("id", "i32", "id")], "menu_button(entity, id, game)"),
    ("placed", "place_block", [GAME_MUT, ("pos", "[i32; 3]", "pos")], "placed(game, pos)"),
    ("right-click", "right_click_block", [GAME_MUT, ("pos", "[i32; 3]", "pos"), ("canceled", "&mut bool", "&mut *canceled")], "right_click(game, pos, canceled)"),
    ("interact-entity", "interact_entity", [GAME_MUT, ("kind", "&str", "kind"), ("stack", "Option<&ItemStack>", "stack"), ("claimed", "&mut bool", "&mut *claimed")], "interact_entity(game, kind, stack, claimed)"),
    ("use-on", "use_item_on", [GAME_MUT, ("stack", "&ItemStack", "stack"), ("pos", "[i32; 3]", "pos"), ("claimed", "&mut bool", "&mut *claimed")], "use_item_on(game, stack, pos, claimed)"),
    ("use-item", "use_item", [GAME_MUT, ("stack", "&ItemStack", "stack"), ("claimed", "&mut bool", "&mut *claimed")], "use_item(game, stack, claimed)"),
    ("destroy", "destroy_block", [GAME_MUT, ("pos", "[i32; 3]", "pos")], "destroyed(game, pos)"),
    ("killed", "mob_killed", [("player", "u128", "player"), ("kind", "&str", "kind")], "mob_killed(player, kind)"),
    ("inventory-tick", "inventory_tick", [GAME_MUT, ("slot", "i32", "slot")], "inventory_tick(game, slot)"),
    ("input-closed", "target:horizon_client::game::native_hooks::Game<'_>::native_menu_input_closed", [GAME_MUT, ("uuid", "u128", "uuid"), ("items", "&mut Vec<Option<ItemStack>>", "&mut *slots")], "menu_input_closed(game, uuid, items)"),
    ("screen", "screen", [("block", "&str", "block"), ("screen", "&mut Option<crate::game::native_hooks::NativeScreen>", "&mut *screen")], "screen(block, screen)"),
    ("screen-draw", "screen_draw", [GAME, ("block", "&str", "block"), ("data", "&[i32]", "data"), ("batch", "&mut horizon_client::hud::HudBatch", "&mut *batch"), ("origin", "[f32; 2]", "origin")], "screen_draw(game, block, data, batch, origin)"),
    ("screen-click", "screen_click", [GAME, ("block", "&str", "block"), ("data", "&[i32]", "data"), ("at", "[f32; 2]", "at"), ("right", "bool", "right"), ("button", "&mut Option<i32>", "&mut *button")], "screen_click(game, block, data, at, right, game.has_shift_down(), button)"),
    ("screen-tooltip", "screen_tooltip", [GAME, ("block", "&str", "block"), ("data", "&[i32]", "data"), ("at", "[f32; 2]", "at"), ("lines", "&mut Vec<String>", "&mut *lines")], "screen_tooltip(game, block, data, at, lines)"),
    ("tooltip", "tooltip", [GAME, ("stack", "&ItemStack", "stack"), ("lines", "&mut Vec<(Text, u32)>", "&mut *lines")], "tooltip(game, stack, lines)"),
    ("hud", "hud", [GAME, ("batch", "&mut horizon_client::hud::HudBatch", "&mut *batch")], "hud(game, batch)"),
    ("update-tag", "block_entity_update_tag", [("entity", "&horizon_client::containers::ModBlockEntity", "&*entity"), ("tag", "&mut Option<horizon_world::nbt::Compound>", "&mut *tag")], "update_tag(entity, tag)"),
    ("kit-frame", "frame", [GAME_MUT], "kit_frame(game)"),
]

SPECIAL_HELPERS = {
    "embedded_content.rs": """// Hostile Neural Networks is one file on the registry: its blocks, recipes and textures are the content mod in this bundle's data/
// (made by tools/make_content.py from the jar). It is written out once and loaded as a dev mod is.
fn hostilenetworks_embedded_content(root: &std::path::Path, enabled: &std::collections::BTreeSet<String>, dev: &[std::path::PathBuf]) -> core::ops::ControlFlow<std::path::PathBuf> {
    static CONTENT: &[u8] = horizon_client::native_data!("hostilenetworks", "hostilenetworks-content.hmod");
    let dir = std::env::current_dir().unwrap_or_default().join(".horizon-cache");
    let file = dir.join("hostilenetworks-embedded-content.hmod");
    if dev.contains(&file) {
        return core::ops::ControlFlow::Continue(());
    }
    let same = std::fs::read(&file).is_ok_and(|on_disk| on_disk == CONTENT);
    if !same && std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&file, CONTENT)).is_err() {
        log::warn!("Horizon: Hostile Neural Networks' content could not be written to {}", file.display());
        return core::ops::ControlFlow::Continue(());
    }
    let mut with = dev.to_vec();
    with.push(file);
    core::ops::ControlFlow::Break(content_root(root, enabled, &with))
}
""",
    "list.rs": """// The mod's row in Options > Mods, and its one key.
fn hostilenetworks_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::native_hooks::NativeMod {
        id: "hostilenetworks".into(),
        title: "Hostile Neural Networks 7.0.1".into(),
        description: "Defeating hostile entities - but in simulations!".into(),
        keys: vec![["guide".to_string(), "Open Guide for Items".to_string(), "key.keyboard.w".to_string()]],
    });
}
""",
}


def binding_lines(bindings):
    return "".join(f'  {{ name = "{n}", ty = "{t}", value = "{v}" }},\n' for n, t, v in bindings)


def main():
    BUNDLE.mkdir(parents=True, exist_ok=True)
    out = [
        "[mod]",
        'id = "hostilenetworks"',
        f'version = "{VERSION}"',
        "",
        "# The Horizon commit this bundle was built and tested on.",
        "[compatibility]",
        f'horizon_revision = "{HORIZON}"',
        "",
        "[[module]]",
        'id = "hostilenetworks"',
        'file = "crates/horizon-client/src/game.rs"',
        'name = "hostilenetworks"',
        'source = "hostile.rs"',
        "",
    ]
    for hook_id, hook, bindings, body in HOOKS:
        helper = f"h_{hook_id.replace('-', '_')}.rs"
        args = ", ".join(f"{n}: {t}" for n, t, _ in bindings)
        (BUNDLE / helper).write_text(f"fn hostilenetworks_{hook_id.replace('-', '_')}({args}) {{\n    {M}::{body};\n}}\n", encoding="utf8")
        out += ["[[injection]]", f'id = "hnn-{hook_id}"', 'kind = "HEAD"', *(([f'target = "{hook[7:]}"'] if hook.startswith("target:") else [f'hook = "{hook}"', "hook_version = 1"])), f'helper = "{helper}"', "bindings = [", binding_lines(bindings).rstrip("\n"), "]", ""]
    for name, text in SPECIAL_HELPERS.items():
        (BUNDLE / name).write_text(text, encoding="utf8")
    out += ["[[injection]]", 'id = "hnn-list"', 'kind = "HEAD"', 'hook = "mods"', "hook_version = 1", 'helper = "list.rs"', "bindings = [", binding_lines([("mods", "&mut Vec<crate::game::native_hooks::NativeMod>", "&mut *mods")]).rstrip("\n"), "]", ""]
    out += ["[[injection]]", 'id = "hnn-embedded-content"', 'kind = "HEAD"', "cancellable = true", 'target = "horizon_client::game::mods::content_root"', 'helper = "embedded_content.rs"', "bindings = [", binding_lines([("root", "&std::path::Path", "root"), ("enabled", "&std::collections::BTreeSet<String>", "enabled"), ("dev", "&[std::path::PathBuf]", "dev")]).rstrip("\n"), "]", ""]
    (BUNDLE / "horizon-mixin.toml").write_text("\n".join(out), encoding="utf8")
    subprocess.run([sys.executable, str(ROOT.parent / "jei-kit" / "apply.py"), "hostilenetworks", str(BUNDLE)], check=True)
    (BUNDLE / "data").mkdir(exist_ok=True)
    subprocess.run([sys.executable, str(ROOT / "tools" / "make_content.py"), str(JAR), str(BUNDLE / "data" / "hostilenetworks-content.hmod")], check=True)
    target = BUILD / "mixins" / "hostilenetworks"
    target.mkdir(parents=True, exist_ok=True)
    for old in target.rglob("*"):
        if old.is_file():
            old.unlink()
    for f in BUNDLE.rglob("*"):
        if f.is_file():
            to = target / f.relative_to(BUNDLE)
            to.parent.mkdir(parents=True, exist_ok=True)
            to.write_bytes(f.read_bytes())
    (BUILD / "horizon-native.json").write_text(json.dumps({"game": "26.3", "horizon": HORIZON, "id": "hostilenetworks", "mixins": MIXINS, "schema": 1, "version": VERSION}, indent=2), encoding="utf8")
    print("bundle written:", len(HOOKS) + 2, "injections")


if __name__ == "__main__":
    main()
