"""Hostile Neural Networks' blocks, items, recipes and textures as a Horizon content mod, from the jar.

Usage: python make_content.py <jar> <out.hmod>

The data models and tiers go in as one table the mod reads (`data/hostilenetworks/hz_models.json`): those for another mod's mobs
or blocks wait on that mod, as NeoForge's load conditions make them.
"""

import glob
import io
import json
import re
import sys
import zipfile

ID = "hostilenetworks"

# Hostile.Blocks: strength(4, 3000), the IO port strength(5, 6000).
BLOCKS = {
    "sim_chamber": {"hardness": 4.0, "explosion_resistance": 3000.0},
    "loot_fabricator": {"hardness": 4.0, "explosion_resistance": 3000.0},
    "data_center": {"hardness": 5.0, "explosion_resistance": 6000.0},
    "data_center_io_port": {"hardness": 5.0, "explosion_resistance": 6000.0},
}

ITEMS = {
    "blank_data_model": 1,
    "prediction_matrix": 64,
    "overworld_prediction": 64,
    "nether_prediction": 64,
    "end_prediction": 64,
    "twilight_prediction": 64,
    "deep_learner": 1,
    "data_model": 1,
    "prediction": 64,
    "fab_directive": 1,
}

C_TAGS = {
    "obsidians": ["minecraft:obsidian", "minecraft:crying_obsidian"],
    "glass_panes": ["minecraft:glass_pane", "minecraft:black_stained_glass_pane", "minecraft:white_stained_glass_pane"],
    "ingots/gold": ["minecraft:gold_ingot"],
    "ingots/iron": ["minecraft:iron_ingot"],
    "ingots/netherite": ["minecraft:netherite_ingot"],
    "dusts/redstone": ["minecraft:redstone"],
    "gems/diamond": ["minecraft:diamond"],
    "gems/lapis": ["minecraft:lapis_lazuli"],
}


def block(name):
    props = dict(BLOCKS[name], sounds="minecraft:iron_block", requires_tool=True)
    if name == "data_center_io_port":
        return {
            "template": "minecraft:beacon",
            "properties": props,
            "state_properties": {"mode": {"type": "enum", "values": ["energy", "models", "inputs", "outputs"]}},
            "default_state": {"mode": "energy"},
        }
    return {
        "template": "minecraft:beacon",
        "properties": props,
        "state_properties": {"facing": {"type": "enum", "values": ["north", "south", "west", "east"]}},
        "default_state": {"facing": "north"},
    }


def fix_conditions(value):
    if isinstance(value, dict):
        return {("type" if k == "condition" else k): fix_conditions(v) for k, v in value.items()}
    if isinstance(value, list):
        return [fix_conditions(v) for v in value]
    return value


def guide(files):
    pages = {
        "models.md": """---
navigation:
  title: Hostile Neural Networks
item_ids:
- hostilenetworks:blank_data_model
- hostilenetworks:data_model
- hostilenetworks:deep_learner
- hostilenetworks:prediction
- hostilenetworks:prediction_matrix
---

# Data Models

Use a Model Framework on a mob or a block to attune it. Put the model in a Deep Learner and carry it: each kill or block mined gives
it data, and with data it goes up its tiers. A Simulation Chamber runs a model of Basic tier or better on Prediction Matrices and power,
for that world's Generalized Prediction and, as accuracy allows, the model's own Prediction.
""",
        "machines.md": """---
navigation:
  title: Machines
item_ids:
- hostilenetworks:sim_chamber
- hostilenetworks:loot_fabricator
- hostilenetworks:fab_directive
- hostilenetworks:data_center
- hostilenetworks:data_center_io_port
---

# Machines

A **Loot Fabricator** turns a Prediction into the drop chosen for its mob. A **Fabrication Directive** copies its choices.

A **Data Center** runs up to twenty-five Self Aware models at once. Build a hollow 7 by 7 by 7 shell around it: an obsidian floor, black
stained glass walls and ceiling. The controller replaces one bottom-wall block, facing out. **IO Ports** can replace any wall or ceiling block.
""",
    }
    for name, text in pages.items():
        files[f"data/{ID}/ae2guide/{name}"] = text.encode()
    files[f"data/{ID}/ae2guide/index.json"] = json.dumps({"pages": list(pages)}, indent=2).encode()


def main(jar, out):
    source = zipfile.ZipFile(jar)
    files = {}
    models, tiers = {}, {}
    for name in source.namelist():
        if name.endswith("/") or not name.startswith(("assets/", "data/")):
            continue
        if name.startswith("data/curios/") or name.startswith(f"data/{ID}/curios/"):
            continue
        if name.startswith(f"data/{ID}/{ID}/data_models/") and name.endswith(".json"):
            doc = json.loads(source.read(name).decode("utf-8-sig"))
            key = name[len(f"data/{ID}/{ID}/data_models/"):-5]
            needs = [c.get("modid") for c in doc.pop("neoforge:conditions", []) if c.get("modid")]
            doc["hz_needs"] = needs
            models[key] = doc
            continue
        if name.startswith(f"data/{ID}/{ID}/model_tiers/") and name.endswith(".json"):
            key = name[len(f"data/{ID}/{ID}/model_tiers/"):-5]
            tiers[key] = json.loads(source.read(name).decode("utf-8-sig"))
            continue
        if name.startswith(f"data/{ID}/loot_table/"):
            files[name] = json.dumps(fix_conditions(json.loads(source.read(name))), indent=2).encode()
            continue
        if name.startswith(f"data/{ID}/recipe/") and name.endswith(".json"):
            doc = json.loads(source.read(name).decode("utf-8-sig"))
            doc.pop("neoforge:conditions", None)
            files[name] = json.dumps(doc, indent=2).encode()
            continue
        if name.startswith(f"assets/{ID}/lang/") and name.endswith(".json"):
            # "%s Data Model": Horizon names an item by its key alone. The model's name is the tooltip's next line.
            # (As text: one of the jar's lang files isn't strict JSON.)
            text = source.read(name).decode("utf-8-sig")
            for key in (f"item.{ID}.data_model", f"item.{ID}.prediction"):
                text = re.sub(r'("' + re.escape(key) + r'"\s*:\s*")([^"]*)"', lambda m: m.group(1) + m.group(2).replace("%s", "").strip() + '"', text)
            files[name] = text.encode("utf8")
            continue
        if name == f"assets/{ID}/items/data_model.json":
            # Its mob-in-a-frame model type is the Java client's: the frame alone here.
            files[name] = json.dumps({"model": {"type": "minecraft:model", "model": f"{ID}:item/data_model_base"}}).encode()
            continue
        if name == f"assets/{ID}/items/prediction.json":
            files[name] = json.dumps({"model": {"type": "minecraft:model", "model": f"{ID}:item/prediction"}}).encode()
            continue
        files[name] = source.read(name)
    files[f"data/{ID}/hz_models.json"] = json.dumps({"models": models, "tiers": tiers}, indent=1).encode()
    for name in BLOCKS:
        files[f"data/{ID}/horizon/block/{name}.json"] = json.dumps(block(name), indent=2).encode()
    for name, stack in ITEMS.items():
        decl = {"template": "minecraft:paper"}
        if stack != 64:
            decl["max_stack_size"] = stack
        files[f"data/{ID}/horizon/item/{name}.json"] = json.dumps(decl, indent=2).encode()
    for tag, values in C_TAGS.items():
        files[f"data/c/tags/item/{tag}.json"] = json.dumps({"replace": False, "values": values}, indent=2).encode()
    files[f"data/{ID}/horizon/menu/sim_chamber.json"] = json.dumps({"slots": 4, "data_slots": 9}, indent=2).encode()
    files[f"data/{ID}/horizon/menu/loot_fabricator.json"] = json.dumps({"slots": 17, "data_slots": 6}, indent=2).encode()
    files[f"data/{ID}/horizon/menu/data_center.json"] = json.dumps({"slots": 45, "data_slots": 56}, indent=2).encode()
    files[f"data/{ID}/horizon/menu/deep_learner.json"] = json.dumps({"slots": 4, "data_slots": 0}, indent=2).encode()
    files[f"data/{ID}/horizon/creative_tab/main.json"] = json.dumps({
        "title": "itemGroup.hostilenetworks",
        "icon": f"{ID}:sim_chamber",
        "items": [f"{ID}:{n}" for n in ("sim_chamber", "loot_fabricator", "data_center", "data_center_io_port", "deep_learner", "blank_data_model", "prediction_matrix", "overworld_prediction", "nether_prediction", "end_prediction", "twilight_prediction", "fab_directive")],
    }, indent=2).encode()
    guide(files)
    files["horizon-mod.toml"] = "\n".join([
        "# Hostile Neural Networks' blocks and items, from HostileNeuralNetworks-26.1.2-7.0.0.jar by hostilenetworks-horizon/tools/make_content.py.",
        f'id = "{ID}"',
        'name = "Hostile Neural Networks (blocks and items)"',
        'version = "7.0.1"',
        'description = "Hostile Neural Networks\' blocks, recipes and textures."',
        'authors = ["Shadows_of_Fire"]',
        'api = "0.2"',
        'side = "both"',
        'capabilities = ["registries"]',
        "",
    ]).encode()
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as z:
        for name in sorted(files):
            info = zipfile.ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            z.writestr(info, files[name])
    with open(out, "wb") as f:
        f.write(buffer.getvalue())
    print(len(files), "files,", len(buffer.getvalue()), "bytes,", len(models), "models,", len(tiers), "tiers")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
