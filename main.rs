use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::OnceLock,
};

type BaseTypes = Vec<BaseType>;
type Filter = String;
type Float = f32;
type Json = String;
type Todo = HashMap<String, HashSet<String>>;

const CUTOFF: Float = 20.0;
const MIN_COUNT: usize = 20;

static LEAGUE: OnceLock<String> = OnceLock::new();
static DEBUG: OnceLock<bool> = OnceLock::new();

const GGG: &str = "https://www.pathofexile.com/api/leagues";
const NINJA: &str = "https://poe.ninja/api/data/ItemOverview";
const EXCLUDE: [&str; 6] =
    ["Standard", "Hardcore", "Solo", "Ruthless", "SSF", "HC"];

const LLKP_FILTER: &str = "LLKP.filter";
const BASE_TYPES_FILTER: &str = "base_types.filter";
const CLUSTER_JEWELS_FILTER: &str = "cluster_jewels.filter";

const BASE_TYPES: [&str; 2] = ["BaseType", "./base_types.json"];
const DIV_CARDS: [&str; 2] = ["DivinationCard", "./div_cards.json"];
const UNIQUE_WEAPONS: [&str; 2] = ["UniqueWeapon", "./unique_weapons.json"];
const UNIQUE_ARMOR: [&str; 2] = ["UniqueArmour", "./unique_armor.json"];
const CLUSTER_JEWELS: [&str; 2] = ["ClusterJewel", "./cluster_jewels.json"];

#[derive(Deserialize, Debug)]
struct League {
    id: String,
}

#[derive(Deserialize, Debug)]
struct BaseType {
    #[serde(rename = "baseType")]
    base_type: String,
    #[serde(rename = "levelRequired")]
    ilvl: Option<u8>,
    #[serde(rename = "chaosValue")]
    price: Float,
    name: String,
    variant: Option<String>,
    links: Option<u8>,
    count: usize,
}

fn main() {
    // debug/local mode
    let _ = DEBUG.set(std::env::args().any(|x| x == "debug" || x == "local"));

    // challenge league
    let leagues = serde_json::from_str::<Vec<League>>(&get(GGG))
        .unwrap()
        .into_iter()
        .map(|league| league.id)
        .filter(|league| !EXCLUDE.iter().any(|x| league.contains(x)))
        .collect::<Vec<_>>();
    let _ = LEAGUE.set(leagues[0].to_owned());
    println!("... leagues = [{}]", leagues.join(", "));

    // base types
    let (high_value, _, _) = split(BASE_TYPES);
    let mut todo = Todo::new();
    for item in high_value {
        let ilvl = item.ilvl.unwrap();
        let influence = if item.variant.is_none() {
            "no_influence".to_string()
        } else if item.variant.as_ref().unwrap().contains('/') {
            continue;
        } else {
            item.variant.unwrap().to_lowercase()
        };
        todo.entry(format!("&ilvl_{ilvl}_{influence}."))
            .or_default()
            .insert(item.base_type);
    }
    find_and_replace(local(BASE_TYPES_FILTER), remote(BASE_TYPES_FILTER), todo);
    leftovers(BASE_TYPES_FILTER, "Divine Orb");

    // cluster jewels
    let (high_value, _, _) = split(CLUSTER_JEWELS);
    let cluster_jewel_names = cluster_jewel_names();
    let mut todo = Todo::new();
    for item in high_value {
        let base = item.base_type.split(' ').next().unwrap().to_lowercase();
        let ilvl = item.ilvl.unwrap();
        let n = item.variant.unwrap().split(' ').next().unwrap().to_owned();
        let enchant = cluster_jewel_names.get(&item.name).unwrap().to_owned();
        todo.entry(format!("&{base}_ilvl_{ilvl}_passive_{n}."))
            .or_default()
            .insert(enchant);
    }
    find_and_replace(
        local(CLUSTER_JEWELS_FILTER),
        remote(CLUSTER_JEWELS_FILTER),
        todo,
    );
    leftovers(CLUSTER_JEWELS_FILTER, "Reservation Efficiency");

    // main filter
    let mut todo = Todo::new();

    // div cards
    let (high_value, low_value, low_confidence) = split(DIV_CARDS);
    let f = |v: BaseTypes| v.into_iter().map(|x| x.base_type);
    todo.insert(
        "&div_cards_show.".to_string(),
        f(high_value).chain(f(low_confidence)).collect(),
    );
    todo.insert("&div_cards_hide.".to_string(), f(low_value).collect());

    // unique armor
    let (high_value_a, low_value_a, low_confidence_a) = split(UNIQUE_ARMOR);
    let (high_value_w, low_value_w, low_confidence_w) = split(UNIQUE_WEAPONS);
    for item in high_value_a
        .into_iter()
        .chain(low_confidence_a.into_iter())
        .chain(high_value_w.into_iter())
        .chain(low_confidence_w.into_iter())
        .filter(|x| !x.name.contains("Replica"))
    {
        let k = match item.links {
            Some(6) => continue,
            Some(5) => "&uniques_links_eq_5.",
            _ => "&uniques_links_le_4.",
        };
        todo.entry(k.to_string())
            .or_default()
            .insert(item.base_type);
    }
    for item in low_value_a.into_iter().chain(low_value_w.into_iter()) {
        todo.entry("&uniques_low_value.".to_string())
            .or_default()
            .insert(item.base_type.to_string());
    }

    find_and_replace(local(LLKP_FILTER), remote(LLKP_FILTER), todo);
}

fn local(file_name: &str) -> Filter {
    let mut filter = std::fs::read_to_string(format!("./{file_name}")).unwrap();
    let style = [
        (
            "\n    &chaos_recipe.",
            "\n    Rarity Rare
    Identified False
    ItemLevel >= 60
    ItemLevel < 75
    &fade.",
        ),
        (
            "\n    &fade.",
            "\n    DisableDropSound
    SetFontSize 32
    SetBackgroundColor 0 0 0 125
    MinimapIcon -1",
        ),
        ("\n    &minimap.", "\n    MinimapIcon 0 Blue Star"),
    ];
    for (k, v) in style {
        filter = filter.replace(k, v);
    }
    filter
}

fn remote(file_name: &str) -> PathBuf {
    let root = std::env::var("llkp_path").unwrap();
    PathBuf::from(format!("{root}/{file_name}",))
}

fn get(url: &str) -> Json {
    println!("Calling {url} ...");
    minreq::get(url)
        .send()
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

fn split([api, local_file]: [&str; 2]) -> (BaseTypes, BaseTypes, BaseTypes) {
    let json = if let (true, Ok(json)) =
        (DEBUG.get().unwrap(), std::fs::read_to_string(local_file))
    {
        println!("Calling {local_file} ...");
        json
    } else {
        let league = LEAGUE.get().unwrap();
        let json = get(&format!("{NINJA}?league={league}&type={api}"));
        std::fs::write(local_file, &json).unwrap();
        json
    };
    let items = serde_json::from_str::<HashMap<String, BaseTypes>>(&json)
        .unwrap()
        .remove("lines")
        .unwrap();
    println!("... found {} items", items.len());
    let (mut high_value, mut low_value, mut low_confidence) =
        (Vec::new(), Vec::new(), Vec::new());
    for item in items.into_iter() {
        if item.count < MIN_COUNT {
            low_confidence.push(item);
        } else if item.price >= CUTOFF {
            high_value.push(item);
        } else {
            low_value.push(item);
        }
    }
    println!("... found {} items >= {CUTOFF} chaos", high_value.len());
    println!("... found {} items < {CUTOFF} chaos", low_value.len());
    println!(
        "... found {} items with count < {MIN_COUNT}",
        low_confidence.len()
    );
    (high_value, low_value, low_confidence)
}

fn find_and_replace(mut filter: Filter, dest: PathBuf, mut todo: Todo) {
    for (k, v) in todo.drain() {
        let items = v
            .into_iter()
            .map(|x| format!("\"{x}\""))
            .collect::<Vec<_>>()
            .join(" ");
        filter = filter.replace(&k, &items);
    }
    std::fs::write(&dest, filter).unwrap();
    println!("... writing to {} ...", dest.to_str().unwrap());
}

fn leftovers(file_name: &str, default: &str) {
    println!("... filling in leftovers");
    let path = remote(file_name);
    let filter = std::fs::read_to_string(&path).unwrap();
    let mut todo = Todo::new();
    for leftover in filter
        .split_whitespace()
        .filter(|w| w.starts_with('&') && w.ends_with('.'))
        .collect::<HashSet<_>>()
    {
        todo.insert(leftover.to_string(), HashSet::from([default.to_string()]));
    }
    find_and_replace(filter, path, todo);
}

fn cluster_jewel_names() -> HashMap<String, String> {
    let ninja_names = [
        "10% increased Area Damage",
        "15% increased Armour",
        "10% increased Attack Damage",
        "12% increased Attack Damage while Dual Wielding",
        "12% increased Attack Damage while holding a Shield",
        "Axe Attacks deal 12% increased Damage with Hits and Ailments, Sword Attacks deal 12% increased Damage with Hits and Ailments",
        "+1% Chance to Block Attack Damage",
        "1% Chance to Block Spell Damage",
        "12% increased Damage with Bows, 12% increased Damage Over Time with Bow Skills",
        "12% increased Brand Damage",
        "Channelling Skills deal 12% increased Damage",
        "12% increased Chaos Damage",
        "12% increased Chaos Damage over Time",
        "+12% to Chaos Resistance",
        "12% increased Cold Damage",
        "12% increased Cold Damage over Time",
        "+15% to Cold Resistance",
        "15% increased Critical Strike Chance",
        "2% increased Effect of your Curses",
        "Claw Attacks deal 12% increased Damage with Hits and Ailments, Dagger Attacks deal 12% increased Damage with Hits and Ailments",
        "10% increased Damage over Time",
        "10% increased Damage while affected by a Herald",
        "12% increased Damage with Two Handed Weapons",
        "10% increased Effect of Non-Damaging Ailments",
        "10% increased Elemental Damage",
        "6% increased maximum Energy Shield",
        "15% increased Evasion Rating",
        "Exerted Attacks deal 20% increased Damage",
        "12% increased Fire Damage",
        "12% increased Burning Damage",
        "+15% to Fire Resistance",
        "6% increased Flask Effect Duration",
        "4% increased maximum Life",
        "12% increased Lightning Damage",
        "+15% to Lightning Resistance",
        "Staff Attacks deal 12% increased Damage with Hits and Ailments, Mace or Sceptre Attacks deal 12% increased Damage with Hits and Ailments",
        "6% increased maximum Mana",
        "Minions deal 10% increased Damage",
        "Minions deal 10% increased Damage while you are affected by a Herald",
        "Minions have 12% increased maximum Life",
        "12% increased Physical Damage",
        "12% increased Physical Damage over Time",
        "10% increased Projectile Damage",
        "10% increased Life Recovery from Flasks, 10% increased Mana Recovery from Flasks",
        "6% increased Mana Reservation Efficiency of Skills",
        "10% increased Spell Damage",
        "+2% chance to Suppress Spell Damage",
        "12% increased Totem Damage",
        "12% increased Trap Damage, 12% increased Mine Damage",
        "Wand Attacks deal 12% increased Damage with Hits and Ailments"
    ];
    let poe_names = [
        "Area Damage",
        "Armour",
        "Attack Damage",
        "Attack Damage while Dual Wielding",
        "Attack Damage while holding a Shield",
        "Axe and Sword Damage",
        "Block Attack Damage",
        "Block Spell Damage",
        "Bow Damage",
        "Brand Damage",
        "Channelling Skill Damage",
        "Chaos Damage",
        "Chaos Damage over Time",
        "Chaos Resistance",
        "Cold Damage",
        "Cold Damage over Time",
        "Cold Resistance",
        "Critical Chance",
        "Curse Effect",
        "Dagger and Claw Damage",
        "Damage over Time",
        "Damage while you have a Herald",
        "Damage with Two Handed Weapons",
        "Effect of Non-Damaging Ailments",
        "Elemental Damage",
        "Energy Shield",
        "Evasion",
        "Exerted Attack Damage",
        "Fire Damage",
        "Fire Damage over Time",
        "Fire Resistance",
        "Flask Duration",
        "Life",
        "Lightning Damage",
        "Lightning Resistance",
        "Mace and Staff Damage",
        "Mana",
        "Minion Damage",
        "Minion Damage while you have a Herald",
        "Minion Life",
        "Physical Damage",
        "Physical Damage over Time",
        "Projectile Damage",
        "Recovery from Flasks",
        "Reservation Efficiency",
        "Spell Damage",
        "Suppress Spell Damage",
        "Totem Damage",
        "Trap and Mine Damage",
        "Wand Damage",
    ];
    ninja_names
        .into_iter()
        .zip(poe_names)
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect::<HashMap<_, _>>()
}
