//! Prints the local player's HUD buff tray once a second.
//!
//! ```text
//! cargo run -p wf-scan --example buffs -- [seconds]
//! ```

use std::error::Error;
use std::time::Duration;

use wf_mem::open_game;
use wf_scan::{LuaState, status_effects, status_list};

fn main() -> Result<(), Box<dyn Error>> {
    let seconds: u64 = std::env::args().nth(1).map_or(Ok(5), |s| s.parse())?;
    let reader = open_game()?;
    let state = LuaState::locate(&*reader)?.ok_or("no Lua state found")?;
    for _ in 0..seconds {
        match status_list(&*reader, &state) {
            None => println!("buff tray not loaded"),
            Some(list) => {
                for e in status_effects(&*reader, list) {
                    println!(
                        "{:>3} type {:>2} data {:>8.2} pending {:>8} initial {:>7.2} {}{}{}",
                        e.id,
                        e.buff_type,
                        e.data,
                        e.pending_data.map_or("-".into(), |p| format!("{p:.2}")),
                        e.initial_data,
                        e.name_tag,
                        if e.is_debuff { " DEBUFF" } else { "" },
                        if e.paused { " PAUSED" } else { "" },
                    );
                }
            }
        }
        println!("--");
        std::thread::sleep(Duration::from_secs(1));
    }
    Ok(())
}
