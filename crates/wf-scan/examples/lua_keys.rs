//! Walks every Lua table reachable from the loaded scripts (through fields, array items
//! and closure upvalues) and prints keys matching a pattern, with their values.
//!
//! ```text
//! lua_keys <case-insensitive substring>...
//! ```

use std::collections::{HashSet, VecDeque};
use std::error::Error;

use wf_mem::open_game;
use wf_scan::LuaState;
use wf_scan::lua::{LuaTable, LuaValue};

const MAX_TABLES: usize = 2_000_000;

fn main() -> Result<(), Box<dyn Error>> {
    let (prefixes, needles): (Vec<String>, Vec<String>) = std::env::args().skip(1).partition(|a| a.starts_with('@'));
    let prefixes: Vec<String> = prefixes.into_iter().map(|p| p[1..].to_owned()).collect();
    let needles: Vec<String> = needles.into_iter().map(|s| s.to_lowercase()).collect();
    let reader = open_game()?;
    let state = LuaState::locate(&*reader)?.ok_or("no Lua state found")?;

    let mut queue: VecDeque<(LuaTable, String)> = state
        .scripts(&*reader)
        .into_iter()
        .enumerate()
        .map(|(i, t)| (t, format!("s{i}")))
        .collect();
    let mut seen: HashSet<LuaTable> = HashSet::new();
    let mut seen_fns = HashSet::new();
    let mut hits = 0;
    while let Some((table, path)) = queue.pop_front() {
        if !seen.insert(table) || seen.len() > MAX_TABLES {
            continue;
        }
        let mut push = |value: LuaValue, path: String, queue: &mut VecDeque<(LuaTable, String)>| match value {
            LuaValue::Table(t) => queue.push_back((t, path)),
            LuaValue::Function(f) if seen_fns.insert(f) => {
                for (j, up) in f.upvalues(&*reader).into_iter().enumerate() {
                    if let LuaValue::Table(t) = up {
                        queue.push_back((t, format!("{path}^{j}")));
                    }
                }
            }
            _ => {}
        };
        for (key, value) in table.entries(&*reader) {
            let lower = key.to_lowercase();
            let full = format!("{path}.{key}");
            let under = prefixes.iter().any(|p| full.contains(p.as_str()));
            if under || needles.iter().any(|n| lower.contains(n.as_str())) {
                match &value {
                    LuaValue::Table(t) => println!("{path}.{key}: table[{}]", t.items(&*reader).len()),
                    LuaValue::Function(_) => {}
                    other => println!("{path}.{key} = {other:?}"),
                }
                hits += 1;
            }
            let child = format!("{path}.{key}");
            push(value, child, &mut queue);
        }
        for (i, value) in table.items(&*reader).into_iter().enumerate() {
            push(value, format!("{path}[{}]", i + 1), &mut queue);
        }
    }
    println!("{} tables walked, {hits} hits", seen.len());
    Ok(())
}
