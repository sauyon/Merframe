//! Walks pointers back from an address to the game's executable image, Cheat Engine style:
//! finds every word in writable memory that points at or a little before the address, then
//! does the same for those words, until one sits in the image. Image addresses stay put
//! between missions, so a chain from one is a stable way back to the address.
//!
//! ```text
//! ptr_scan <addr> [depth] [reach] [keep]
//! ptr_scan tribute [depth] [reach] [keep]   start from every Tribute buff timer
//! ptr_scan meet <root> [depth] [reach] [keep] walk back from the Tribute timers and forward
//!                                             from <root> until the two walks meet
//! ```
//!
//! `reach` is how far before a target a pointer may land (the target's offset in the object
//! it points to, default 0x400); `keep` caps how many holders each level carries on.

use std::collections::HashMap;
use std::error::Error;

use wf_mem::{GAME_PROCESS, open_game};

type Res<T> = Result<T, Box<dyn Error>>;

const CHUNK: usize = 32 << 20;

fn hex(arg: Option<&String>, default: u64) -> Res<u64> {
    arg.map_or(Ok(default), |s| {
        Ok(u64::from_str_radix(s.trim_start_matches("0x"), 16)?)
    })
}

fn main() -> Res<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let reader = open_game()?;
    if args.first().map(String::as_str) == Some("follow") {
        // follow <root> <off>...: read a pointer at each offset but the last, then add it.
        let mut at = hex(args.get(1), 0)?;
        let offs: Vec<u64> = args[2..]
            .iter()
            .map(|o| hex(Some(o), 0))
            .collect::<Res<_>>()?;
        for (i, off) in offs.iter().enumerate() {
            at += off;
            if i + 1 < offs.len() {
                at = reader.read_u64(at)?;
            }
            println!("  +{off:#x} => {at:#x}");
        }
        let mut b = [0u8; 8];
        reader.read_exact(at, &mut b)?;
        let f = |i: usize| f32::from_le_bytes(b[i..i + 4].try_into().unwrap_or_default());
        println!("{at:#x}: {:.1} of {:.1}", f(0), f(4));
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("words") {
        // words <file>: every distinct NUL-terminated identifier in writable memory.
        let out = args.get(1).ok_or("usage: ptr_scan words <file>")?;
        let mut words = std::collections::BTreeSet::new();
        for region in reader
            .regions()?
            .into_iter()
            .filter(|r| r.readable && r.writable)
        {
            let Ok(data) = reader.read_vec(region.range.start, usize::try_from(region.size())?)
            else {
                continue;
            };
            let mut start = None;
            for (i, &b) in data.iter().enumerate() {
                let ident = b.is_ascii_alphanumeric() || b == b'_';
                match (start, ident) {
                    (None, true) => start = Some(i),
                    (Some(s), false) => {
                        if b == 0 && (3..=48).contains(&(i - s)) && !data[s].is_ascii_digit() {
                            words.insert(String::from_utf8_lossy(&data[s..i]).into_owned());
                        }
                        start = None;
                    }
                    _ => {}
                }
            }
        }
        std::fs::write(
            out,
            words.into_iter().collect::<Vec<_>>().join(
                "
",
            ),
        )?;
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("image") {
        // image <file>: the game's executable image as mapped, unreadable pages zeroed.
        let out = args.get(1).ok_or("usage: ptr_scan image <file>")?;
        let module = reader.module(GAME_PROCESS)?;
        let mut bytes = vec![0u8; usize::try_from(module.size)?];
        for (i, page) in bytes.chunks_mut(0x1000).enumerate() {
            let _ = reader.read(module.base + 0x1000 * i as u64, page);
        }
        std::fs::write(out, &bytes)?;
        println!("image base {:#x}, {} MB", module.base, bytes.len() >> 20);
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("reach") {
        // reach <root> <target> [depth] [span]: forward paths from <root> to a word that
        // points at or up to 0x40 before <target>.
        let root = hex(args.get(1), 0)?;
        let target = hex(args.get(2), 0)?;
        let depth: usize = args.get(3).map_or(Ok(3), |s| s.parse())?;
        let span = hex(args.get(4), 0x400)?;
        let mut seen = std::collections::HashSet::new();
        let mut frontier = vec![(root, String::new())];
        let mut shown = 0;
        for _ in 0..depth {
            let mut next = Vec::new();
            for (object, path) in frontier {
                let Ok(data) = reader.read_vec(object, usize::try_from(span)?) else {
                    continue;
                };
                for (i, w) in data.as_chunks::<8>().0.iter().enumerate() {
                    let ptr = u64::from_le_bytes(*w);
                    let here = format!("{path}[+{:#x}]", 8 * i);
                    if ptr <= target && target - ptr <= 0x40 {
                        println!("{here} -> {ptr:#x}");
                        shown += 1;
                        if shown >= 10 {
                            return Ok(());
                        }
                    }
                    if (0x10000..0x7ff0_0000_0000).contains(&ptr)
                        && ptr % 8 == 0
                        && seen.insert(ptr)
                        && next.len() < 300_000
                    {
                        next.push((ptr, here));
                    }
                }
            }
            frontier = next;
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("refs") {
        // refs <addr>: words anywhere in the image or the heap equal to <addr>.
        let want = hex(args.get(1), 0)?;
        for region in reader.regions()?.into_iter().filter(|r| r.readable) {
            let Ok(data) = reader.read_vec(region.range.start, usize::try_from(region.size())?)
            else {
                continue;
            };
            for (i, w) in data.as_chunks::<8>().0.iter().enumerate() {
                if u64::from_le_bytes(*w) == want {
                    println!("{:#x}", region.range.start + 8 * i as u64);
                }
            }
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("hudtimers") {
        // hudtimers <hud> <lists off> <secs>: poll the HUD's per-ability timer lists, each a
        // {ptr, bytes, cap} array of {key u64, seconds f32} entries, printing changes.
        let hud = hex(args.get(1), 0)?;
        let off = hex(args.get(2), 0x478)?;
        let secs: u64 = args.get(3).map_or(Ok(600), |s| s.parse())?;
        let mut last = String::new();
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < secs {
            let lists = reader.read_u64(hud + off)?;
            let n = (reader.read_u64(hud + off + 8)? as u32) / 16;
            let mut line = String::new();
            for i in 0..u64::from(n.min(16)) {
                let data = reader.read_u64(lists + 16 * i)?;
                let len = (reader.read_u64(lists + 16 * i + 8)? as u32) / 16;
                for j in 0..u64::from(len.min(16)) {
                    let key = reader.read_u64(data + 16 * j)?;
                    let word = reader.read_u64(data + 16 * j + 8)?;
                    let t = f32::from_bits(word as u32);
                    let tail = word >> 32;
                    line += &format!(" [{i}] {key:x} {t:.2} {tail:x};");
                }
            }
            if line != last {
                println!("{:7.1}s{line}", start.elapsed().as_secs_f32());
                last = line;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("strs") {
        // strs <addr> [len] [depth]: pointers in the object that lead to text, two levels deep.
        let at = hex(args.get(1), 0)?;
        let len = usize::try_from(hex(args.get(2), 0x200)?)?;
        let depth: usize = args.get(3).map_or(Ok(2), |s| s.parse())?;
        let mut frontier = vec![(at, String::new())];
        for _ in 0..depth {
            let mut next = Vec::new();
            for (object, path) in frontier {
                let Ok(data) = reader.read_vec(object, len) else {
                    continue;
                };
                for (i, w) in data.as_chunks::<8>().0.iter().enumerate() {
                    let ptr = u64::from_le_bytes(*w);
                    if !(0x10000..0x7fff_ffff_ffff).contains(&ptr) {
                        continue;
                    }
                    let here = format!("{path}+{:#x}", 8 * i);
                    if let Ok(text) = reader.read_vec(ptr, 64) {
                        let run = text.iter().take_while(|b| b.is_ascii_graphic()).count();
                        if run >= 6 {
                            println!("{here} -> {}", String::from_utf8_lossy(&text[..run]));
                        }
                        next.push((ptr, format!("{here}->")));
                    }
                }
            }
            frontier = next;
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("text") {
        // text <addr> [len]: the bytes at <addr> as ASCII, dots for the rest.
        let at = hex(args.get(1), 0)?;
        let data = reader.read_vec(at, usize::try_from(hex(args.get(2), 0x80)?)?)?;
        let shown: String = data
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("{shown}");
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("swig") {
        // swig <type_info rva>: heap words naming the SWIG type, with the words around them.
        let base = reader.module(GAME_PROCESS)?.base;
        let want = base + hex(args.get(1), 0)?;
        for region in reader
            .regions()?
            .into_iter()
            .filter(|r| r.readable && r.writable)
        {
            let Ok(data) = reader.read_vec(region.range.start, usize::try_from(region.size())?)
            else {
                continue;
            };
            for (i, w) in data.as_chunks::<8>().0.iter().enumerate() {
                if u64::from_le_bytes(*w) == want {
                    let around: Vec<String> = (i.saturating_sub(3)..i + 4)
                        .filter_map(|j| data.get(8 * j..8 * j + 8))
                        .map(|b| {
                            format!("{:x}", u64::from_le_bytes(b.try_into().unwrap_or_default()))
                        })
                        .collect();
                    println!(
                        "{:#x}: {}",
                        region.range.start + 8 * i as u64,
                        around.join(" ")
                    );
                }
            }
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("dump") {
        // dump <addr> <len>: words with their two floats, skipping zeros.
        let at = hex(args.get(1), 0)?;
        let len = hex(args.get(2), 0x400)?;
        let data = reader.read_vec(at, usize::try_from(len)?)?;
        for (i, w) in data.as_chunks::<8>().0.iter().enumerate() {
            let q = u64::from_le_bytes(*w);
            if q == 0 {
                continue;
            }
            let lo = f32::from_le_bytes(w[..4].try_into()?);
            let hi = f32::from_le_bytes(w[4..].try_into()?);
            let f = |f: f32| {
                if f.abs() > 1e-3 && f.abs() < 1e6 {
                    format!("{f:10.3}")
                } else {
                    format!("{:>10}", "·")
                }
            };
            println!("+{:04x} {q:016x} {} {}", 8 * i, f(lo), f(hi));
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("instances") {
        // instances <vtable>: every object starting with <vtable>, with its first floats.
        let vtable = hex(args.get(1), 0)?;
        for region in reader
            .regions()?
            .into_iter()
            .filter(|r| r.readable && r.writable)
        {
            let Ok(data) = reader.read_vec(region.range.start, usize::try_from(region.size())?)
            else {
                continue;
            };
            for (i, w) in data.as_chunks::<8>().0.iter().enumerate() {
                if u64::from_le_bytes(*w) != vtable {
                    continue;
                }
                let at = 8 * i;
                let floats: Vec<String> = (8..0x60)
                    .step_by(4)
                    .filter_map(|o| data.get(at + o..at + o + 4))
                    .map(|b| f32::from_le_bytes(b.try_into().unwrap_or_default()))
                    .map(|f| {
                        if f.abs() > 1e-3 && f.abs() < 1e5 {
                            format!("{f:.2}")
                        } else {
                            "·".into()
                        }
                    })
                    .collect();
                println!(
                    "{:#x}: {}",
                    region.range.start + at as u64,
                    floats.join(" ")
                );
            }
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("falling") {
        // falling <total>: timers whose full duration is <total> and whose remaining time falls.
        let total: f32 = args
            .get(1)
            .ok_or("usage: ptr_scan falling <total>")?
            .parse()?;
        let read = || -> Res<Vec<(u64, f32)>> {
            let mut out = Vec::new();
            for region in reader
                .regions()?
                .into_iter()
                .filter(|r| r.readable && r.writable)
            {
                let Ok(data) = reader.read_vec(region.range.start, usize::try_from(region.size())?)
                else {
                    continue;
                };
                for (i, w) in data.as_chunks::<4>().0.iter().enumerate().skip(1) {
                    if f32::from_le_bytes(*w) == total {
                        let left = f32::from_le_bytes(data[4 * i - 4..4 * i].try_into()?);
                        if left > 0.0 && left < total {
                            out.push((region.range.start + 4 * (i as u64 - 1), left));
                        }
                    }
                }
            }
            Ok(out)
        };
        if args.get(2).map(String::as_str) == Some("all") {
            for (at, left) in read()? {
                let tail = reader.read_vec(at + 8, 12).unwrap_or_default();
                let f = |i: usize| {
                    tail.get(i..i + 4).map_or(0.0, |b| {
                        f32::from_le_bytes(b.try_into().unwrap_or_default())
                    })
                };
                println!(
                    "{at:#x}: {left:.2} of {total} then {:.2} {:.2} {:.2}",
                    f(0),
                    f(4),
                    f(8)
                );
            }
            return Ok(());
        }
        let first: HashMap<u64, f32> = read()?.into_iter().collect();
        std::thread::sleep(std::time::Duration::from_secs(2));
        for (at, left) in read()? {
            if let Some(&was) = first.get(&at)
                && was > left
            {
                println!("{at:#x}: {was:.1} -> {left:.1}");
            }
        }
        return Ok(());
    }
    let meet = if args.first().map(String::as_str) == Some("meet") {
        Some(hex(args.get(1), 0)?)
    } else {
        None
    };
    let args: Vec<String> = if meet.is_some() {
        std::iter::once("tribute".to_owned())
            .chain(args.into_iter().skip(2))
            .collect()
    } else {
        args
    };
    let mut targets = if args.first().map(String::as_str) == Some("tribute") {
        tribute_timers(&*reader)?
    } else {
        vec![
            hex(args.first(), 0)
                .map_err(|_| "usage: ptr_scan <addr|tribute> [depth] [reach] [keep]")?,
        ]
    };
    for t in &targets {
        let mut b = [0u8; 8];
        reader.read_exact(*t, &mut b)?;
        let f = |i: usize| f32::from_le_bytes(b[i..i + 4].try_into().unwrap_or_default());
        println!("timer {t:#x}: {:.1} of {:.1}", f(0), f(4));
    }
    let depth: usize = args.get(1).map_or(Ok(4), |s| s.parse())?;
    let reach = hex(args.get(2), 0x400)?;
    let keep: usize = args.get(3).map_or(Ok(4000), |s| s.parse())?;

    let module = reader.module(GAME_PROCESS)?;
    let image = module.base..module.base + module.size;
    let regions: Vec<_> = reader
        .regions()?
        .into_iter()
        .filter(|r| r.readable && r.writable)
        .collect();

    // Per level: holder address -> (the address it leads to, offset added to the pointer).
    let mut levels: Vec<HashMap<u64, (u64, u64)>> = Vec::new();
    let mut found = 0;
    for level in 0..depth {
        targets.sort_unstable();
        let mut holders: HashMap<u64, (u64, u64)> = HashMap::new();
        for region in &regions {
            let mut at = region.range.start;
            while at < region.range.end {
                let len = usize::try_from((region.range.end - at).min(CHUNK as u64))?;
                let Ok(data) = reader.read_vec(at, len) else {
                    break;
                };
                for (i, word) in data.as_chunks::<8>().0.iter().enumerate() {
                    let ptr = u64::from_le_bytes(*word);
                    let next = targets.partition_point(|&t| t < ptr);
                    let Some(&t) = targets.get(next) else {
                        continue;
                    };
                    if t - ptr <= reach {
                        holders.insert(at + 8 * i as u64, (t, t - ptr));
                    }
                }
                at += len as u64;
            }
        }
        let statics: Vec<u64> = holders
            .keys()
            .copied()
            .filter(|h| image.contains(h))
            .collect();
        println!(
            "level {}: {} holders, {} in the image",
            level + 1,
            holders.len(),
            statics.len()
        );
        if holders.len() <= 20 {
            let mut shown: Vec<_> = holders.iter().collect();
            shown.sort_unstable();
            for (h, (t, off)) in shown {
                println!("  {h:#x} -> {t:#x} (+{off:#x})");
            }
        }
        levels.push(holders);
        for &root in &statics {
            print_chain(root, module.base, &levels);
            found += 1;
        }
        if found >= 20 {
            break;
        }
        if let Some(root) = meet
            && walk_forward(&*reader, root, &levels, reach)
        {
            break;
        }
        let mut next: Vec<(u64, u64)> = levels[level]
            .iter()
            .filter(|(h, _)| !image.contains(h))
            .map(|(&h, &(_, off))| (off, h))
            .collect();
        next.sort_unstable();
        next.truncate(keep);
        targets = next.into_iter().map(|(_, h)| h).collect();
        if targets.is_empty() {
            break;
        }
    }
    Ok(())
}

fn print_chain(root: u64, base: u64, levels: &[HashMap<u64, (u64, u64)>]) {
    let mut line = format!("[exe+{:#x}]", root - base);
    let mut at = root;
    for level in levels.iter().rev() {
        let Some(&(next, off)) = level.get(&at) else {
            break;
        };
        line.push_str(&format!(" +{off:#x}"));
        at = next;
    }
    println!("  {line} -> {at:#x}");
}

/// Tribute's buff state: remaining seconds, the full duration, then 5.0, 60.0 and 0.2.
fn tribute_timers(reader: &dyn wf_mem::MemoryReader) -> Res<Vec<u64>> {
    let tail: Vec<u8> = [5.0f32, 60.0, 0.2]
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let finder = memchr::memmem::Finder::new(&tail);
    let mut found = Vec::new();
    for region in reader
        .regions()?
        .into_iter()
        .filter(|r| r.readable && r.writable)
    {
        let Ok(data) = reader.read_vec(region.range.start, usize::try_from(region.size())?) else {
            continue;
        };
        for at in finder.find_iter(&data) {
            if at < 8 || at % 4 != 0 {
                continue;
            }
            let f = |i: usize| f32::from_le_bytes(data[i..i + 4].try_into().unwrap_or_default());
            let (left, total) = (f(at - 8), f(at - 4));
            if (10.0..=600.0).contains(&total) && left > 0.0 && left <= total {
                found.push(region.range.start + (at - 8) as u64);
            }
        }
    }
    Ok(found)
}

/// Follows pointers out of `root`, three objects deep, looking for a word that is one of the
/// holders the backward walk found; prints the full path when it does.
fn walk_forward(
    reader: &dyn wf_mem::MemoryReader,
    root: u64,
    levels: &[HashMap<u64, (u64, u64)>],
    span: u64,
) -> bool {
    let mut seen = std::collections::HashSet::new();
    let mut frontier = vec![(root, String::new())];
    for _ in 0..3 {
        let mut next = Vec::new();
        for (object, path) in frontier {
            let Ok(data) = reader.read_vec(object, usize::try_from(span).unwrap_or(0)) else {
                continue;
            };
            for (i, word) in data.as_chunks::<8>().0.iter().enumerate() {
                let field = object + 8 * i as u64;
                let here = format!("{path} +{:#x}", 8 * i);
                if let Some(level) = levels.iter().rposition(|l| l.contains_key(&field)) {
                    let mut line = format!("[root]{here}");
                    let mut at = field;
                    for l in levels[..=level].iter().rev() {
                        let Some(&(to, off)) = l.get(&at) else { break };
                        line.push_str(&format!(" -> +{off:#x}"));
                        at = to;
                    }
                    println!("  {line} => {at:#x}");
                    return true;
                }
                let ptr = u64::from_le_bytes(*word);
                if (0x10000..0x7ff0_0000_0000).contains(&ptr)
                    && ptr % 8 == 0
                    && seen.insert(ptr)
                    && next.len() < 200_000
                {
                    next.push((ptr, format!("{here} ->")));
                }
            }
        }
        frontier = next;
    }
    false
}
