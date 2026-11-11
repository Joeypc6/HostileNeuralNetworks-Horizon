// Hostile Neural Networks (Shadows_of_Fire, MIT): data models, the Deep Learner, Simulation Chamber, Loot Fabricator and Data Center.
// A port of `SimChamberTileEntity`, `LootFabTileEntity`, `DataCenterTileEntity`, `DataCenterShell`, `DataModelInstance`,
// `HostileEvents` and their screens, on Horizon's hooks.
use super::Game;
hostilenetworks_optional::kit_use!(guide as kit_guide);
hostilenetworks_optional::if_mod_items!("jei", {
hostilenetworks_optional::kit_use!(layer as kit_layer);
});
use horizon_client::containers::{ModBlockEntity, ModContainer, ModEnergyBuffer};
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_world::nbt::{Compound, JavaString, Tag};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

type Pos = [i32; 3];

const NS: &str = "hostilenetworks";
const CUSTOM: &str = "minecraft:custom_data";

fn java(s: &str) -> JavaString {
    JavaString::from(s)
}

fn get_i(nbt: &Compound, key: &str, default: i32) -> i32 {
    match nbt.get(&java(key)) {
        Some(Tag::Int(v)) => *v,
        _ => default,
    }
}

fn set_i(nbt: &mut Compound, key: &str, value: i32) {
    nbt.insert(java(key), Tag::Int(value));
}

fn lang(key: &str, english: &str) -> String {
    horizon_client::lang::get_or(key, english).to_string()
}

/// A menu's data slot carries 16 bits: a number past that goes in two.
fn put_wide(data: &mut [i32], lo: usize, hi: usize, v: i32) {
    data[lo] = v & 0xFFFF;
    data[hi] = v >> 16;
}

fn wide(data: &[i32], lo: usize, hi: usize) -> i32 {
    (data[lo] & 0xFFFF) | (data[hi] << 16)
}

fn inside(rect: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= rect[0] && at[0] < rect[0] + rect[2] && at[1] >= rect[1] && at[1] < rect[1] + rect[3]
}

// ---- Config (hostilenetworks.toml, the Java mod's names and defaults)

struct Config {
    sim_power_cap: i32,
    fab_power_cap: i32,
    fab_power_cost: i32,
    data_center_power_cap: i32,
    right_click_to_attune: bool,
    sim_model_upgrade: i32,
    action_upgrades_model: bool,
    continuous_accuracy: bool,
    enable_block_models: bool,
    data_center_sim_cost: f32,
}

const DEFAULT_CONFIG: &str = "# Hostile Neural Networks' options, as the Java mod's.
Sim Chamber Power Cap = 2000000
Loot Fabricator Power Cap = 1000000
Loot Fabricator Power Cost = 256
Data Center Power Cap = 8000000
Right Click To Attune = true
Sim Chamber Upgrades Model = 1
Action Upgrades Model = true
Continuous Accuracy = true
Enable Block Data Models = true
Data Center Sim Cost Multiplier = 1.0
";

fn config() -> &'static Config {
    static CONFIG: OnceLock<Config> = OnceLock::new();
    CONFIG.get_or_init(|| {
        let text = Game::native_config_dir().ok().and_then(|dir| {
            let file = dir.join("hostilenetworks.toml");
            match std::fs::read_to_string(&file) {
                Ok(text) => Some(text),
                Err(_) => {
                    let _ = std::fs::write(&file, DEFAULT_CONFIG);
                    None
                }
            }
        });
        let text = text.unwrap_or_else(|| DEFAULT_CONFIG.to_owned());
        let read = |key: &str| text.lines().find_map(|l| l.split_once('=').filter(|(k, _)| k.trim() == key).map(|(_, v)| v.trim().to_owned()));
        let int = |key: &str, d: i32| read(key).and_then(|v| v.parse().ok()).unwrap_or(d);
        let flag = |key: &str, d: bool| read(key).and_then(|v| v.parse().ok()).unwrap_or(d);
        Config {
            sim_power_cap: int("Sim Chamber Power Cap", 2_000_000),
            fab_power_cap: int("Loot Fabricator Power Cap", 1_000_000),
            fab_power_cost: int("Loot Fabricator Power Cost", 256),
            data_center_power_cap: int("Data Center Power Cap", 8_000_000),
            right_click_to_attune: flag("Right Click To Attune", true),
            sim_model_upgrade: int("Sim Chamber Upgrades Model", 1),
            action_upgrades_model: flag("Action Upgrades Model", true),
            continuous_accuracy: flag("Continuous Accuracy", true),
            enable_block_models: flag("Enable Block Data Models", true),
            data_center_sim_cost: read("Data Center Sim Cost Multiplier").and_then(|v| v.parse().ok()).unwrap_or(1.0),
        }
    })
}

// ---- The models and tiers, from the table the content carries

#[derive(Clone, Debug)]
pub struct Model {
    pub id: String,
    /// The mob's or block's id.
    pub source: String,
    pub is_block: bool,
    pub variants: Vec<String>,
    pub color: String,
    pub sim_cost: i32,
    pub input: serde_json::Value,
    pub base_drop: (String, u32),
    pub fab: Vec<(String, u32)>,
    pub trivia: String,
    pub required: [Option<i32>; 4],
    pub gained: [Option<i32>; 4],
    pub needs: Vec<String>,
    /// DisplayEntity: the size its mob is drawn at, and its offsets.
    pub scale: f32,
    pub offset: [f32; 3],
}

#[derive(Clone, Debug)]
pub struct Tier {
    pub name: String,
    pub required: i32,
    pub gained: i32,
    pub color: String,
    pub accuracy: f32,
    pub can_sim: bool,
}

struct Table {
    models: BTreeMap<String, Model>,
    tiers: Vec<Tier>,
}

static TABLE: OnceLock<Table> = OnceLock::new();

fn stack_spec(v: &serde_json::Value) -> (String, u32) {
    (v.get("id").and_then(|i| i.as_str()).unwrap_or("minecraft:air").to_owned(), v.get("count").and_then(|c| c.as_u64()).unwrap_or(1) as u32)
}

fn table(game: &Game<'_>) -> Option<&'static Table> {
    if let Some(t) = TABLE.get() {
        return Some(t);
    }
    let bytes = game.native_data(&format!("data/{NS}/hz_models.json"))?;
    let doc: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let mut tiers: Vec<Tier> = doc["tiers"]
        .as_object()?
        .iter()
        .map(|(name, t)| Tier {
            name: name.clone(),
            required: t["required_data"].as_i64().unwrap_or(0) as i32,
            gained: t["data_gained"].as_i64().unwrap_or(0) as i32,
            color: t["color"].as_str().unwrap_or("white").to_owned(),
            accuracy: t["accuracy"].as_f64().unwrap_or(0.0) as f32,
            can_sim: t["can_sim"].as_bool().unwrap_or(true),
        })
        .collect();
    tiers.sort_by_key(|t| t.required);
    let per_tier = |v: &serde_json::Value| -> [Option<i32>; 4] {
        ["basic", "advanced", "superior", "self_aware"].map(|k| v.get(k).and_then(|x| x.as_i64()).map(|x| x as i32))
    };
    let parse = |key: &str, m: &serde_json::Value| -> Model {
        let is_block = m.get("block").is_some();
        let source = m.get(if is_block { "block" } else { "entity" }).and_then(|s| s.as_str()).unwrap_or("").to_owned();
        Model {
            id: format!("{NS}:{key}"),
            source,
            is_block,
            variants: m["variants"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
            color: m["name_color"].as_str().unwrap_or("#FFFFFF").to_owned(),
            sim_cost: m["sim_cost"].as_i64().unwrap_or(0) as i32,
            input: m["input"].clone(),
            base_drop: stack_spec(&m["base_drop"]),
            fab: m["fabricator_drops"].as_array().map(|a| a.iter().filter(|d| !d["optional"].as_bool().unwrap_or(false) || game.res.items().get(d["id"].as_str().unwrap_or("")).is_some()).map(stack_spec).collect()).unwrap_or_default(),
            trivia: m["trivia"].as_str().unwrap_or("").to_owned(),
            required: per_tier(&m["required_data"]),
            gained: per_tier(&m["data_gained"]),
            needs: m["hz_needs"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
            scale: m["display"]["scale"].as_f64().unwrap_or(1.0) as f32,
            offset: ["x_offset", "y_offset", "z_offset"].map(|k| m["display"][k].as_f64().unwrap_or(0.0) as f32),
        }
    };
    let mut models = BTreeMap::new();
    for (key, m) in doc["models"].as_object()? {
        let model = parse(key, m);
        models.insert(model.id.clone(), model);
    }
    // Every other mob Horizon knows, a mod's included: its data model file if a mod or pack ships one (the Java
    // mod's format, at data/hostilenetworks/data_models/<namespace>/<mob>.json or data/<namespace>/data_models/<mob>.json),
    // else one made from the mob itself (DataModelRegistry has only files; this keeps no mob without a model).
    let covered = |models: &BTreeMap<String, Model>, id: &str| models.values().any(|m| !m.is_block && (m.source == id || m.variants.iter().any(|v| v == id)));
    let mut kinds: Vec<(String, std::sync::Arc<horizon_client::mobs::MobType>)> = game.res.mob_types().iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    kinds.sort_by(|a, b| a.0.cmp(&b.0));
    let tables = game.res.items().hz_kit_tables_hostilenetworks();
    for (id, kind) in kinds {
        if covered(&models, &id) || !auto_model_wanted(&id, &kind) {
            continue;
        }
        let (ns, path) = id.split_once(':').unwrap_or(("minecraft", &id));
        let file = [format!("data/{NS}/data_models/{ns}/{path}.json"), format!("data/{ns}/data_models/{path}.json")]
            .iter()
            .find_map(|p| game.native_data(p))
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
        let model = match file {
            Some(m) => parse(&format!("{ns}/{path}"), &m),
            None => auto_model(&id, &kind, &tables),
        };
        models.insert(model.id.clone(), model);
    }
    Some(TABLE.get_or_init(|| Table { models, tiers }))
}

/// Whether a mob type gets a data model: a living mob that can be fought, not a player or a display.
fn auto_model_wanted(id: &str, kind: &horizon_client::mobs::MobType) -> bool {
    !matches!(id, "minecraft:player" | "minecraft:armor_stand" | "minecraft:mannequin") && kind.max_health > 0.0
}

/// A data model for a mob no file covers, as the shipped ones are made: its simulation cost from its health (a
/// monster's twice a passive mob's, a power of two, 32 to 4096), the Nether's prediction for a fire-immune mob and
/// the Overworld's otherwise, and its loot table's items as the fabricator's drops (about sixteen kills' worth each).
fn auto_model(id: &str, kind: &horizon_client::mobs::MobType, tables: &horizon_world::loot::LootTables) -> Model {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    let per_hp = if kind.category == "monster" { 6.4 } else { 3.2 };
    let raw = (kind.max_health as f64 * per_hp).max(32.0);
    let sim_cost = (2f64.powf(raw.log2().round()) as i32).clamp(32, 4096);
    let base = if kind.flags.fire_immune { "hostilenetworks:nether_prediction" } else { "hostilenetworks:overworld_prediction" };
    let table = [format!("{ns}:entities/{path}"), format!("{ns}:entity/{path}")].iter().find_map(|t| tables.get(t)).cloned();
    let fab = table.map(|t| loot_items(&t)).unwrap_or_default();
    let tall = kind.height.max(kind.width) as f32;
    Model {
        id: format!("{NS}:auto/{ns}/{path}"),
        source: id.to_owned(),
        is_block: false,
        variants: Vec::new(),
        color: "#FFFFFF".to_owned(),
        sim_cost,
        input: serde_json::Value::String("hostilenetworks:prediction_matrix".to_owned()),
        base_drop: (base.to_owned(), 1),
        fab,
        trivia: "hostilenetworks.trivia.auto".to_owned(),
        required: [None; 4],
        gained: [None; 4],
        needs: Vec::new(),
        scale: if tall > 2.2 { (2.2 / tall).max(0.2) } else { 1.0 },
        offset: [0.0; 3],
    }
}

/// The items a mob's loot table can drop, each with about sixteen kills' worth (its most per kill times sixteen, a
/// rarer one's a quarter of that, 1 to 64), in the table's order.
fn loot_items(table: &serde_json::Value) -> Vec<(String, u32)> {
    fn most(f: &serde_json::Value) -> Option<f64> {
        match f {
            serde_json::Value::Number(n) => n.as_f64(),
            serde_json::Value::Object(o) => o.get("max").and_then(most).or_else(|| o.get("value").and_then(most)),
            _ => None,
        }
    }
    let mut out: Vec<(String, u32)> = Vec::new();
    let walk = |entry: &serde_json::Value, pool_rare: bool, out: &mut Vec<(String, u32)>| {
        if entry["type"].as_str() != Some("minecraft:item") {
            return;
        }
        let Some(name) = entry["name"].as_str() else { return };
        let count = entry["functions"].as_array().into_iter().flatten().filter(|f| f["function"].as_str() == Some("minecraft:set_count")).find_map(|f| most(&f["count"])).unwrap_or(1.0);
        let rare = pool_rare || entry["conditions"].as_array().is_some_and(|c| !c.is_empty());
        let n = ((count.max(1.0) * if rare { 4.0 } else { 16.0 }).round() as u32).clamp(1, 64);
        if !out.iter().any(|(i, _)| i == name) {
            out.push((name.to_owned(), n));
        }
    };
    for pool in table["pools"].as_array().into_iter().flatten() {
        let rare = pool["conditions"].as_array().is_some_and(|c| !c.is_empty());
        for e in pool["entries"].as_array().into_iter().flatten() {
            walk(e, rare, &mut out);
            for child in e["children"].as_array().into_iter().flatten() {
                walk(child, rare, &mut out);
            }
        }
    }
    out
}

fn model_on(game: &Game<'_>, m: &Model) -> bool {
    (!m.is_block || config().enable_block_models) && m.needs.iter().all(|n| game.native_on(n))
}

fn model_of(game: &Game<'_>, stack: &ItemStack) -> Option<&'static Model> {
    let id = custom_str(stack, "hostilenetworks:data_model")?;
    let m = table(game)?.models.get(&id)?;
    model_on(game, m).then_some(m)
}

fn tier_index(name: &str) -> Option<usize> {
    ["basic", "advanced", "superior", "self_aware"].iter().position(|t| *t == name)
}

/// RequiredData: the model's own number for a tier, else the tier's.
fn required(model: &Model, tier: &Tier) -> i32 {
    tier_index(&tier.name).and_then(|i| model.required[i]).unwrap_or(tier.required)
}

fn gained(model: &Model, tier: &Tier) -> i32 {
    tier_index(&tier.name).and_then(|i| model.gained[i]).unwrap_or(tier.gained)
}

/// ModelTierRegistry.getByData.
fn tier_of<'a>(t: &'a Table, model: &Model, data: i32) -> &'a Tier {
    let mut best = &t.tiers[0];
    for tier in &t.tiers {
        if data >= required(model, tier) {
            best = tier;
        }
    }
    best
}

fn next_tier<'a>(t: &'a Table, tier: &Tier) -> &'a Tier {
    let i = t.tiers.iter().position(|x| x.name == tier.name).unwrap_or(0);
    &t.tiers[(i + 1).min(t.tiers.len() - 1)]
}

fn is_max(t: &Table, tier: &Tier) -> bool {
    t.tiers.last().is_some_and(|l| l.name == tier.name)
}

/// DataModelInstance.getAccuracy.
fn accuracy(t: &Table, model: &Model, data: i32) -> f32 {
    let tier = tier_of(t, model, data);
    if config().continuous_accuracy && !is_max(t, tier) {
        let next = next_tier(t, tier);
        let (a, b) = (required(model, tier), required(model, next));
        let diff = (b - a).max(1);
        tier.accuracy + (next.accuracy - tier.accuracy) * (diff - (b - data)) as f32 / diff as f32
    } else {
        tier.accuracy
    }
}

fn roll(chance: f32) -> i32 {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    let floor = chance as i32;
    floor + i32::from(((nanos % 10_000) as f32 / 10_000.0) < chance - floor as f32)
}

// ---- Stack data

fn custom(stack: &ItemStack) -> Option<&Compound> {
    match stack.extra.as_ref()?.other.iter().find(|(k, _)| k == CUSTOM).map(|(_, t)| t) {
        Some(Tag::Compound(c)) => Some(c),
        _ => None,
    }
}

fn custom_str(stack: &ItemStack, key: &str) -> Option<String> {
    match custom(stack)?.get(&java(key)) {
        Some(Tag::String(s)) => s.to_string().ok(),
        _ => None,
    }
}

fn custom_i(stack: &ItemStack, key: &str) -> i32 {
    custom(stack).map_or(0, |c| get_i(c, key, 0))
}

fn set_custom(stack: &mut ItemStack, key: &str, value: Tag) {
    let extra = stack.extra.get_or_insert_with(Default::default);
    let mut data = match extra.other.iter().position(|(k, _)| k == CUSTOM) {
        Some(i) => match extra.other.remove(i).1 {
            Tag::Compound(c) => c,
            _ => Compound::new(),
        },
        None => Compound::new(),
    };
    data.insert(java(key), value);
    extra.other.push((CUSTOM.to_owned(), Tag::Compound(data)));
    extra.other.sort_by(|a, b| a.0.cmp(&b.0));
}

fn model_stack(item: &str, id: &str, count: u32) -> ItemStack {
    let mut s = ItemStack::new(item, count);
    set_custom(&mut s, "hostilenetworks:data_model", Tag::String(java(id)));
    s
}

/// A model's prediction. PredictionTintSource tints it with the model's name colour: here the colour is the stack's
/// `custom_model_data` (its item definition's `minecraft:custom_model_data` tint reads it).
fn prediction_stack(model: &Model, count: u32) -> ItemStack {
    let mut s = model_stack("hostilenetworks:prediction", &model.id, count);
    let extra = s.extra.get_or_insert_with(Default::default);
    let colors = Tag::List(vec![Tag::Int(name_rgb(&model.color) as i32)]);
    extra.other.retain(|(k, _)| k != "minecraft:custom_model_data");
    let mut data = Compound::new();
    data.insert(java("colors"), colors);
    extra.other.push(("minecraft:custom_model_data".to_owned(), Tag::Compound(data)));
    extra.other.sort_by(|a, b| a.0.cmp(&b.0));
    s
}

/// A `TextColor` as JSON writes it: `#RRGGBB` or one of the sixteen names. 0xRRGGBB.
fn name_rgb(color: &str) -> u32 {
    if let Some(hex) = color.strip_prefix('#') {
        return u32::from_str_radix(hex, 16).unwrap_or(0xFFFFFF) & 0xFFFFFF;
    }
    LEGACY.iter().find(|(name, ..)| *name == color).map_or(0xFFFFFF, |(_, _, rgb)| *rgb)
}

/// ChatFormatting's colours: name, code, RGB.
const LEGACY: [(&str, char, u32); 16] = [
    ("black", '0', 0x000000), ("dark_blue", '1', 0x0000AA), ("dark_green", '2', 0x00AA00), ("dark_aqua", '3', 0x00AAAA),
    ("dark_red", '4', 0xAA0000), ("dark_purple", '5', 0xAA00AA), ("gold", '6', 0xFFAA00), ("gray", '7', 0xAAAAAA),
    ("dark_gray", '8', 0x555555), ("blue", '9', 0x5555FF), ("green", 'a', 0x55FF55), ("aqua", 'b', 0x55FFFF),
    ("red", 'c', 0xFF5555), ("light_purple", 'd', 0xFF55FF), ("yellow", 'e', 0xFFFF55), ("white", 'f', 0xFFFFFF),
];

/// The formatting code nearest a colour, for tooltip lines (which take only codes).
fn legacy_code(rgb: u32) -> String {
    let d = |c: u32| {
        let p = |v: u32, s: u32| (((v >> s) & 0xFF) as i32 - ((rgb >> s) & 0xFF) as i32).pow(2);
        p(c, 16) + p(c, 8) + p(c, 0)
    };
    let best = LEGACY.iter().min_by_key(|(_, _, c)| d(*c)).map_or('f', |(_, code, _)| *code);
    format!("§{best}")
}

fn data_of(stack: &ItemStack) -> i32 {
    custom_i(stack, "hostilenetworks:data")
}

fn input_matches(game: &Game<'_>, model: &Model, stack: &ItemStack) -> bool {
    game.native_recipe_ingredient(&model.input).is_some_and(|i| i.order.iter().any(|id| **id == *stack.item))
}

// ---- Machines

fn is_ours(block: &str) -> bool {
    matches!(block, "hostilenetworks:sim_chamber" | "hostilenetworks:loot_fabricator" | "hostilenetworks:data_center" | "hostilenetworks:data_center_io_port")
}

pub fn block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if entity.is_none() && is_ours(block) {
        *entity = Some((block.to_owned(), Compound::new()));
    }
}

pub fn block_entity_ticks(block: &str, ticks: &mut bool) {
    if is_ours(block) {
        *ticks = true;
    }
}

pub fn block_entity_container(block: &str, out: &mut Option<ModContainer>) {
    let (slots, inputs, outputs, menu): (usize, Vec<usize>, Vec<usize>, Option<&str>) = match block {
        "hostilenetworks:sim_chamber" => (4, vec![0, 1], vec![2, 3], Some("hostilenetworks:sim_chamber")),
        "hostilenetworks:loot_fabricator" => (17, vec![0], (1..17).collect(), Some("hostilenetworks:loot_fabricator")),
        "hostilenetworks:data_center" => (45, (0..29).collect(), (29..45).collect(), Some("hostilenetworks:data_center")),
        // DataCenterIOPortTileEntity's proxy: nine slots of its own, the controller moves them by the port's mode.
        "hostilenetworks:data_center_io_port" => (9, (0..9).collect(), (0..9).collect(), None),
        _ => return,
    };
    let all: Vec<usize> = (0..slots).collect();
    *out = Some(ModContainer { slots, faces: std::array::from_fn(|_| all.clone()), inputs, outputs, menu: menu.map(str::to_owned) });
}

pub fn block_entity_energy(block: &str, out: &mut Option<ModEnergyBuffer>) {
    let cap = match block {
        "hostilenetworks:sim_chamber" => config().sim_power_cap,
        "hostilenetworks:loot_fabricator" => config().fab_power_cap,
        "hostilenetworks:data_center" => config().data_center_power_cap,
        "hostilenetworks:data_center_io_port" => config().data_center_power_cap,
        _ => return,
    };
    *out = Some(ModEnergyBuffer { capacity: cap, max_insert: cap, max_extract: 0, insert_faces: [true; 6], extract_faces: [false; 6] });
}

fn energy(e: &ModBlockEntity) -> i32 {
    get_i(&e.nbt, "energy", 0)
}

fn set_energy(e: &mut ModBlockEntity, v: i32) {
    set_i(&mut e.nbt, "energy", v.max(0));
}

fn redstone_ok(game: &mut Game<'_>, pos: Pos, state: i32) -> bool {
    // RedstoneState: ignored, on with a signal, on without.
    if state == 0 {
        return true;
    }
    let dimension = game.native_dimension().to_owned();
    let powered = game.native_neighbor_signal(&dimension, pos) > 0;
    if state == 1 { powered } else { !powered }
}

fn max_stack(game: &Game<'_>, item: &str) -> u32 {
    game.res.items().get(item).map_or(64, |i| i.max_stack)
}

/// The item's stack limit, at least 1.
fn stack_limit(game: &Game<'_>, item: &str) -> u32 {
    max_stack(game, item).max(1)
}

/// How many of `n` more a slot holding `have` (0 when empty) takes when the item stacks to `limit`.
fn slot_room(limit: u32, have: u32, n: u32) -> u32 {
    limit.saturating_sub(have).min(n)
}

fn can_stack(game: &Game<'_>, slot: &Option<ItemStack>, stack: &ItemStack) -> bool {
    let limit = stack_limit(game, &stack.item);
    match slot {
        None => slot_room(limit, 0, stack.count) == stack.count,
        Some(have) => have.same_item_same_components(stack) && slot_room(limit, have.count, stack.count) == stack.count,
    }
}

/// Puts what fits of `stack` into `slot`, which stacks to `limit`; returns the rest.
fn add_limited(limit: u32, slot: &mut Option<ItemStack>, stack: ItemStack) -> Option<ItemStack> {
    let room = match slot {
        None => slot_room(limit, 0, stack.count),
        Some(have) if have.same_item_same_components(&stack) => slot_room(limit, have.count, stack.count),
        Some(_) => 0,
    };
    if room == 0 {
        return Some(stack);
    }
    match slot {
        Some(have) => have.count += room,
        None => {
            let mut put = stack.clone();
            put.count = room;
            *slot = Some(put);
        }
    }
    (room < stack.count).then(|| {
        let mut rest = stack;
        rest.count -= room;
        rest
    })
}

/// What the simulation stopped on: FailureState's ordinal.
const F_NONE: i32 = 0;
const F_OUTPUT: i32 = 1;
const F_ENERGY: i32 = 2;
const F_INPUT: i32 = 3;
const F_MODEL: i32 = 4;
const F_FAULTY: i32 = 5;
const F_ENERGY_MID: i32 = 6;
const F_REDSTONE: i32 = 7;
const F_CANNOT_TRAIN: i32 = 8;
const F_SHELL: i32 = 9;
const F_NOT_SELF_AWARE: i32 = 10;
const F_NO_SELECTION: i32 = 11;
const F_BUFFERING: i32 = 12;
const FAIL_KEYS: [&str; 13] = ["", "output", "energy", "input", "model", "faulty", "energy_mid_cycle", "redstone", "cannot_train", "shell_broken", "not_self_aware", "no_selection", "buffering"];

fn add_to(game: &Game<'_>, slot: &mut Option<ItemStack>, stack: ItemStack) -> Option<ItemStack> {
    add_limited(stack_limit(game, &stack.item), slot, stack)
}

fn sim_tick(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity) {
    let Some(t) = table(game) else { return };
    if e.data.len() < 9 {
        e.data.resize(9, 0);
    }
    let mut mode = get_i(&e.nbt, "simMode", 0);
    if mode == 1 && config().sim_model_upgrade == 0 {
        mode = 0;
        set_i(&mut e.nbt, "simMode", 0);
    }
    let redstone = get_i(&e.nbt, "redstoneState", 0);
    let mut runtime = get_i(&e.nbt, "runtime", 0);
    // Every way through the run below sets it.
    let fail: i32;
    let run_time = if mode == 0 { 240 } else { 300 };
    let cost_mult = if mode == 0 { 1.0f32 } else { 1.2 };
    'run: {
        let Some(stack) = e.items[0].clone() else {
            fail = F_MODEL;
            runtime = 0;
            break 'run;
        };
        let Some(model) = model_of(game, &stack) else {
            fail = F_MODEL;
            runtime = 0;
            break 'run;
        };
        let data = data_of(&stack);
        let tier = tier_of(t, model, data);
        if !tier.can_sim {
            fail = F_FAULTY;
            runtime = 0;
            break 'run;
        }
        let cost = (model.sim_cost as f32 * cost_mult) as i32;
        if runtime == 0 {
            // canStartSimulation.
            let has_input = e.items[1].as_ref().is_some_and(|s| input_matches(game, model, s));
            if !has_input {
                fail = F_INPUT;
                break 'run;
            }
            if !redstone_ok(game, pos, redstone) {
                fail = F_REDSTONE;
                break 'run;
            }
            let can_upgrade = !is_max(t, tier) && (config().sim_model_upgrade == 1 || (config().sim_model_upgrade == 2 && data < required(model, &t.tiers[t.tiers.len() - 2])));
            if mode == 1 && !can_upgrade {
                fail = F_CANNOT_TRAIN;
                break 'run;
            }
            let base = ItemStack::new(&model.base_drop.0, model.base_drop.1);
            let pred = prediction_stack(model, 1);
            if !can_stack(game, &e.items[2], &base) || !can_stack(game, &e.items[3], &pred) {
                fail = F_OUTPUT;
                break 'run;
            }
            if energy(e) < cost {
                fail = F_ENERGY;
                break 'run;
            }
            fail = F_NONE;
            runtime = run_time;
            set_i(&mut e.nbt, "predSuccess", roll(accuracy(t, model, data)));
            if let Some(s) = e.items[1].as_mut() {
                s.count -= 1;
                if s.count == 0 {
                    e.items[1] = None;
                }
            }
        } else if energy(e) >= cost {
            if redstone_ok(game, pos, redstone) {
                fail = F_NONE;
                runtime -= 1;
                if runtime == 0 {
                    if mode == 0 {
                        let base = ItemStack::new(&model.base_drop.0, model.base_drop.1);
                        let _ = add_to(game, &mut e.items[2], base);
                        let success = get_i(&e.nbt, "predSuccess", 0);
                        if success > 0 {
                            let pred = prediction_stack(model, success as u32);
                            let _ = add_to(game, &mut e.items[3], pred);
                        }
                    }
                    if let Some(s) = e.items[0].as_mut() {
                        if mode == 1 {
                            set_custom(s, "hostilenetworks:data", Tag::Int(data + 1));
                        }
                        let iters = custom_i(s, "hostilenetworks:iterations");
                        set_custom(s, "hostilenetworks:iterations", Tag::Int(iters + 1));
                    }
                } else {
                    let left = energy(e) - cost;
                    set_energy(e, left);
                }
            } else {
                fail = F_REDSTONE;
            }
        } else {
            fail = F_ENERGY_MID;
        }
    }
    set_i(&mut e.nbt, "runtime", runtime);
    set_i(&mut e.nbt, "failState", fail);
    let en = energy(e);
    e.data[0] = runtime;
    e.data[1] = run_time;
    e.data[2] = fail;
    put_wide(&mut e.data, 3, 7, en);
    put_wide(&mut e.data, 4, 8, config().sim_power_cap);
    e.data[5] = redstone;
    e.data[6] = mode;
}

// ---- The Loot Fabricator: what's chosen per model is in the block's nbt, `sel.<model>`, as (mode, cursor, entries...).

fn selection(nbt: &Compound, model: &str) -> Vec<i32> {
    match nbt.get(&java(&format!("sel.{model}"))) {
        Some(Tag::IntArray(a)) => a.to_vec(),
        _ => Vec::new(),
    }
}

fn set_selection(nbt: &mut Compound, model: &str, v: Vec<i32>) {
    let key = java(&format!("sel.{model}"));
    if v.is_empty() {
        nbt.remove(&key);
    } else {
        nbt.insert(key, Tag::IntArray(v.into()));
    }
}

/// FabSelection.current: the drop being made, -1 for none. (mode 0: fixed, the entry; mode 1: queue, at the cursor.)
fn current_drop(sel: &[i32]) -> i32 {
    match sel {
        [] => -1,
        [0, _, fixed, ..] => *fixed,
        [1, cursor, rest @ ..] if !rest.is_empty() => rest[(*cursor as usize).min(rest.len() - 1)],
        _ => -1,
    }
}

fn advance(sel: &mut [i32]) {
    if sel.len() > 2 && sel[0] == 1 {
        sel[1] = (sel[1] + 1) % (sel.len() as i32 - 2);
    }
}

fn into_outputs(game: &Game<'_>, items: &mut [Option<ItemStack>], range: std::ops::Range<usize>, stack: ItemStack) -> Option<ItemStack> {
    let mut left = Some(stack);
    for i in range {
        let Some(s) = left.take() else { return None };
        left = add_to(game, &mut items[i], s);
    }
    left
}

fn fits(game: &Game<'_>, items: &[Option<ItemStack>], range: std::ops::Range<usize>, stack: &ItemStack) -> bool {
    let mut copy: Vec<Option<ItemStack>> = items.to_vec();
    into_outputs(game, &mut copy, range, stack.clone()).is_none()
}

fn fab_tick(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity) {
    if e.data.len() < 6 {
        e.data.resize(6, 0);
    }
    let redstone = get_i(&e.nbt, "redstoneState", 0);
    let mut runtime = get_i(&e.nbt, "runtime", 0);
    let model = e.items[0].clone().and_then(|s| model_of(game, &s));
    match model {
        Some(model) => {
            let id = model.id.clone();
            let sel = selection(&e.nbt, &id);
            let chosen = current_drop(&sel);
            if get_i(&e.nbt, "currentSel", -1) != chosen {
                set_i(&mut e.nbt, "currentSel", chosen);
                runtime = 0;
            } else if chosen >= 0 && redstone_ok(game, pos, redstone) {
                if runtime >= 60 {
                    if let Some(drop) = model.fab.get(chosen as usize) {
                        let out = ItemStack::new(&drop.0, drop.1);
                        if fits(game, &e.items, 1..17, &out) {
                            runtime = 0;
                            let _ = into_outputs(game, &mut e.items, 1..17, out);
                            if let Some(s) = e.items[0].as_mut() {
                                s.count -= 1;
                                if s.count == 0 {
                                    e.items[0] = None;
                                }
                            }
                            let mut sel = sel;
                            advance(&mut sel);
                            set_selection(&mut e.nbt, &id, sel);
                        }
                    }
                } else if energy(e) >= config().fab_power_cost {
                    let left = energy(e) - config().fab_power_cost;
                    set_energy(e, left);
                    runtime += 1;
                }
            } else if chosen < 0 {
                runtime = 0;
            }
        }
        None => runtime = 0,
    }
    set_i(&mut e.nbt, "runtime", runtime);
    let en = energy(e);
    e.data[0] = runtime;
    e.data[1] = redstone;
    put_wide(&mut e.data, 2, 4, en);
    put_wide(&mut e.data, 3, 5, config().fab_power_cap);
}

// ---- The Data Center

/// DataCenterShell: the block a cell must be, for a shell whose corner is `min`. A wall may also be an IO port.
fn shell_ok(game: &Game<'_>, controller: Pos, min: Pos) -> Option<Vec<Pos>> {
    let mut ports = Vec::new();
    for x in 0..7 {
        for y in 0..7 {
            for z in 0..7 {
                let p = [min[0] + x, min[1] + y, min[2] + z];
                if p == controller {
                    continue;
                }
                let block = game.native_block(p);
                let edge = x == 0 || x == 6 || z == 0 || z == 6;
                if y == 0 {
                    if block != "minecraft:obsidian" {
                        return None;
                    }
                } else if y == 6 || edge {
                    if block == "hostilenetworks:data_center_io_port" {
                        ports.push(p);
                    } else if block != "minecraft:black_stained_glass" {
                        return None;
                    }
                } else if block != "minecraft:air" {
                    return None;
                }
            }
        }
    }
    Some(ports)
}

/// bestGuessLayout: the controller replaces a bottom-wall block, its back toward the shell's inside.
fn find_shell(game: &Game<'_>, pos: Pos) -> Option<(Pos, Vec<Pos>)> {
    let facing = game.native_block_property(pos, "facing").unwrap_or_else(|| "north".to_owned());
    let floor = pos[1] - 1;
    let candidates: Vec<Pos> = match facing.as_str() {
        "north" => (0..7).map(|o| [pos[0] - o, floor, pos[2]]).collect(),
        "south" => (0..7).map(|o| [pos[0] - o, floor, pos[2] - 6]).collect(),
        "west" => (0..7).map(|o| [pos[0], floor, pos[2] - o]).collect(),
        _ => (0..7).map(|o| [pos[0] - 6, floor, pos[2] - o]).collect(),
    };
    candidates.into_iter().find_map(|min| shell_ok(game, pos, min).map(|ports| (min, ports)))
}

fn center_tick(game: &mut Game<'_>, pos: Pos, e: &mut ModBlockEntity) {
    let Some(t) = table(game) else { return };
    if e.data.len() < 56 {
        e.data.resize(56, 0);
    }
    let mut runtimes = int_array(&e.nbt, "runtimes", 25);
    let mut success = int_array(&e.nbt, "success", 25);
    let mut fails = int_array(&e.nbt, "fails", 25);
    let time = game.world.game_time as i64;
    let mut valid = get_i(&e.nbt, "shellValid", 0) != 0;
    if time % 20 == 0 {
        let found = find_shell(game, pos);
        valid = found.is_some();
        set_i(&mut e.nbt, "shellValid", i32::from(valid));
        if let Some((_, ports)) = found {
            let list: Vec<i32> = ports.iter().flatten().copied().collect();
            set_int_array(&mut e.nbt, "ports", list);
        } else {
            set_int_array(&mut e.nbt, "ports", Vec::new());
        }
    }
    // The ports.
    let ports = match e.nbt.get(&java("ports")) {
        Some(Tag::IntArray(a)) => a.to_vec(),
        _ => Vec::new(),
    };
    for port in ports.chunks_exact(3) {
        let at = [port[0], port[1], port[2]];
        let mode = game.native_block_property(at, "mode").unwrap_or_default();
        let room = (config().data_center_power_cap - energy(e)).max(0);
        match mode.as_str() {
            "energy" => {
                let moved = game.native_with_mod_block_entity(at, |p| {
                    let have = get_i(&p.nbt, "energy", 0);
                    let n = have.min(room);
                    set_i(&mut p.nbt, "energy", have - n);
                    n
                });
                let total = energy(e) + moved.unwrap_or(0);
                set_energy(e, total);
            }
            "models" | "inputs" => {
                let range = if mode == "models" { 0..25 } else { 25..29 };
                let taken = game.native_with_mod_block_entity(at, |p| {
                    p.items.resize(9, None);
                    p.items.iter_mut().map(Option::take).collect::<Vec<_>>()
                });
                let mut back = Vec::new();
                for stack in taken.unwrap_or_default().into_iter().flatten() {
                    let wanted = if mode == "models" {
                        &*stack.item == "hostilenetworks:data_model"
                    } else {
                        (0..25).filter_map(|i| e.items[i].as_ref()).filter_map(|m| model_of(game, m)).any(|m| input_matches(game, m, &stack))
                    };
                    if wanted && let Some(rest) = into_outputs(game, &mut e.items, range.clone(), stack.clone()) {
                        back.push(rest);
                    } else if !wanted {
                        back.push(stack);
                    }
                }
                game.native_with_mod_block_entity(at, |p| {
                    for (i, stack) in back.into_iter().enumerate().take(9) {
                        p.items[i] = Some(stack);
                    }
                });
            }
            "outputs" => {
                for i in 29..45 {
                    let Some(stack) = e.items[i].take() else { continue };
                    let limit = stack_limit(game, &stack.item);
                    // What the port can't take stays in the controller's output slot.
                    let rest = game.native_with_mod_block_entity(at, |p| {
                        p.items.resize(9, None);
                        let mut left = Some(stack.clone());
                        for slot in p.items.iter_mut() {
                            let Some(s) = left.take() else { break };
                            left = add_limited(limit, slot, s);
                        }
                        left
                    });
                    e.items[i] = rest.unwrap_or(Some(stack));
                }
            }
            _ => {}
        }
    }
    let redstone = get_i(&e.nbt, "redstoneState", 0);
    if !valid {
        runtimes = vec![0; 25];
        fails = vec![F_SHELL; 25];
    } else if !redstone_ok(game, pos, redstone) {
        for i in 0..25 {
            if e.items[i].is_some() {
                fails[i] = F_REDSTONE;
            }
        }
    } else {
        // What didn't fit last time goes first.
        let mut buffered = take_buffer(e);
        buffered = buffered.into_iter().filter_map(|s| into_outputs(game, &mut e.items, 29..45, s)).collect();
        for i in 0..25 {
            let Some(stack) = e.items[i].clone() else {
                runtimes[i] = 0;
                fails[i] = F_MODEL;
                continue;
            };
            let Some(model) = model_of(game, &stack) else {
                runtimes[i] = 0;
                fails[i] = F_MODEL;
                continue;
            };
            let data = data_of(&stack);
            let tier = tier_of(t, model, data);
            if !tier.can_sim {
                runtimes[i] = 0;
                fails[i] = F_FAULTY;
                continue;
            }
            if !is_max(t, tier) {
                runtimes[i] = 0;
                fails[i] = F_NOT_SELF_AWARE;
                continue;
            }
            let cost = (model.sim_cost as f32 * config().data_center_sim_cost).ceil() as i32;
            if runtimes[i] == 0 {
                let chosen = current_drop(&selection(&e.nbt, &model.id));
                if fails[i] == F_BUFFERING && !buffered.is_empty() {
                    continue;
                }
                if chosen < 0 || chosen as usize >= model.fab.len() {
                    fails[i] = F_NO_SELECTION;
                    continue;
                }
                let input = (25..29).find(|s| e.items[*s].as_ref().is_some_and(|x| input_matches(game, model, x)));
                let Some(input) = input else {
                    fails[i] = F_INPUT;
                    continue;
                };
                if energy(e) < cost {
                    fails[i] = F_ENERGY;
                    continue;
                }
                let d = &model.fab[chosen as usize];
                let (expected, base) = (ItemStack::new(&d.0, d.1), ItemStack::new(&model.base_drop.0, model.base_drop.1));
                if !fits_any(game, &e.items, &expected) || !fits_any(game, &e.items, &base) {
                    fails[i] = F_OUTPUT;
                    continue;
                }
                runtimes[i] = 300;
                if let Some(s) = e.items[input].as_mut() {
                    s.count -= 1;
                    if s.count == 0 {
                        e.items[input] = None;
                    }
                }
                success[i] = roll(accuracy(t, model, data));
                fails[i] = F_NONE;
            } else if energy(e) < cost {
                fails[i] = F_ENERGY_MID;
            } else {
                let left = energy(e) - cost;
                set_energy(e, left);
                fails[i] = F_NONE;
                runtimes[i] -= 1;
                if runtimes[i] == 0 {
                    // emitOutputs.
                    let mut over = false;
                    let base = ItemStack::new(&model.base_drop.0, model.base_drop.1);
                    if let Some(rest) = into_outputs(game, &mut e.items, 29..45, base) {
                        buffered.push(rest);
                        over = true;
                    }
                    let mut sel = selection(&e.nbt, &model.id);
                    for _ in 0..success[i] {
                        let idx = current_drop(&sel);
                        let Some(d) = (idx >= 0).then(|| model.fab.get(idx as usize)).flatten() else { break };
                        if let Some(rest) = into_outputs(game, &mut e.items, 29..45, ItemStack::new(&d.0, d.1)) {
                            buffered.push(rest);
                            over = true;
                        }
                        advance(&mut sel);
                    }
                    set_selection(&mut e.nbt, &model.id, sel);
                    if over {
                        fails[i] = F_BUFFERING;
                    }
                    if let Some(s) = e.items[i].as_mut() {
                        let iters = custom_i(s, "hostilenetworks:iterations");
                        set_custom(s, "hostilenetworks:iterations", Tag::Int(iters + 1));
                    }
                }
            }
        }
        put_buffer(e, buffered);
    }
    set_int_array(&mut e.nbt, "runtimes", runtimes.clone());
    set_int_array(&mut e.nbt, "success", success);
    set_int_array(&mut e.nbt, "fails", fails.clone());
    for i in 0..25 {
        e.data[i] = runtimes[i];
        e.data[25 + i] = fails[i];
    }
    let en = energy(e);
    e.data[50] = redstone;
    e.data[51] = i32::from(valid);
    put_wide(&mut e.data, 52, 54, en);
    put_wide(&mut e.data, 53, 55, config().data_center_power_cap);
}

fn fits_any(game: &Game<'_>, items: &[Option<ItemStack>], stack: &ItemStack) -> bool {
    (29..45).any(|i| match &items[i] {
        None => true,
        Some(have) => have.same_item_same_components(stack) && have.count < max_stack(game, &have.item),
    })
}

fn take_buffer(e: &mut ModBlockEntity) -> Vec<ItemStack> {
    let list = match e.nbt.remove(&java("buffered")) {
        Some(Tag::String(s)) => s.to_string().unwrap_or_default(),
        _ => String::new(),
    };
    list.split(';').filter_map(|p| p.split_once('*')).filter_map(|(id, n)| Some(ItemStack::new(id, n.parse().ok()?))).collect()
}

fn put_buffer(e: &mut ModBlockEntity, list: Vec<ItemStack>) {
    if !list.is_empty() {
        let text: Vec<String> = list.iter().map(|s| format!("{}*{}", s.item, s.count)).collect();
        e.nbt.insert(java("buffered"), Tag::String(java(&text.join(";"))));
    }
}

fn int_array(nbt: &Compound, key: &str, len: usize) -> Vec<i32> {
    let mut out = match nbt.get(&java(key)) {
        Some(Tag::IntArray(a)) => a.to_vec(),
        _ => Vec::new(),
    };
    out.resize(len, 0);
    out
}

fn set_int_array(nbt: &mut Compound, key: &str, values: Vec<i32>) {
    nbt.insert(java(key), Tag::IntArray(values.into()));
}

// ---- AE2: how long a machine takes, for a crafting job's time left

hostilenetworks_optional::if_mod_items!("port_ae2_me", {
/// A pattern's craft in a Simulation Chamber (240 ticks a run, 300 training, and the tick it starts on) or a Loot
/// Fabricator (60 ticks and the tick it makes the drop on), one run for each matrix or prediction it takes.
fn ae2_machine(game: &Game<'_>, pos: [i32; 3], block: &str, inputs: &[(&str, i64)]) -> Option<crate::game::port_ae2_me::MachineTime> {
    let Some(horizon_client::containers::BlockEntity::Mod(e)) = game.world.block_entities.get(&(pos[0], pos[1], pos[2])) else { return None };
    let runtime = get_i(&e.nbt, "runtime", 0);
    let (cycle, runs, done) = match block {
        "hostilenetworks:sim_chamber" => {
            let model = e.items.first().and_then(Option::as_ref).and_then(|s| model_of(game, s))?;
            let runs = inputs.iter().find(|(id, _)| input_matches(game, model, &ItemStack::new(id, 1))).map(|(_, n)| *n)?;
            let run_time = if get_i(&e.nbt, "simMode", 0) == 1 { 300 } else { 240 };
            (f64::from(run_time + 1), runs, (runtime > 0).then(|| f64::from(run_time - runtime + 1)))
        }
        "hostilenetworks:loot_fabricator" => {
            let runs = inputs.iter().find(|(id, _)| *id == "hostilenetworks:prediction").map(|(_, n)| *n)?;
            (61.0, runs, (runtime > 0).then(|| f64::from(runtime)))
        }
        _ => return None,
    };
    let total = cycle * runs.max(1) as f64;
    Some(crate::game::port_ae2_me::MachineTime { ticks: total, latency: total, done })
}
});

fn register_ae2() {
    hostilenetworks_optional::if_mod!("port_ae2_me", {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| crate::game::port_ae2_me::register_machine(ae2_machine));
    } else { () });
}

// ---- Jade: HostileJadePlugin, the IO port's mode

hostilenetworks_optional::if_mod_items!("jade", {
const JADE_IO_PORT: &str = "plugin_hostilenetworks.io_port_mode";

fn jade_lines(game: &Game<'_>, target: &crate::game::jade::Target, lines: &mut Vec<crate::game::jade::Line>) {
    let crate::game::jade::Target::Block { pos, block } = target else { return };
    if block != "hostilenetworks:data_center_io_port" || !crate::game::jade::plugin_on(game, JADE_IO_PORT) {
        return;
    }
    let id = game.world.block(pos[0], pos[1], pos[2]);
    let mode = game.pack.assets.registry.state(id).and_then(|s| s.properties.get("mode").map(|m| m.to_string())).unwrap_or_else(|| "energy".to_owned());
    let word = lang(&format!("hostilenetworks.io_port.mode.{mode}"), &mode);
    lines.push(crate::game::jade::Line::text(lang("hostilenetworks.io_port.mode_set", "Mode: %s").replace("%s", &word)));
}
});

fn register_jade() {
    hostilenetworks_optional::if_mod!("jade", {
        crate::game::jade::register_option(JADE_IO_PORT, "Data Center IO Port Mode", true);
        crate::game::jade::register_client(jade_lines);
    } else { () });
}

pub(in crate::game) fn block_entity_tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    register_ae2();
    match entity.id.as_str() {
        "hostilenetworks:sim_chamber" => {
            entity.items.resize(4, None);
            sim_tick(game, pos, entity);
        }
        "hostilenetworks:loot_fabricator" => {
            entity.items.resize(17, None);
            fab_tick(game, pos, entity);
        }
        "hostilenetworks:data_center" => {
            entity.items.resize(45, None);
            center_tick(game, pos, entity);
        }
        _ => {}
    }
}

/// `native_menu_slot_rules`-style checks a hopper asks on the server.
pub(in crate::game) fn container_accepts(game: &Game<'_>, entity: &ModBlockEntity, slot: usize, stack: &ItemStack, cancel: &mut bool) {
    let ok = match entity.id.as_str() {
        "hostilenetworks:sim_chamber" => match slot {
            0 => &*stack.item == "hostilenetworks:data_model",
            1 => entity.items.first().and_then(|m| m.as_ref()).and_then(|m| model_of(game, m)).is_some_and(|m| input_matches(game, m, stack)),
            _ => false,
        },
        "hostilenetworks:loot_fabricator" => slot == 0 && &*stack.item == "hostilenetworks:prediction",
        "hostilenetworks:data_center" => match slot {
            0..=24 => &*stack.item == "hostilenetworks:data_model",
            25..=28 => (0..25).filter_map(|i| entity.items.get(i)?.as_ref()).filter_map(|m| model_of(game, m)).any(|m| input_matches(game, m, stack)),
            _ => false,
        },
        // By its mode, which the entity alone doesn't say: the controller hands back what it can't use.
        "hostilenetworks:data_center_io_port" => true,
        _ => return,
    };
    if !ok {
        *cancel = true;
    }
}

// ---- Who clicked what

#[derive(Default)]
struct State {
    /// Kills and blocks not yet given to the player's learners: (player, kind).
    pending: Vec<(u128, String)>,
    mouse: [f32; 2],
    /// The loot fabricator's page.
    page: usize,
    open_learner: HashMap<u128, i32>,
    /// The machine screen drawn this frame: its container, block, menu data and corner, for its tooltips.
    shown: Option<(Option<(i32, i32, i32)>, String, Vec<i32>, [f32; 2])>,
    learner: LearnerView,
    /// Where the learner's screen was last drawn, for its clicks.
    learner_origin: [f32; 2],
    /// Moves players' learner screens asked for, done in their inventory ticks.
    learner_ops: Vec<(u128, LearnerOp)>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn tell(game: &mut Game<'_>, uuid: u128, text: &str) {
    game.native_action_bar(uuid, horizon_client::text::Text::legacy(text));
}

/// HostileEvents.modelAttunement: a Model Framework on a mob.
pub(in crate::game) fn interact_entity(game: &mut Game<'_>, kind: &str, stack: Option<&ItemStack>, claimed: &mut bool) {
    if !config().right_click_to_attune || stack.is_none_or(|s| &*s.item != "hostilenetworks:blank_data_model") {
        return;
    }
    *claimed = true;
    attune(game, kind, false);
}

/// modelAttunementBlock: and on a block.
pub(in crate::game) fn use_item_on(game: &mut Game<'_>, stack: &ItemStack, pos: Pos, claimed: &mut bool) {
    match &*stack.item {
        "hostilenetworks:blank_data_model" if config().right_click_to_attune => {
            *claimed = true;
            let block = game.native_block(pos);
            attune(game, &block, true);
        }
        _ => {}
    }
}

fn attune(game: &mut Game<'_>, kind: &str, block: bool) {
    let Some(t) = table(game) else { return };
    let Some(player) = game.native_acting_player() else { return };
    let found: Vec<&Model> = t.models.values().filter(|m| m.is_block == block && model_on(game, m) && (m.source == kind || m.variants.iter().any(|v| v == kind))).collect();
    match found.as_slice() {
        [model] => {
            let name = model_name(model);
            if let Some(held) = game.held_mut() {
                *held = model_stack("hostilenetworks:data_model", &model.id, 1);
            }
            let line = lang("hostilenetworks.msg.built", "Attuned to %s").replace("%s", &name);
            tell(game, player.uuid, &format!("§a{line}"));
        }
        [] => {
            let line = lang("hostilenetworks.msg.no_model", "No model available");
            tell(game, player.uuid, &format!("§c{line}"));
        }
        _ => {
            let line = lang("hostilenetworks.msg.multiple_models", "More than one model");
            tell(game, player.uuid, &format!("§c{line}"));
        }
    }
}

fn model_name(m: &Model) -> String {
    let (ns, path) = m.source.split_once(':').unwrap_or(("minecraft", &m.source));
    let kind = if m.is_block { "block" } else { "entity" };
    lang(&format!("{kind}.{ns}.{path}"), path)
}

/// HostileEvents.kill: the blow's player gets data in every learner they carry.
pub fn mob_killed(player: u128, kind: &str) {
    if config().action_upgrades_model {
        STATE.with_borrow_mut(|s| s.pending.push((player, kind.to_owned())));
    }
}

/// HostileEvents.mine.
pub(in crate::game) fn destroyed(game: &mut Game<'_>, pos: Pos) {
    if !config().action_upgrades_model {
        return;
    }
    let block = game.native_block(pos);
    if let Some(p) = game.native_acting_player() {
        STATE.with_borrow_mut(|s| s.pending.push((p.uuid, block)));
    }
}

/// `native_inventory_tick`: the slot of a player's inventory, with that player in place.
pub(in crate::game) fn inventory_tick(game: &mut Game<'_>, slot: i32, selected: bool) {
    let Some(player) = game.native_acting_player() else { return };
    if (selected || slot == -106) && game.native_carried_stack(slot).is_some_and(|s| &*s.item == LEARNER) {
        apply_learner_ops(game, player.uuid, slot);
    }
    let kinds: Vec<String> = STATE.with_borrow(|s| s.pending.iter().filter(|(u, _)| *u == player.uuid).map(|(_, k)| k.clone()).collect());
    if kinds.is_empty() {
        return;
    }
    let is_learner = game.native_carried_stack(slot).is_some_and(|s| &*s.item == "hostilenetworks:deep_learner");
    if !is_learner {
        return;
    }
    let Some(t) = table(game) else { return };
    let models: Vec<&Model> = t.models.values().filter(|m| model_on(game, m)).collect();
    if let Some(stack) = game.native_carried_stack(slot) {
        let mut inside = learner_models(stack);
        for kind in &kinds {
            for m in inside.iter_mut().flatten() {
                let Some(id) = custom_str(m, "hostilenetworks:data_model") else { continue };
                let Some(model) = models.iter().find(|x| x.id == id) else { continue };
                if model.source == *kind || model.variants.iter().any(|v| v == kind) {
                    let data = data_of(m);
                    let tier = tier_of(t, model, data);
                    set_custom(m, "hostilenetworks:data", Tag::Int(data + gained(model, tier)));
                }
            }
        }
        set_learner_models(stack, &inside);
    }
    STATE.with_borrow_mut(|s| s.pending.retain(|(u, _)| *u != player.uuid));
}

/// The learner's four models, as a compact text on the stack: id, data and iterations each.
fn learner_models(stack: &ItemStack) -> Vec<Option<ItemStack>> {
    let mut out = vec![None; 4];
    let text = custom_str(stack, "hostilenetworks:learner_inv").unwrap_or_default();
    for (i, part) in text.split(';').enumerate().take(4) {
        let f: Vec<&str> = part.split(',').collect();
        if let [id, data, iters] = f.as_slice()
            && !id.is_empty()
        {
            let mut m = model_stack("hostilenetworks:data_model", id, 1);
            set_custom(&mut m, "hostilenetworks:data", Tag::Int(data.parse().unwrap_or(0)));
            set_custom(&mut m, "hostilenetworks:iterations", Tag::Int(iters.parse().unwrap_or(0)));
            out[i] = Some(m);
        }
    }
    out
}

fn set_learner_models(stack: &mut ItemStack, models: &[Option<ItemStack>]) {
    let text: Vec<String> = models
        .iter()
        .map(|m| match m {
            Some(m) => format!("{},{},{}", custom_str(m, "hostilenetworks:data_model").unwrap_or_default(), data_of(m), custom_i(m, "hostilenetworks:iterations")),
            None => String::new(),
        })
        .collect();
    set_custom(stack, "hostilenetworks:learner_inv", Tag::String(java(&text.join(";"))));
}

/// DeepLearnerItem.use: its four slots in a menu.
pub(in crate::game) fn use_item(_game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    if &*stack.item != "hostilenetworks:deep_learner" {
        return;
    }
    // The screen opens on the player's own client (`learner_frame`); the server only takes the use, and moves models
    // when that screen asks (`learner_message`).
    *claimed = true;
}

// ---- The Deep Learner's screen (DeepLearnerScreen and DeepLearnerMenu), on its player's own client.

const LEARNER: &str = "hostilenetworks:deep_learner";
const DATA_MODEL: &str = "hostilenetworks:data_model";
const NET_LEARNER: &str = "hostilenetworks:learner";
const LEARNER_W: f32 = 338.0;
const LEARNER_H: f32 = 235.0;
/// DeepLearnerMenu's slots, from the screen's corner: the learner's four, then the 27 of the inventory and the hotbar.
const LEARNER_SLOTS: [[f32; 2]; 4] = [[256.0, 99.0], [274.0, 99.0], [256.0, 117.0], [274.0, 117.0]];

/// A move the client asked for, done on the server in the player's inventory tick, checked again there.
#[derive(Clone, Copy, Debug, PartialEq)]
enum LearnerOp {
    /// One model from inventory slot n (0..36) into the learner's first free slot.
    Put(i32),
    /// The learner's model n (0..4) out into the inventory.
    Take(usize),
}

fn parse_learner_op(data: &[u8]) -> Option<LearnerOp> {
    if data.len() > 16 {
        return None;
    }
    let text = std::str::from_utf8(data).ok()?;
    let (verb, n) = text.split_once(' ')?;
    match (verb, n.parse::<i64>().ok()?) {
        ("put", n @ 0..=35) => Some(LearnerOp::Put(n as i32)),
        ("take", n @ 0..=3) => Some(LearnerOp::Take(n as usize)),
        _ => None,
    }
}

#[derive(Default)]
struct LearnerView {
    selected: usize,
    last_right: bool,
    /// A click on the screen, sent at the next frame (the press hook can't send).
    click: Option<LearnerOp>,
}

/// `net_from_client`: a player's learner screen asks for a move; at most four wait per player.
pub(in crate::game) fn learner_message(_game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if channel != NET_LEARNER {
        return;
    }
    let Some(op) = parse_learner_op(data) else { return };
    STATE.with_borrow_mut(|s| {
        if s.learner_ops.iter().filter(|(p, _)| *p == player).count() < 4 {
            s.learner_ops.push((player, op));
        }
    });
}

/// The moves `player` asked for, on the learner in hand at `learner_slot`: each checked against what is there now.
fn apply_learner_ops(game: &mut Game<'_>, player: u128, learner_slot: i32) {
    let ops: Vec<LearnerOp> = STATE.with_borrow_mut(|s| {
        let (mine, rest): (Vec<_>, Vec<_>) = s.learner_ops.drain(..).partition(|(p, _)| *p == player);
        s.learner_ops = rest;
        mine.into_iter().map(|(_, op)| op).collect()
    });
    for op in ops {
        let Some(learner) = game.native_carried_stack(learner_slot).cloned() else { return };
        let mut models = learner_models(&learner);
        match op {
            LearnerOp::Put(from) => {
                if from == learner_slot {
                    continue;
                }
                let Some(free) = models.iter().position(Option::is_none) else { continue };
                let Some(src) = game.native_carried_stack(from).filter(|s| &*s.item == DATA_MODEL && s.count > 0) else { continue };
                let mut one = src.clone();
                one.count = 1;
                src.count -= 1;
                models[free] = Some(one);
            }
            LearnerOp::Take(i) => {
                let Some(model) = models[i].take() else { continue };
                // Into the inventory as a shift click puts it; it stays in the learner when there is no room.
                if let Some(rest) = game.native_give_stack(player, model) {
                    models[i] = Some(rest);
                    continue;
                }
            }
        }
        if let Some(l) = game.native_carried_stack(learner_slot) {
            set_learner_models(l, &models);
        }
    }
}

/// The learner in the local player's hand: its slot (`native_carried_stack`'s numbers) and stack.
fn local_learner(game: &Game<'_>) -> Option<(i32, ItemStack)> {
    if let Some(s) = game.inventory.get(game.selected).and_then(|s| s.as_ref()).filter(|s| &*s.item == LEARNER) {
        return Some((game.selected as i32, s.clone()));
    }
    game.offhand.as_ref().filter(|s| &*s.item == LEARNER).map(|s| (-106, s.clone()))
}

/// Each frame on the client: a right click with a learner in hand opens its screen; a click there goes to the server.
fn learner_frame(game: &mut Game<'_>) {
    let right = game.right;
    let playing = game.player.is_some() && matches!(game.screen, super::Screen::Playing);
    let (edge, click) = STATE.with_borrow_mut(|s| {
        let edge = right && !s.learner.last_right;
        s.learner.last_right = right;
        (edge, s.learner.click.take())
    });
    if edge && playing && local_learner(game).is_some() && game.native_mod_screen_open(NS) {
        STATE.with_borrow_mut(|s| s.learner.selected = 0);
    }
    if let Some(op) = click {
        let text = match op {
            LearnerOp::Put(n) => format!("put {n}"),
            LearnerOp::Take(n) => format!("take {n}"),
        };
        let _ = game.native_net_send_to_server(NET_LEARNER, text.as_bytes());
    }
}

fn learner_origin(batch: &HudBatch) -> [f32; 2] {
    [((batch.width - LEARNER_W) / 2.0).floor(), ((batch.height - LEARNER_H) / 2.0).floor()]
}

/// The inventory slots the learner screen shows: (slot number, its corner from the screen's).
fn learner_inventory_slots() -> impl Iterator<Item = (i32, [f32; 2])> {
    let main = (0..27).map(|i| (9 + i, [89.0 + (i % 9) as f32 * 18.0, 153.0 + (i / 9) as f32 * 18.0]));
    let bar = (0..9).map(|i| (i, [89.0 + i as f32 * 18.0, 211.0]));
    main.chain(bar)
}

/// `screen_mouse_button_pressed_pre` on the learner's screen: a model in the inventory goes in, one in the learner
/// comes out, the arrows choose which model shows.
pub(in crate::game) fn learner_press(game: &Game<'_>, screen: &super::native_hooks::NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if game.native_mod_screen() != Some(NS) || button != 0 {
        return;
    }
    *canceled = true;
    let Some((held_slot, learner)) = local_learner(game) else { return };
    let o = STATE.with_borrow(|s| s.learner_origin);
    let at = [screen.mouse[0] - o[0], screen.mouse[1] - o[1]];
    let models = learner_models(&learner);
    let count = models.iter().flatten().count();
    let step = |d: i32| {
        STATE.with_borrow_mut(|s| {
            for k in 1..=4 {
                let i = (s.learner.selected as i32 + d * k).rem_euclid(4) as usize;
                if models[i].is_some() {
                    s.learner.selected = i;
                    break;
                }
            }
        })
    };
    if count > 1 && inside([-27.0, 105.0, 24.0, 24.0], at) {
        return step(-1);
    }
    if count > 1 && inside([-1.0, 105.0, 24.0, 24.0], at) {
        return step(1);
    }
    for (i, p) in LEARNER_SLOTS.iter().enumerate() {
        if inside([p[0] - 1.0, p[1] - 1.0, 18.0, 18.0], at) && models[i].is_some() {
            STATE.with_borrow_mut(|s| s.learner.click = Some(LearnerOp::Take(i)));
            return;
        }
    }
    for (n, p) in learner_inventory_slots() {
        if n != held_slot
            && inside([p[0] - 1.0, p[1] - 1.0, 18.0, 18.0], at)
            && game.inventory.get(n as usize).and_then(|s| s.as_ref()).is_some_and(|s| &*s.item == DATA_MODEL)
        {
            STATE.with_borrow_mut(|s| s.learner.click = Some(LearnerOp::Put(n)));
            return;
        }
    }
}

fn tier_rgb(name: &str) -> u32 {
    match name {
        "dark_gray" => 0x555555,
        "green" => 0x55FF55,
        "blue" => 0x5555FF,
        "light_purple" => 0xFF55FF,
        "gold" => 0xFFAA00,
        _ => 0xFFFFFF,
    }
}

/// The learner's screen, drawn over the world in `screen_render_post`.
fn learner_draw(game: &Game<'_>, batch: &mut HudBatch, mouse: [f32; 2]) {
    let Some((held_slot, learner)) = local_learner(game) else { return };
    let o = learner_origin(batch);
    STATE.with_borrow_mut(|s| s.learner_origin = o);
    let (left, top) = (o[0], o[1]);
    let tex = "hostilenetworks:gui/deep_learner";
    let models = learner_models(&learner);
    let count = models.iter().flatten().count();
    let selected = STATE.with_borrow_mut(|s| {
        if models[s.learner.selected].is_none() {
            s.learner.selected = models.iter().position(Option::is_some).unwrap_or(0);
        }
        s.learner.selected
    });
    let white = 0xFFFFFF;
    let aqua = 0x55FFFF;
    let line = 12.0;
    sprite(game, batch, tex, o, 41.0, 0.0, 0.0, 0.0, 256.0, 140.0);
    sprite(game, batch, "hostilenetworks:gui/default_gui", o, 81.0, 145.0, 0.0, 0.0, 176.0, 90.0);
    let stats_title = lang("hostilenetworks.gui.stats", "Statistics");
    let stats_w = game.native_text_width(&stats_title);
    let current = models[selected].as_ref().and_then(|m| model_of(game, m).map(|model| (m, model)));
    if let (Some((stack, model)), Some(t)) = (current, table(game)) {
        sprite(game, batch, tex, o, -41.0, 0.0, 9.0, 140.0, 75.0, 101.0);
        let spin = (game.world.game_time % 160) as f32 * 2.25;
        mob_preview(game, batch, model, [left - 4.0, top + 90.0], 40.0, 180.0 + spin, [left - 41.0, top, left + 34.0, top + 101.0]);
        // The text: name and trivia, then the tier, accuracy and what the next one needs.
        game.native_text(batch, &lang("hostilenetworks.gui.name", "Name"), [left + 49.0, top + 6.0], aqua, true);
        let text_w = 200.0 - stats_w + 36.0;
        let name = wrap_text(game, &model_name(model), text_w);
        for (i, l) in name.iter().enumerate() {
            game.native_text(batch, l, [left + 49.0, top + 6.0 + line * (1 + i) as f32], white, true);
        }
        let info_y = top + 6.0 + line * (1 + name.len()) as f32;
        game.native_text(batch, &lang("hostilenetworks.gui.info", "Information"), [left + 49.0, info_y], aqua, true);
        let trivia = lang(&model.trivia, if model.trivia == "hostilenetworks.trivia.auto" { "Learned from this mob's own health and loot." } else { "" });
        for (i, l) in wrap_text(game, &trivia, text_w).iter().enumerate().take(5) {
            game.native_text(batch, l, [left + 49.0, info_y + line * (1 + i) as f32], white, true);
        }
        let d = data_of(stack);
        let tier = tier_of(t, model, d);
        let tier_name = lang(&format!("hostilenetworks.tier.{}", tier.name), &tier.name);
        let data_y = top + 6.0 + line * 8.0;
        let prefix = lang("hostilenetworks.gui.tier", "Model Tier: %s").replace("%s", "");
        game.native_text(batch, &prefix, [left + 49.0, data_y], white, true);
        game.native_text(batch, &tier_name, [left + 49.0 + game.native_text_width(&prefix), data_y], tier_rgb(&tier.color), true);
        let acc = lang("hostilenetworks.gui.accuracy", "Model Accuracy: %s").replace("%s", &format!("{:.2}%", accuracy(t, model, d) * 100.0));
        game.native_text(batch, &acc, [left + 49.0, data_y + line], white, true);
        let third = if is_max(t, tier) {
            (lang("hostilenetworks.gui.max_tier", "You cannot improve me further"), 0xFF5555)
        } else if !config().action_upgrades_model {
            (lang("hostilenetworks.gui.upgrade_disabled", "Models cannot be upgraded by world actions."), white)
        } else {
            let next = next_tier(t, tier);
            let per = gained(model, tier).max(1);
            let actions = ((required(model, next) - d).max(0) + per - 1) / per;
            let word = match (model.is_block, actions > 1) {
                (false, false) => lang("hostilenetworks.gui.action.kill", "kill"),
                (false, true) => lang("hostilenetworks.gui.action.kills", "kills"),
                (true, false) => lang("hostilenetworks.gui.action.mine", "mine"),
                (true, true) => lang("hostilenetworks.gui.action.mines", "mines"),
            };
            let next_name = lang(&format!("hostilenetworks.tier.{}", next.name), &next.name);
            (lang("hostilenetworks.gui.next_tier", "Upgrades to %s in %s %s").replacen("%s", &next_name, 1).replacen("%s", &actions.to_string(), 1).replacen("%s", &word, 1), white)
        };
        for (i, l) in wrap_text(game, &third.0, 200.0).iter().enumerate() {
            game.native_text(batch, l, [left + 49.0, data_y + line * (2 + i) as f32], third.1, true);
        }
        // The statistics: health, armour and experience, with their icons (the texture's first column).
        let sx = left + LEARNER_W - 49.0 - stats_w;
        game.native_text(batch, &stats_title, [sx, top + 6.0], aqua, true);
        let kind = game.res.mob_types().get(&model.source).cloned();
        let stats = match &kind {
            Some(k) if !model.is_block => [
                format!("{}", k.max_health / 2.0),
                format!("{}", k.armor),
                if k.category == "monster" { "5".to_owned() } else { "1-3".to_owned() },
            ],
            _ => ["-".to_owned(), "-".to_owned(), "-".to_owned()],
        };
        for (i, v) in stats.iter().enumerate() {
            let y = top + 8.0 + 9.0 + 11.0 * i as f32;
            sprite(game, batch, tex, o, LEARNER_W - 49.0 - stats_w, 8.0 + 9.0 + 11.0 * i as f32, 0.0, 140.0 + 9.0 * i as f32, 9.0, 9.0);
            game.native_text(batch, v, [left + LEARNER_W - 36.0 - stats_w, y + 1.0], white, true);
        }
        if count > 1 {
            for (x, name) in [(-27.0, "left"), (-1.0, "right")] {
                let hover = inside([left + x, top + 105.0, 24.0, 24.0], mouse);
                let sprite_name = format!("hostilenetworks:gui/sprites/widget/{name}{}", if hover { "_hovered" } else { "" });
                game.native_sprite(batch, &sprite_name, [0.0, 0.0, 24.0, 24.0], [left + x, top + 105.0, 24.0, 24.0]);
            }
        }
    } else {
        for i in 0..7 {
            let l = lang(&format!("hostilenetworks.gui.learner_empty.{i}"), "");
            game.native_text(batch, &l, [left + 49.0, top + 6.0 + line * i as f32], if i == 0 { aqua } else { white }, true);
        }
    }
    // The slots: the learner's four, then the inventory (the learner in hand can't be moved).
    let mut hovered: Option<ItemStack> = None;
    let mut slot = |game: &Game<'_>, batch: &mut HudBatch, p: [f32; 2], stack: Option<&ItemStack>, locked: bool| {
        let r = [left + p[0], top + p[1], 16.0, 16.0];
        if let Some(s) = stack {
            game.native_item_stack(batch, s, [r[0], r[1]], 16.0);
        }
        if locked {
            game.native_fill(batch, r, 0x80000000);
        } else if inside(r, mouse) {
            game.native_fill(batch, r, 0x80FFFFFF);
            hovered = stack.cloned();
        }
    };
    for (i, p) in LEARNER_SLOTS.iter().enumerate() {
        slot(game, batch, *p, models[i].as_ref(), false);
    }
    for (n, p) in learner_inventory_slots() {
        slot(game, batch, p, game.inventory.get(n as usize).and_then(|s| s.as_ref()), n == held_slot);
    }
    if let Some(s) = hovered {
        let mut lines: Vec<(horizon_client::text::Text, u32)> = Vec::new();
        let (ns, path) = s.item.split_once(':').unwrap_or(("minecraft", &s.item));
        lines.push((horizon_client::text::Text::literal(lang(&format!("item.{ns}.{path}"), &lang(&format!("block.{ns}.{path}"), path))), u32::MAX));
        tooltip(game, &s, &mut lines);
        game.draw_tooltip_lines(batch, &lines, mouse[0], mouse[1]);
    }
}

// ---- Mobs drawn in screens: a Data Model's own mob on its icon, and the learner's spinning one.

/// `renderEntityInInventory`: model `model`'s mob with its feet at `feet`, `size` GUI pixels a block (times the
/// model's display scale), turned `yaw` degrees (180 faces the viewer), its quads depth-sorted and clipped to `clip`.
/// The quads of a still mob preview, from its feet, by model, size and yaw: an icon's mob is the same every frame.
type PreviewQuads = Vec<([[f32; 2]; 4], [[f32; 2]; 4], u32)>;
thread_local! {
    static PREVIEWS: RefCell<HashMap<(String, u32, u32), std::rc::Rc<PreviewQuads>>> = RefCell::new(HashMap::new());
}

/// What stands in for a mob Horizon draws only live (the dragon): its head, else its spawn egg.
fn mob_stand_in(game: &Game<'_>, model: &Model) -> Option<String> {
    let (ns, path) = model.source.split_once(':')?;
    let head = if path == "ender_dragon" { "minecraft:dragon_head".to_owned() } else { format!("{ns}:{path}_head") };
    [head, format!("{ns}:{path}_spawn_egg")].into_iter().find(|i| game.res.items().get(i).is_some())
}

fn mob_preview(game: &Game<'_>, batch: &mut HudBatch, model: &Model, feet: [f32; 2], size: f32, yaw: f32, clip: [f32; 4]) {
    mob_preview_with(game, batch, model, feet, size, yaw, clip, false);
}

#[allow(clippy::too_many_arguments)]
fn mob_preview_with(game: &Game<'_>, batch: &mut HudBatch, model: &Model, feet: [f32; 2], size: f32, yaw: f32, clip: [f32; 4], still: bool) {
    let stand_in = |batch: &mut HudBatch, item: &str| {
        let s = (size * 1.6).min(clip[2] - clip[0]);
        game.native_item(batch, item, [feet[0] - s / 2.0, feet[1] - s], s);
    };
    if model.is_block {
        let s = size * model.scale * 0.6;
        game.native_item(batch, &model.source, [feet[0] - s / 2.0, feet[1] - s], s);
        return;
    }
    if still {
        let key = (model.id.clone(), size.to_bits(), yaw.to_bits());
        let cached = PREVIEWS.with_borrow(|p| p.get(&key).cloned());
        let quads = match cached {
            Some(q) => q,
            None => {
                let mut scratch = HudBatch::new([4096, 4096]);
                mob_preview_with(game, &mut scratch, model, [0.0, 0.0], size, yaw, [-4096.0, -4096.0, 4096.0, 4096.0], false);
                // `HudBatch::quad` writes two triangles, corners 0 1 2 0 2 3.
                let q: PreviewQuads = scratch.vertices.as_chunks::<6>().0.iter().map(|v| ([v[0].pos, v[1].pos, v[2].pos, v[5].pos], [v[0].uv, v[1].uv, v[2].uv, v[5].uv], v[0].color)).collect();
                let q = std::rc::Rc::new(q);
                PREVIEWS.with_borrow_mut(|p| p.insert(key, q.clone()));
                q
            }
        };
        if quads.is_empty() {
            if let Some(item) = mob_stand_in(game, model) {
                stand_in(batch, &item);
            }
            return;
        }
        let start = batch.vertices.len();
        for (p, uv, color) in quads.iter() {
            batch.quad(p.map(|v| [v[0] + feet[0], v[1] + feet[1]]), *uv, *color);
        }
        batch.clip_since(start, clip[0], clip[1], clip[2], clip[3]);
        return;
    }
    let Some(kind) = game.res.mob_types().get(&model.source).cloned() else {
        if let Some(item) = mob_stand_in(game, model) {
            stand_in(batch, &item);
        }
        return;
    };
    let mut m = horizon_client::mobs::Mob::new(0, kind, horizon_core::geometry::Vec3::new(0.0, 0.0, 0.0), 0.0);
    m.previous = m.position;
    m.body_yaw = yaw;
    m.previous_body_yaw = yaw;
    m.head_yaw = yaw;
    m.previous_head_yaw = yaw;
    m.age = (game.world.game_time.max(0) as u32) % 24000;
    let camera = glam::DVec3::ZERO;
    let atlas = &game.gui.atlas;
    let lookup = |name: &str| atlas.sprites.get(name).map(|r| [r.u0, r.v0, r.u1, r.v1]);
    let mut verts = Vec::new();
    horizon_client::mobs::cem_anim::in_gui(|| {
        horizon_client::mobs::render::render_with(&m, 1.0, camera, &lookup, 240 | 240 << 8, game.res.mob_data(), &[], &mut verts);
    });
    let s = size * model.scale;
    let gui = glam::Mat4::from_translation(glam::Vec3::new(feet[0] + model.offset[0] * s, feet[1] - model.offset[1] * s, 0.0))
        * glam::Mat4::from_scale(glam::Vec3::new(s, s, -s))
        * glam::Mat4::from_rotation_z(std::f32::consts::PI);
    let mut quads: Vec<(f32, [[f32; 2]; 4], [[f32; 2]; 4], u32)> = Vec::new();
    for q in verts.as_chunks::<4>().0 {
        let p: [glam::Vec3; 4] = std::array::from_fn(|i| gui.transform_point3(glam::Vec3::from_array(q[i].pos)));
        let n = (p[1] - p[0]).cross(p[2] - p[0]);
        if n.z >= 0.0 {
            continue;
        }
        let n = n.normalize_or_zero();
        let shade = (0.55 + 0.45 * -n.z - 0.15 * n.y).clamp(0.4, 1.0);
        let c = q[0].color;
        let ch = |sh: u32| ((((c >> sh) & 255) as f32 * shade) as u32) << sh;
        let color = (c & 0xFF00_0000) | ch(0) | ch(8) | ch(16);
        let depth = p.iter().map(|v| v.z).sum::<f32>() / 4.0;
        quads.push((depth, p.map(|v| [v.x, v.y]), std::array::from_fn(|i| q[i].uv), color));
    }
    quads.sort_by(|a, b| a.0.total_cmp(&b.0));
    if quads.is_empty()
        && let Some(item) = mob_stand_in(game, model)
    {
        return stand_in(batch, &item);
    }
    let start = batch.vertices.len();
    for (_, p, uv, color) in quads {
        batch.quad(p, uv, color);
    }
    batch.clip_since(start, clip[0], clip[1], clip[2], clip[3]);
}

/// `item_decorations`: a Data Model's icon is the base plate; its mob stands on it (DataModelItemStackRenderer, GUI).
pub(in crate::game) fn item_decorations(game: &Game<'_>, stack: &ItemStack, x: f32, y: f32, batch: &mut HudBatch) {
    if &*stack.item != DATA_MODEL || !game.native_on(NS) {
        return;
    }
    let Some(model) = model_of(game, stack) else { return };
    // translate(.5, .5) then down 0.32 of the icon: the feet; 0.4 of the icon a block; turned 30 degrees.
    mob_preview_with(game, batch, model, [x + 8.0, y + 8.0 + 0.32 * 16.0], 16.0 * 0.4, 150.0, [x - 2.0, y - 6.0, x + 18.0, y + 17.0], true);
}

/// The menu shut: what's in its slots goes back on the learner in hand.
pub(in crate::game) fn menu_input_closed(game: &mut Game<'_>, uuid: u128, items: &mut Vec<Option<ItemStack>>) {
    let open = STATE.with_borrow_mut(|s| s.open_learner.remove(&uuid).is_some());
    if !open {
        return;
    }
    let models: Vec<Option<ItemStack>> = items.iter().take(4).map(|m| m.clone().filter(|x| &*x.item == "hostilenetworks:data_model")).collect();
    let rest: Vec<usize> = items.iter().enumerate().take(4).filter(|(_, m)| m.as_ref().is_some_and(|x| &*x.item == "hostilenetworks:data_model")).map(|(i, _)| i).collect();
    if let Some(held) = game.held_mut()
        && &*held.item == "hostilenetworks:deep_learner"
    {
        set_learner_models(held, &models);
        for i in rest {
            items[i] = None;
        }
    }
}

/// DataCenterIOPortBlock: a click cycles what it proxies.
pub(in crate::game) fn right_click(game: &mut Game<'_>, pos: Pos, canceled: &mut bool) {
    let block = game.native_block(pos);
    if block == "hostilenetworks:data_center_io_port" {
        let now = game.native_block_property(pos, "mode").unwrap_or_default();
        let next = match now.as_str() {
            "energy" => "models",
            "models" => "inputs",
            "inputs" => "outputs",
            _ => "energy",
        };
        game.native_set_block_property_with(pos, "mode", next, 3);
        if let Some(p) = game.native_acting_player() {
            let word = lang(&format!("hostilenetworks.io_port.mode.{next}"), next);
            let line = lang("hostilenetworks.io_port.mode_set", "Mode: %s").replace("%s", &word);
            tell(game, p.uuid, &line);
        }
        *canceled = true;
    }
    if block == "hostilenetworks:sim_chamber" || block == "hostilenetworks:loot_fabricator" || block == "hostilenetworks:data_center" {
        let held = game.held().cloned();
        if let Some(h) = held
            && &*h.item == "hostilenetworks:fab_directive"
        {
            directive(game, pos, &h);
            *canceled = true;
        }
    }
}

/// FabDirectiveItem: right click copies the machine's choices, sneaking applies them.
fn directive(game: &mut Game<'_>, pos: Pos, held: &ItemStack) {
    let sneaking = game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft) || game.player.as_ref().is_some_and(|p| p.pose == horizon_client::pose::Pose::Crouching);
    let uuid = game.native_acting_player().map(|p| p.uuid);
    if sneaking {
        let Some(text) = custom_str(held, "hostilenetworks:fab_selections") else { return };
        let applied = game.native_with_mod_block_entity(pos, |e| {
            let mut n = 0;
            for part in text.split('|') {
                let Some((model, list)) = part.split_once('=') else { continue };
                let v: Vec<i32> = list.split(',').filter_map(|x| x.parse().ok()).collect();
                set_selection(&mut e.nbt, model, v);
                n += 1;
            }
            n
        });
        if let (Some(uuid), Some(n)) = (uuid, applied) {
            tell(game, uuid, &format!("§aApplied {n} selections"));
        }
    } else {
        let text = game.native_with_mod_block_entity(pos, |e| {
            e.nbt
                .iter()
                .filter_map(|(k, v)| {
                    let k = k.to_string().ok()?;
                    let model = k.strip_prefix("sel.")?.to_owned();
                    let Tag::IntArray(a) = v else { return None };
                    Some(format!("{model}={}", a.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")))
                })
                .collect::<Vec<_>>()
                .join("|")
        });
        if let Some(text) = text {
            let n = text.split('|').filter(|p| !p.is_empty()).count();
            if let Some(h) = game.held_mut() {
                set_custom(h, "hostilenetworks:fab_selections", Tag::String(java(&text)));
            }
            if let Some(uuid) = uuid {
                tell(game, uuid, &format!("§aCopied {n} selections"));
            }
        }
    }
}

/// getStateForPlacement: it faces the player.
pub(in crate::game) fn placed(game: &mut Game<'_>, pos: Pos) {
    let block = game.native_block(pos);
    if !matches!(block.as_str(), "hostilenetworks:sim_chamber" | "hostilenetworks:loot_fabricator" | "hostilenetworks:data_center") {
        return;
    }
    let Some(player) = game.native_acting_player() else { return };
    let y = player.yaw.rem_euclid(360.0);
    let facing = if !(45.0..315.0).contains(&y) {
        "north"
    } else if y < 135.0 {
        "east"
    } else if y < 225.0 {
        "south"
    } else {
        "west"
    };
    game.native_set_block_property_with(pos, "facing", facing, 3);
}

// ---- Screens

pub fn screen(block: &str, out: &mut Option<super::native_hooks::NativeScreen>) {
    use super::native_hooks::NativeScreen;
    match block {
        "hostilenetworks:sim_chamber" => {
            *out = Some(NativeScreen {
                width: 232.0,
                height: 230.0,
                slots: vec![Some([-13.0, 1.0]), Some([176.0, 7.0]), Some([196.0, 7.0]), Some([186.0, 27.0])],
                // SimChamberMenu.addPlayerSlots(pInv, 36, 153).
                inventory_y: 153.0,
                inventory_x: 36.0,
                own_labels: true,
                own_background: true,
            });
        }
        "hostilenetworks:loot_fabricator" => {
            let mut slots = vec![Some([79.0, 62.0])];
            for y in 0..4 {
                for x in 0..4 {
                    slots.push(Some([100.0 + x as f32 * 18.0, 7.0 + y as f32 * 18.0]));
                }
            }
            *out = Some(NativeScreen { width: 176.0, height: 178.0, slots, inventory_y: 96.0, inventory_x: 8.0, own_labels: true, own_background: true });
        }
        "hostilenetworks:data_center" => {
            let mut slots = Vec::new();
            for row in 0..5 {
                for col in 0..5 {
                    slots.push(Some([9.0 + col as f32 * 18.0, 21.0 + row as f32 * 20.0]));
                }
            }
            for col in 0..4 {
                slots.push(Some([132.0 + col as f32 * 18.0, 20.0]));
            }
            for row in 0..4 {
                for col in 0..4 {
                    slots.push(Some([132.0 + col as f32 * 18.0, 48.0 + row as f32 * 18.0]));
                }
            }
            *out = Some(NativeScreen { width: 230.0, height: 230.0, slots, inventory_y: 140.0, inventory_x: 35.0, own_labels: true, own_background: false });
        }
        _ => {}
    }
}

/// `text`'s lines (its own line breaks kept), each broken at spaces to fit `width` GUI pixels.
fn wrap_text(game: &Game<'_>, text: &str, width: f32) -> Vec<String> {
    wrap_words(text, width, |s| game.native_text_width(s))
}

fn wrap_words(text: &str, width: f32, measure: impl Fn(&str) -> f32) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ').filter(|w| !w.is_empty()) {
            let tried = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
            if measure(&tried) <= width || line.is_empty() {
                line = tried;
            } else {
                out.push(std::mem::take(&mut line));
                line = word.to_owned();
            }
        }
        out.push(line);
    }
    out
}

fn sprite(game: &Game<'_>, batch: &mut HudBatch, texture: &str, o: [f32; 2], x: f32, y: f32, u: f32, v: f32, w: f32, h: f32) {
    game.native_sprite(batch, texture, [u, v, w, h], [o[0] + x, o[1] + y, w, h]);
}

/// The viewed block's slot as the client has it.
fn viewed_stack(game: &Game<'_>, slot: usize) -> Option<ItemStack> {
    match game.container.and_then(|k| game.world.block_entities.get(&k))? {
        horizon_client::containers::BlockEntity::Mod(m) => m.items.get(slot).cloned().flatten(),
        _ => None,
    }
}

fn viewed_selection(game: &Game<'_>, model: &str) -> Vec<i32> {
    match game.container.and_then(|k| game.world.block_entities.get(&k)) {
        Some(horizon_client::containers::BlockEntity::Mod(m)) => selection(&m.nbt, model),
        _ => Vec::new(),
    }
}

/// What a client needs of a machine beyond its menu: the choices it shows.
pub fn update_tag(entity: &ModBlockEntity, tag: &mut Option<Compound>) {
    if is_ours(&entity.id) && tag.is_none() {
        *tag = Some(entity.nbt.clone());
    }
}

pub(in crate::game) fn screen_draw(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let mouse = STATE.with_borrow_mut(|s| {
        s.shown = Some((game.container, block.to_owned(), data.to_vec(), origin));
        s.mouse
    });
    match block {
        "hostilenetworks:sim_chamber" if data.len() >= 9 => {
            let tex = "hostilenetworks:gui/sim_chamber";
            sprite(game, batch, tex, origin, 8.0, 0.0, 0.0, 0.0, 216.0, 141.0);
            sprite(game, batch, tex, origin, -14.0, 0.0, 0.0, 141.0, 18.0, 18.0);
            sprite(game, batch, tex, origin, 228.0, 0.0, 0.0, 141.0, 18.0, 18.0);
            sprite(game, batch, tex, origin, 228.0, 20.0, 0.0, 141.0, 18.0, 18.0);
            let (en, cap) = (wide(data, 3, 7), wide(data, 4, 8));
            let h = if cap > 0 { (87.0 * (f64::from(en) / f64::from(cap))) as f32 } else { 0.0 };
            sprite(game, batch, tex, origin, 211.0, 48.0 + 87.0 - h, 18.0, 141.0 + 87.0 - h, 7.0, h);
            sprite(game, batch, "hostilenetworks:gui/default_gui", origin, 28.0, 145.0, 0.0, 0.0, 176.0, 90.0);
            let stack = viewed_stack(game, 0);
            let model = stack.as_ref().and_then(|s| model_of(game, s));
            let t = table(game);
            if let (Some(m), Some(t), Some(s)) = (model, t, stack.as_ref()) {
                let d = data_of(s);
                let tier = tier_of(t, m, d);
                let (a, b) = (required(m, tier), required(m, next_tier(t, tier)));
                let hh = if is_max(t, tier) { 87.0 } else { 87.0 * (d - a) as f32 / (b - a).max(1) as f32 };
                sprite(game, batch, tex, origin, 14.0, 48.0 + 87.0 - hh, 18.0, 141.0 + 87.0 - hh, 7.0, hh);
                let accuracy_now = accuracy(t, m, d) * 100.0;
                // "Name: %s", the model's name in its colour (DataModel.name()).
                let label = format!("{}: ", lang("hostilenetworks.gui.name", "Name"));
                game.native_text(batch, &label, [origin[0] + 28.0, origin[1] + 9.0], 0xFFFFFF, true);
                game.native_text(batch, &model_name(m), [origin[0] + 28.0 + game.native_text_width(&label), origin[1] + 9.0], name_rgb(&m.color), true);
                game.native_text(batch, &lang("hostilenetworks.gui.tier", "Model Tier: %s").replace("%s", &tier.name), [origin[0] + 28.0, origin[1] + 21.0], 0xFFFFFF, true);
                game.native_text(batch, &lang("hostilenetworks.gui.accuracy", "Model Accuracy: %s").replace("%s", &format!("{accuracy_now:.2}%")), [origin[0] + 28.0, origin[1] + 33.0], 0xFFFFFF, true);
            }
            if data[2] != F_NONE && data[2] < 13 {
                let msg = lang(&format!("hostilenetworks.fail.{}", FAIL_KEYS[data[2] as usize]), "");
                // Font.split at the black panel's width (28 to 206), as the Java screen keeps it inside.
                for (i, line) in wrap_text(game, &msg, 178.0).iter().enumerate() {
                    game.native_text(batch, line, [origin[0] + 28.0, origin[1] + 60.0 + 12.0 * i as f32], 0xFF5555, true);
                }
            } else if data[0] > 0 {
                let pct = 100 - data[0] * 100 / data[1].max(1);
                game.native_text(batch, &format!("{pct}%"), [origin[0] + 184.0, origin[1] + 123.0], 0x62D8FF, true);
            }
            // Redstone and mode, as the buttons beside it.
            let reds = ["minecraft:gunpowder", "minecraft:redstone", "minecraft:redstone_torch"];
            game.native_item(batch, reds[(data[5].clamp(0, 2)) as usize], [origin[0] + 229.0, origin[1] + 1.0], 16.0);
            game.native_item(batch, if data[6] == 0 { "minecraft:ender_eye" } else { "minecraft:experience_bottle" }, [origin[0] + 229.0, origin[1] + 21.0], 16.0);
            let _ = mouse;
        }
        "hostilenetworks:loot_fabricator" if data.len() >= 6 => {
            let tex = "hostilenetworks:gui/loot_fabricator";
            sprite(game, batch, tex, origin, 0.0, 0.0, 0.0, 0.0, 176.0, 83.0);
            sprite(game, batch, "hostilenetworks:gui/default_gui", origin, 0.0, 88.0, 0.0, 0.0, 176.0, 90.0);
            let (en, cap) = (wide(data, 2, 4), wide(data, 3, 5));
            let h = if cap > 0 { (53.0 * (f64::from(en) / f64::from(cap))) as f32 } else { 0.0 };
            sprite(game, batch, tex, origin, 6.0, 10.0 + 53.0 - h, 0.0, 83.0 + 53.0 - h, 7.0, h);
            let w = (35.0 * data[0] as f32 / 60.0).clamp(0.0, 35.0);
            sprite(game, batch, tex, origin, 84.0, 23.0, 7.0, 83.0, 6.0, w);
            let page = STATE.with_borrow(|s| s.page);
            if let Some(m) = viewed_stack(game, 0).as_ref().and_then(|s| model_of(game, s)) {
                let sel = viewed_selection(game, &m.id);
                let chosen = current_drop(&sel);
                for (i, drop) in m.fab.iter().enumerate().skip(page * 9).take(9) {
                    let at = i - page * 9;
                    let (x, y) = (18.0 + 18.0 * (at % 3) as f32, 10.0 + 18.0 * (at / 3) as f32);
                    game.native_item(batch, &drop.0, [origin[0] + x, origin[1] + y], 16.0);
                    if chosen == i as i32 {
                        game.native_fill(batch, [origin[0] + x, origin[1] + y, 16.0, 16.0], 0x6000FF00);
                    }
                }
                if chosen >= 0
                    && let Some(d) = m.fab.get(chosen as usize)
                {
                    game.native_item(batch, &d.0, [origin[0] + 79.0, origin[1] + 5.0], 16.0);
                }
            }
        }
        "hostilenetworks:data_center" if data.len() >= 56 => {
            game.native_fill(batch, [origin[0], origin[1], 230.0, 140.0], 0xFF2B2B2B);
            let tex = "hostilenetworks:gui/data_center";
            sprite(game, batch, tex, origin, 0.0, 0.0, 0.0, 0.0, 230.0, 140.0);
            let title = lang("block.hostilenetworks.data_center", "Data Center");
            game.native_text(batch, &title, [origin[0] + 8.0, origin[1] + 6.0], 0xFFFFFF, true);
            if data[51] == 0 {
                game.native_text(batch, &lang("hostilenetworks.fail.shell_broken", "Multiblock shell is incomplete or invalid"), [origin[0] + 8.0, origin[1] + 126.0], 0xFF5555, true);
            } else {
                let (en, cap) = (wide(data, 52, 54), wide(data, 53, 55));
                let per = f64::from(en) / f64::from(cap.max(1)) * 100.0;
                game.native_text(batch, &format!("{en} FE ({per:.0}%)"), [origin[0] + 8.0, origin[1] + 126.0], 0x62D8FF, true);
            }
            for i in 0..25 {
                let (x, y) = (9.0 + (i % 5) as f32 * 18.0, 21.0 + (i / 5) as f32 * 20.0 + 16.0);
                let pct = if data[i] > 0 { (300 - data[i]) as f32 / 300.0 } else { 0.0 };
                game.native_fill(batch, [origin[0] + x, origin[1] + y, 16.0 * pct, 2.0], 0xFF62D8FF);
            }
        }
        _ => {}
    }
}

pub(in crate::game) fn screen_click(game: &Game<'_>, block: &str, data: &[i32], at: [f32; 2], right: bool, shift: bool, button: &mut Option<i32>) {
    match block {
        "hostilenetworks:sim_chamber" if data.len() >= 9 => {
            if inside([228.0, 0.0, 18.0, 18.0], at) {
                *button = Some(0);
            } else if inside([228.0, 20.0, 18.0, 18.0], at) {
                *button = Some(1);
            }
        }
        "hostilenetworks:loot_fabricator" if data.len() >= 6 => {
            if inside([-22.0, 0.0, 18.0, 18.0], at) {
                *button = Some(1000);
                return;
            }
            let Some(m) = viewed_stack(game, 0).as_ref().and_then(|s| model_of(game, s)) else { return };
            let page = STATE.with_borrow(|s| s.page);
            if inside([13.0, 68.0, 29.0, 12.0], at) {
                STATE.with_borrow_mut(|s| s.page = s.page.saturating_sub(1));
            } else if inside([46.0, 68.0, 29.0, 12.0], at) {
                if (page + 1) * 9 < m.fab.len() {
                    STATE.with_borrow_mut(|s| s.page += 1);
                }
            } else if inside([79.0, 5.0, 16.0, 16.0], at) {
                *button = Some(if right { 1001 } else { 1002 });
            } else {
                for i in 0..9 {
                    let (x, y) = (18.0 + 18.0 * (i % 3) as f32, 10.0 + 18.0 * (i / 3) as f32);
                    if inside([x, y, 16.0, 16.0], at) && page * 9 + i < m.fab.len() {
                        *button = Some(((page * 9 + i) as i32) * 2 + i32::from(shift));
                    }
                }
            }
        }
        "hostilenetworks:data_center" if data.len() >= 56 => {
            if inside([200.0, 4.0, 18.0, 12.0], at) {
                *button = Some(1000);
            }
            // Right click on a model cycles what that model makes.
            if right {
                for i in 0..25 {
                    let (x, y) = (9.0 + (i % 5) as f32 * 18.0, 21.0 + (i / 5) as f32 * 20.0);
                    if inside([x, y, 16.0, 16.0], at) {
                        *button = Some(2000 + i as i32);
                    }
                }
            }
        }
        _ => {}
    }
}

/// The game's own path for a screen's tooltips draws them before the JEI panel, which then covers them: they are
/// drawn after it instead, in `screen_post`.
pub(in crate::game) fn screen_tooltip(_game: &Game<'_>, _block: &str, _data: &[i32], at: [f32; 2], _lines: &mut Vec<String>) {
    STATE.with_borrow_mut(|s| s.mouse = at);
}

/// `native_screen_render_post`: the machine screen's tooltips, over everything drawn beside the screen.
pub(in crate::game) fn screen_post(game: &Game<'_>, batch: &mut HudBatch, screen: &super::native_hooks::NativeScreenView<'_>) {
    if game.native_mod_screen() == Some(NS) {
        return learner_draw(game, batch, screen.mouse);
    }
    let Some((key, block, data, origin)) = STATE.with_borrow_mut(|s| s.shown.take()) else { return };
    if key != game.container || game.carried.is_some() || kit_guide::is_open() {
        return;
    }
    let (cx, cy) = game.cursor;
    let at = [cx - origin[0], cy - origin[1]];
    let mut lines = Vec::new();
    machine_tooltip(game, &block, &data, at, &mut lines);
    let lines: Vec<(horizon_client::text::Text, u32)> = lines.into_iter().map(|l| (horizon_client::text::Text::literal(l), u32::MAX)).collect();
    game.draw_tooltip_lines(batch, &lines, cx, cy);
}

fn machine_tooltip(game: &Game<'_>, block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    if block == "hostilenetworks:sim_chamber" && data.len() >= 9 {
        if inside([211.0, 48.0, 7.0, 87.0], at) {
            lines.push(format!("{} / {} FE", wide(data, 3, 7), wide(data, 4, 8)));
        }
        if inside([228.0, 0.0, 18.0, 18.0], at) {
            lines.push(["Redstone: ignored", "Redstone: with a signal", "Redstone: without a signal"][data[5].clamp(0, 2) as usize].to_owned());
        }
        if inside([228.0, 20.0, 18.0, 18.0], at) {
            lines.push(if data[6] == 0 { "Inference" } else { "Training" }.to_owned());
        }
    }
    if block == "hostilenetworks:loot_fabricator" && data.len() >= 6 {
        if inside([6.0, 10.0, 7.0, 53.0], at) {
            lines.push(format!("{} / {} FE", wide(data, 2, 4), wide(data, 3, 5)));
        }
        if let Some(m) = viewed_stack(game, 0).as_ref().and_then(|s| model_of(game, s)) {
            let page = STATE.with_borrow(|s| s.page);
            for i in 0..9 {
                let (x, y) = (18.0 + 18.0 * (i % 3) as f32, 10.0 + 18.0 * (i / 3) as f32);
                if inside([x, y, 16.0, 16.0], at)
                    && let Some(d) = m.fab.get(page * 9 + i)
                {
                    let (ns, path) = d.0.split_once(':').unwrap_or(("minecraft", &d.0));
                    let name = lang(&format!("item.{ns}.{path}"), &lang(&format!("block.{ns}.{path}"), path));
                    lines.push(format!("{} x {name}", d.1));
                }
            }
        }
    }
}

pub(in crate::game) fn menu_button(entity: &mut ModBlockEntity, id: i32, game: &Game<'_>) {
    match entity.id.as_str() {
        "hostilenetworks:sim_chamber" => {
            if id == 0 {
                let r = (get_i(&entity.nbt, "redstoneState", 0) + 1) % 3;
                set_i(&mut entity.nbt, "redstoneState", r);
            } else if id == 1 {
                let m = get_i(&entity.nbt, "simMode", 0);
                set_i(&mut entity.nbt, "simMode", if m == 0 && config().sim_model_upgrade != 0 { 1 } else { 0 });
                set_i(&mut entity.nbt, "runtime", 0);
            }
        }
        "hostilenetworks:loot_fabricator" => {
            if id == 1000 {
                let r = (get_i(&entity.nbt, "redstoneState", 0) + 1) % 3;
                set_i(&mut entity.nbt, "redstoneState", r);
                return;
            }
            let Some(m) = entity.items.first().and_then(|s| s.as_ref()).and_then(|s| model_of(game, s)) else { return };
            let id_str = m.id.clone();
            if id == 1001 {
                set_selection(&mut entity.nbt, &id_str, Vec::new());
            } else if id == 1002 {
                // cycleMode: fixed and queue.
                let sel = selection(&entity.nbt, &id_str);
                let next = match sel.as_slice() {
                    [0, _, fixed, ..] => vec![1, 0, *fixed],
                    [1, ..] => {
                        let c = current_drop(&sel);
                        if c >= 0 { vec![0, 0, c] } else { Vec::new() }
                    }
                    _ => vec![1, 0],
                };
                set_selection(&mut entity.nbt, &id_str, next);
            } else if id >= 0 {
                let (drop, queue) = (id / 2, id % 2 == 1);
                if (drop as usize) >= m.fab.len() {
                    return;
                }
                let mut sel = selection(&entity.nbt, &id_str);
                if queue || sel.first() == Some(&1) {
                    if sel.first() != Some(&1) {
                        sel = vec![1, 0];
                    }
                    sel.push(drop);
                } else {
                    sel = vec![0, 0, drop];
                }
                set_selection(&mut entity.nbt, &id_str, sel);
            }
        }
        "hostilenetworks:data_center" => {
            if id == 1000 {
                let r = (get_i(&entity.nbt, "redstoneState", 0) + 1) % 3;
                set_i(&mut entity.nbt, "redstoneState", r);
            } else if (2000..2025).contains(&id) {
                let Some(m) = entity.items.get((id - 2000) as usize).and_then(|s| s.as_ref()).and_then(|s| model_of(game, s)) else { return };
                let id_str = m.id.clone();
                let c = current_drop(&selection(&entity.nbt, &id_str));
                let next = c + 1;
                set_selection(&mut entity.nbt, &id_str, if (next as usize) < m.fab.len() { vec![0, 0, next] } else { Vec::new() });
            }
        }
        _ => {}
    }
}

// ---- Tooltips

pub(in crate::game) fn tooltip(game: &Game<'_>, stack: &ItemStack, lines: &mut Vec<(horizon_client::text::Text, u32)>) {
    use horizon_client::text::Text;
    if !stack.item.starts_with("hostilenetworks:") {
        return;
    }
    let mut add = |line: String| lines.push((Text::legacy(&line), u32::MAX));
    match &*stack.item {
        "hostilenetworks:data_model" | "hostilenetworks:prediction" => {
            let (Some(m), Some(t)) = (model_of(game, stack), table(game)) else {
                add("§cError: §kBroke_AF".to_owned());
                return;
            };
            add(format!("{}{}", legacy_code(name_rgb(&m.color)), model_name(m)));
            if &*stack.item == "hostilenetworks:prediction" {
                return;
            }
            let d = data_of(stack);
            let tier = tier_of(t, m, d);
            add(lang("hostilenetworks.info.tier", "Tier: %s").replace("%s", &format!("{}{}", legacy_color(&tier.color), tier.name)));
            if !is_max(t, tier) {
                let (a, b) = (required(m, tier), required(m, next_tier(t, tier)));
                add(lang("hostilenetworks.info.data", "Data Collected: %s").replace("%s", &format!("§7{}/{}", d - a, b - a)));
                let key = if m.is_block { "hostilenetworks.info.dpm" } else { "hostilenetworks.info.dpk" };
                add(lang(key, "Data Per Kill: %s").replace("%s", &format!("§7{}", gained(m, tier))));
            }
            add(lang("hostilenetworks.info.sim_cost", "Simulation Cost: %s").replace("%s", &format!("§c{} FE/t", m.sim_cost)));
            if !m.variants.is_empty() {
                add(lang("hostilenetworks.info.subtypes", "Variants:"));
                for v in &m.variants {
                    let (ns, path) = v.split_once(':').unwrap_or(("minecraft", v));
                    add(format!("§7  - {}", lang(&format!("entity.{ns}.{path}"), path)));
                }
            }
        }
        "hostilenetworks:deep_learner" => {
            for m in learner_models(stack).into_iter().flatten() {
                if let (Some(model), Some(t)) = (model_of(game, &m), table(game)) {
                    let d = data_of(&m);
                    let tier = tier_of(t, model, d);
                    add(format!("§7{} {}{} ({d})", model_name(model), legacy_color(&tier.color), tier.name));
                }
            }
        }
        name => {
            let key = name.replace(':', ".");
            for suffix in ["desc", "desc2"] {
                for kind in ["block", "item"] {
                    let full = format!("{kind}.{key}.{suffix}");
                    let line = lang(&full, "");
                    if !line.is_empty() {
                        add(format!("§7{line}"));
                    }
                }
            }
        }
    }
}

fn legacy_color(name: &str) -> &'static str {
    match name {
        "dark_gray" => "§8",
        "green" => "§a",
        "blue" => "§9",
        "light_purple" => "§d",
        "gold" => "§6",
        _ => "§f",
    }
}

// ---- The Deep Learner's HUD (DeepLearnerHudRenderer): the models of the learner in hand.

pub(in crate::game) fn hud(game: &Game<'_>, batch: &mut HudBatch) {
    let Some(held) = game.held() else { return };
    if &*held.item != "hostilenetworks:deep_learner" {
        return;
    }
    let Some(t) = table(game) else { return };
    let mut y = 6.0;
    for m in learner_models(held).into_iter().flatten() {
        let Some(model) = model_of(game, &m) else { continue };
        let d = data_of(&m);
        let tier = tier_of(t, model, d);
        let (a, b) = (required(model, tier), required(model, next_tier(t, tier)));
        let part = if is_max(t, tier) { 1.0 } else { (d - a) as f32 / (b - a).max(1) as f32 };
        game.native_item(batch, &model_icon(model), [6.0, y], 16.0);
        let name = model_name(model);
        game.native_text(batch, &name, [26.0, y], name_rgb(&model.color), true);
        game.native_text(batch, &format!(" ({})", tier.name), [26.0 + game.native_text_width(&name), y], 0xFFFFFF, true);
        game.native_fill(batch, [26.0, y + 10.0, 89.0, 7.0], 0xFF202020);
        game.native_fill(batch, [26.0, y + 10.0, 89.0 * part.clamp(0.0, 1.0), 7.0], 0xFF00CCFF);
        y += 24.0;
    }
}

fn model_icon(m: &Model) -> String {
    if m.is_block {
        m.source.clone()
    } else {
        m.source.replace("minecraft:", "minecraft:") + "_spawn_egg"
    }
}

// ---- JEI and the guide

pub(in crate::game) fn kit_frame(game: &mut Game<'_>) {
    if !game.native_on("hostilenetworks") {
        return;
    }
    learner_frame(game);
    kit_guide::register("hostilenetworks", "Hostile Neural Networks", "Hostile Neural Networks guide");
    register_jade();
    hostilenetworks_optional::if_mod!("jei", {
        kit_layer::register(kit_layer::Provider::new("hostilenetworks"));
    } else { () });
}

#[cfg(test)]
mod stack_limit_tests {
    use super::slot_room;

    /// The IO port's loop over its nine slots, by counts: what's left after `n` more of an item stacking to `limit`.
    fn push(slots: &mut [u32; 9], limit: u32, mut n: u32) -> u32 {
        for have in slots.iter_mut() {
            let room = slot_room(limit, *have, n);
            *have += room;
            n -= room;
        }
        n
    }

    #[test]
    fn consecutive_enderman_outputs_keep_pearls_at_16() {
        let mut port = [0; 9];
        assert_eq!(push(&mut port, 16, 16), 0);
        assert_eq!(push(&mut port, 16, 16), 0);
        assert_eq!(&port[..3], &[16, 16, 0]);
        for _ in 0..7 {
            push(&mut port, 16, 16);
        }
        assert!(port.iter().all(|&c| c == 16));
        // A full port leaves the batch in the controller.
        assert_eq!(push(&mut port, 16, 16), 16);
        assert!(port.iter().all(|&c| c <= 16));
    }

    #[test]
    fn oversized_drops_are_split_into_an_empty_slot() {
        // A configured drop of 40 pearls: an empty slot takes 16, never 40.
        assert_eq!(slot_room(16, 0, 40), 16);
        let mut port = [0; 9];
        assert_eq!(push(&mut port, 16, 40), 0);
        assert_eq!(&port[..3], &[16, 16, 8]);
        // An unstackable item, one per slot.
        let mut port = [0; 9];
        assert_eq!(push(&mut port, 1, 12), 3);
        assert!(port.iter().all(|&c| c == 1));
    }

    #[test]
    fn partly_filled_and_normal_stacks() {
        assert_eq!(slot_room(16, 12, 16), 4);
        assert_eq!(slot_room(16, 16, 16), 0);
        assert_eq!(slot_room(64, 32, 16), 16);
        assert_eq!(slot_room(64, 60, 16), 4);
        // A count already past the limit never underflows.
        assert_eq!(slot_room(16, 32, 16), 0);
    }
}
