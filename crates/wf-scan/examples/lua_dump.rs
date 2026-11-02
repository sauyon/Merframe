//! Dumps every loaded Lua script's fields (and the tables they close over) to stdout,
//! to find where the game keeps state worth reading.
//!
//! ```text
//! cargo run -p wf-scan --example lua_dump -- [depth]
//! ```

use std::collections::HashSet;
use std::error::Error;

use wf_mem::{MemoryReader, open_game};
use wf_scan::LuaState;
use wf_scan::lua::{LuaTable, LuaValue};

fn main() -> Result<(), Box<dyn Error>> {
    let depth: usize = std::env::args().nth(1).map_or(Ok(2), |d| d.parse())?;
    let reader = open_game()?;
    let state = LuaState::locate(&*reader)?.ok_or("no Lua state found")?;
    let scripts = state.scripts(&*reader);
    println!("{} scripts", scripts.len());
    let mut seen = HashSet::new();
    for (i, script) in scripts.into_iter().enumerate() {
        println!("== script {i}");
        dump(&*reader, script, 1, depth, &mut seen);
        for local in script.locals(&*reader) {
            println!("  [local]");
            dump(&*reader, local, 2, depth, &mut seen);
        }
    }
    Ok(())
}

fn dump(reader: &dyn MemoryReader, table: LuaTable, indent: usize, depth: usize, seen: &mut HashSet<LuaTable>) {
    if !seen.insert(table) {
        return;
    }
    let pad = "  ".repeat(indent);
    let mut entries = table.entries(reader);
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    for (key, value) in entries {
        match value {
            LuaValue::Table(t) => {
                println!("{pad}{key}: table[{}]", t.items(reader).len());
                if indent < depth {
                    dump(reader, t, indent + 1, depth, seen);
                }
            }
            LuaValue::Function(_) => {}
            LuaValue::Text(s) => println!("{pad}{key} = {s:?}"),
            LuaValue::Number(n) => println!("{pad}{key} = {n}"),
            LuaValue::Boolean(b) => println!("{pad}{key} = {b}"),
            other => println!("{pad}{key} = {other:?}"),
        }
    }
}
