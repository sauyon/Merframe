//! Looks into the game's Lua bindings by name, to find native state worth reading.
//!
//! ```text
//! swig class <Class> [words]   the class's methods, named from a word list where they match
//! swig instances <Class>       live objects Lua holds of the class
//! swig timers [seconds]        the HUD's ability timers, printed as they change
//! ```
//!
//! Method names are only kept as hashes, so `words` (one candidate name per line, e.g. the
//! identifiers `ptr_scan words` collects) is how they get their names back; names are also
//! tried with `Get`, `Set`, `Is` and `Has` in front.

use std::collections::HashMap;
use std::error::Error;
use std::time::{Duration, Instant};

use wf_mem::open_game;
use wf_scan::AbilityTimers;
use wf_scan::swig::{Image, member_hash};

const PREFIXES: [&str; 5] = ["", "Get", "Set", "Is", "Has"];

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let reader = open_game()?;
    match args.first().map(String::as_str) {
        Some("class") => {
            let name = args.get(1).ok_or("usage: swig class <Class> [words]")?;
            let image = Image::read(&*reader)?;
            let class = image.class(name).ok_or("no such class")?;
            println!("{name}: type info {:#x}", class.type_info);
            let mut names = HashMap::new();
            if let Some(file) = args.get(2) {
                for word in std::fs::read_to_string(file)?.lines() {
                    for prefix in PREFIXES {
                        let candidate = format!("{prefix}{word}");
                        names.entry(member_hash(&candidate)).or_insert(candidate);
                    }
                }
            }
            for (hash, wrapper) in class.methods(&image) {
                let body = image
                    .callee(wrapper)
                    .map_or_else(String::new, |at| format!("{at:#x}"));
                let known = names.get(&hash).map_or("?", String::as_str);
                println!("  {hash:08x} {known:40} binding {wrapper:#x} body {body}");
            }
        }
        Some("instances") => {
            let name = args.get(1).ok_or("usage: swig instances <Class>")?;
            let image = Image::read(&*reader)?;
            let class = image.class(name).ok_or("no such class")?;
            for (handle, object) in class.instances(&*reader)? {
                println!("{object:#x} (handle {handle:#x})");
            }
        }
        Some("timers") => {
            let seconds: u64 = args.get(1).map_or(Ok(60), |s| s.parse())?;
            let timers = AbilityTimers::locate(&*reader)?.ok_or("no HUD with ability timers")?;
            println!("{timers:x?}");
            let start = Instant::now();
            let mut last = None;
            while start.elapsed().as_secs() < seconds {
                let now = timers.read(&*reader);
                if now != last {
                    println!("{:6.1}s {now:x?}", start.elapsed().as_secs_f32());
                    last = now;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
        }
        _ => return Err("usage: swig <class|instances|timers> ...".into()),
    }
    Ok(())
}
