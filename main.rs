use std::{fs::read_to_string, fs::write, process::Command};

const LOW: usize = 4;
const HIGH: usize = 10;
const API: &str = "https://poe.ninja/api/data/ItemOverview";
const LEAGUE: [&str; 4] = ["Standard", "Hardcore", "SSF", "NoParties"];
const TEMPLATE: &str = "Z:/git/llkp/llkp.filter";
const FILTER: &str = "./asdf.txt";
const FADE: &str = "DisableDropSound
    SetFontSize 32
    SetBackgroundColor 0 0 0 125";

fn main() {
    let mut filter = read_to_string(TEMPLATE).unwrap();

    let league = league();
    println!("{league} league");

    let url = |s: &str| format!("{API}?league={league}&type={s}");

    let u = url("BaseType");
    println!("- Fetching from {u}");
    let (low, mid, high) = base_types::filter(&example("base_types"));
    filter = filter.replace("&base_types_low.", &low);
    filter = filter.replace("&base_types_mid.", &mid);
    filter = filter.replace("&base_types_high.", &high);

    let u = url("DivinationCard");
    println!("- Fetching from {u}");
    let (low, mid, high) = div_cards::filter(&example("div_cards"));
    filter = filter.replace("&div_cards_low.", &low);
    filter = filter.replace("&div_cards_mid.", &mid);
    filter = filter.replace("&div_cards_high.", &high);

    let u1 = url("UniqueWeapon");
    let u2 = url("UniqueArmor");
    println!("- Fetching from {u1}");
    println!("- Fetching from {u2}");
    let (low, mid, high) = uniques::filter(
        &[example("unique_armor"), example("unique_weapons")].concat(),
    );
    filter = filter.replace("&uniques_low.", &low);
    filter = filter.replace("&uniques_mid.", &mid);
    filter = filter.replace("&uniques_high.", &high);

    filter = filter.replace("&fade.", FADE);
    write(FILTER, filter).unwrap();
    println!("Filter output to {FILTER}");
}

fn example(s: &str) -> String {
    read_to_string(format!("./{s}.txt")).unwrap()
}

fn curl(url: &str) -> String {
    let curl = format!("curl --compressed '{url}'");
    let cmd = Command::new("bash").arg("-c").arg(curl).output().unwrap();
    String::from_utf8(cmd.stdout).unwrap()
}

fn league() -> String {
    curl("https://www.pathofexile.com/api/leagues")
        .split(&['{', ','])
        .map(|s| {
            s.chars()
                .filter(|&ch| ch.is_alphabetic() || ch == ':')
                .collect::<String>()
        })
        .flat_map(|s| match s.split_once(':') {
            Some(("id", b)) => Some(b.to_string()),
            _ => None,
        })
        .find(|league| !LEAGUE.iter().any(|x| league.contains(x)))
        .unwrap()
}

fn unquote(s: &str) -> String {
    s.replace('"', "")
}

pub mod uniques {
    use crate::*;
    use std::collections::{HashMap, HashSet};

    type T = HashSet<String>;

    const JSON_KEYS: [&str; 5] =
        ["name", "itemType", "baseType", "chaosValue", "links"];

    #[derive(Debug)]
    struct Unique {
        base_type: String,
        replica: bool,
        links: usize,
        price: usize,
    }

    pub fn filter(json: &str) -> (String, String, String) {
        let (low, mid, high) = low_mid_high(json);
        println!(
            "--- Uniques: {} low, {} mid, {} high",
            low.len(),
            mid.len(),
            high.len()
        );

        let string = |set: HashSet<String>| {
            set.into_iter().collect::<Vec<_>>().join(" ")
        };

        (string(low), string(mid), string(high))
    }

    fn low_mid_high(json: &str) -> (T, T, T) {
        let (mut low, mut mid, mut high) =
            (HashSet::new(), HashSet::new(), HashSet::new());
        for unique in json
            .split(r#"},{"id":"#)
            .map(parse)
            .filter(|u| !u.replica && u.links == 0)
        {
            if unique.price < LOW {
                mid.remove(&unique.base_type);
                if high.remove(&unique.base_type) {
                    mid.insert(unique.base_type);
                } else {
                    low.insert(unique.base_type);
                };
            } else if unique.price < HIGH {
                high.remove(&unique.base_type);
                mid.insert(unique.base_type);
            } else if !low.contains(&unique.base_type)
                && !mid.contains(&unique.base_type)
            {
                high.insert(unique.base_type);
            }
        }
        (low, mid, high)
    }

    fn parse(json_obj: &str) -> Unique {
        let map = HashMap::<String, String>::from_iter(
            json_obj
                .split(',')
                .filter_map(|s| s.split_once(':'))
                .map(|(k, v)| (unquote(k), v.to_string()))
                .filter(|(k, _)| JSON_KEYS.contains(&k.as_str())),
        );

        let string = |k: &str| map.get(k).unwrap().to_string();
        let int = |k: &str| {
            map.get(k)
                .and_then(|s| s.parse::<f32>().ok())
                .map(|x| x.floor() as usize)
        };

        Unique {
            base_type: string("baseType"),
            replica: string("name").contains("Replica"),
            links: int("links").unwrap_or(0),
            price: int("chaosValue").unwrap(),
        }
    }
}

pub mod div_cards {
    use crate::*;

    type T = Vec<String>;

    pub fn filter(json: &str) -> (String, String, String) {
        let (low, mid, high) = low_mid_high(json);
        println!(
            "--- Div cards: {} low, {} mid, {} high",
            low.len(),
            mid.len(),
            high.len()
        );

        (low.join(" "), mid.join(" "), high.join(" "))
    }

    fn low_mid_high(json: &str) -> (T, T, T) {
        json.split("},{").map(parse).fold(
            (Vec::new(), Vec::new(), Vec::new()),
            |(mut low, mut mid, mut high), (k, v)| {
                if v < LOW {
                    low.push(k)
                } else if v < HIGH {
                    mid.push(k)
                } else {
                    high.push(k);
                };
                (low, mid, high)
            },
        )
    }

    fn parse(json_obj: &str) -> (String, usize) {
        let (mut name, mut price) = (None, None);
        for (k, v) in json_obj.split(',').filter_map(|s| s.split_once(':')) {
            let k = unquote(k);
            if k == "name" {
                name = Some(v.to_string());
            } else if k == "chaosValue" {
                price = Some(v.parse::<f32>().unwrap().floor() as usize)
            }
        }

        match (name, price) {
            (Some(a), Some(b)) => (a, b),
            _ => unreachable!(),
        }
    }
}

pub mod base_types {
    use crate::*;
    use std::collections::HashMap;

    type K = (usize, String, String); // ilvl, class, influence
    type V = Vec<String>; // base types
    type T = HashMap<K, V>;

    const INFLUENCE: [&str; 7] = [
        "None", "Elder", "Shaper", "Crusader", "Hunter", "Redeemer", "Warlord",
    ];
    const JSON_KEYS: [&str; 7] = [
        "itemType",
        "baseType",
        "variant",
        "levelRequired",
        "chaosValue",
        "count",
        "listingCount",
    ];

    #[derive(Debug)]
    struct BaseType {
        class: String,
        name: String,
        influence: String,
        ilvl: usize,
        price: usize,
        count: usize,
        listing_count: usize,
    }

    pub fn filter(json: &str) -> (String, String, String) {
        let (low, mid, high) = low_mid_high(json);
        println!(
            "--- Base types: {} low, {} mid, {} high",
            low.len(),
            mid.len(),
            high.len()
        );

        let filter = |map: HashMap<K, V>, tier| {
            let mut rules = Vec::new();
            for ((ilvl, class, influence), base_types) in map {
                let base_types = base_types.join(" ");
                let mut template = vec![
                    format!("ItemLevel {ilvl}"),
                    format!("Class {class}"),
                    format!("HasInfluence {influence}"),
                    format!("BaseType {base_types}"),
                ];
                let mut s = Vec::new();
                match tier {
                    "low" => s.push("Hide".to_string()),
                    _ => s.push("Show".to_string()),
                }
                s.append(&mut template);
                match tier {
                    "low" => s.push("&fade.".to_string()),
                    "high" => s.push("MinimapIcon 0 Blue Star".to_string()),
                    _ => (),
                }
                rules.push(s.join("\r\n    "));
            }
            rules.join("\r\n")
        };

        (filter(low, "low"), filter(mid, "mid"), filter(high, "high"))
    }

    fn low_mid_high(json: &str) -> (T, T, T) {
        json.split("},{")
            .map(parse)
            .filter(|bt| INFLUENCE.contains(&bt.influence.as_str()))
            .filter(|bt| bt.count >= 10 || bt.listing_count >= 10)
            .fold(
                (HashMap::new(), HashMap::new(), HashMap::new()),
                |(mut low, mut mid, mut high), bt| {
                    let class = format!(" {}", unquote(&bt.class));
                    let k = (bt.ilvl, bt.class, bt.influence);
                    let v = bt.name.replace(&class, "").trim().to_string();
                    if bt.price < LOW {
                        low.entry(k).or_insert_with(Vec::new).push(v);
                    } else if bt.price < HIGH {
                        mid.entry(k).or_insert_with(Vec::new).push(v);
                    } else {
                        high.entry(k).or_insert_with(Vec::new).push(v);
                    }
                    (low, mid, high)
                },
            )
    }

    fn parse(json_obj: &str) -> BaseType {
        let clean =
            |s: &str| s.replace("One Handed ", "").replace("Two Handed ", "");

        let map = HashMap::<String, String>::from_iter(
            json_obj
                .split(',')
                .filter_map(|s| s.split_once(':'))
                .map(|(k, v)| (unquote(k), clean(v)))
                .filter(|(k, _)| JSON_KEYS.contains(&k.as_str())),
        );

        let string = |s: &str| map.get(s).map(ToString::to_string);
        let int = |s: &str| {
            map.get(s)
                .and_then(|s| s.parse::<f32>().ok())
                .map(|x| x.floor() as usize)
        };

        BaseType {
            class: string("itemType").unwrap(),
            name: string("baseType").unwrap(),
            influence: string("variant").unwrap_or_else(|| "None".to_string()),
            ilvl: int("levelRequired").unwrap(),
            price: int("chaosValue").unwrap(),
            count: int("count").unwrap_or(0),
            listing_count: int("listingCount").unwrap_or(0),
        }
    }
}
