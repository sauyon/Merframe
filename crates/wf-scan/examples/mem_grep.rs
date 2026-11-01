//! Prints context around every occurrence of a byte string in the game's writable memory.
//!
//! ```text
//! cargo run -p wf-scan --example mem_grep -- <pattern> [context] [max]
//! ```

use std::collections::HashSet;
use std::error::Error;

use memchr::memmem;
use wf_mem::open_game;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let pattern = args.next().ok_or("usage: mem_grep <pattern> [context] [max]")?;
    let context: usize = args.next().map_or(Ok(200), |s| s.parse())?;
    let max: usize = args.next().map_or(Ok(20), |s| s.parse())?;
    let reader = open_game()?;
    let finder = memmem::Finder::new(pattern.as_bytes());
    let mut shown = 0;
    let mut seen = HashSet::new();
    for region in reader.regions()? {
        if !(region.readable && region.writable) {
            continue;
        }
        let Ok(data) = reader.read_vec(region.range.start, usize::try_from(region.size())?) else {
            continue;
        };
        for at in finder.find_iter(&data) {
            let lo = at.saturating_sub(context);
            let hi = (at + pattern.len() + context).min(data.len());
            let text: String = data[lo..hi]
                .iter()
                .map(|&b| if b.is_ascii_graphic() || b == b' ' { char::from(b) } else { '.' })
                .collect();
            if !seen.insert(text.clone()) {
                continue;
            }
            println!("@{:#x}\n{text}\n", region.range.start + at as u64);
            shown += 1;
            if shown >= max {
                return Ok(());
            }
        }
    }
    Ok(())
}
