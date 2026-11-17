//! Prints the seconds ability scripts have the HUD write under their icons through
//! `LotusHudRedux:SetAbilityTimer`, like Grasp of Lohk's, whenever they change.
//!
//! ```text
//! cargo run -p wf-scan --example ability_labels -- [seconds]
//! ```

use std::error::Error;
use std::time::{Duration, Instant};

use wf_mem::open_game;
use wf_scan::AbilityTimers;

fn main() -> Result<(), Box<dyn Error>> {
    let seconds: u64 = std::env::args().nth(1).map_or(Ok(60), |s| s.parse())?;
    let reader = open_game()?;
    let timers = AbilityTimers::locate(&*reader)?.ok_or("no HUD with ability timers")?;
    println!("{timers:x?}");
    println!("abilities {:?}", timers.abilities(&*reader));
    let start = Instant::now();
    let mut last = None;
    while start.elapsed().as_secs() < seconds {
        let now = timers.labels(&*reader);
        if now != last {
            println!("{:6.1}s {now:?}", start.elapsed().as_secs_f32());
            last = now;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Ok(())
}
