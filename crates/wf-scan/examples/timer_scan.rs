//! Finds timers in the game's memory, Cheat Engine style. Candidates persist in a file
//! between runs so passes can be timed around casts.
//!
//! ```text
//!   timer_scan new <lo> <hi>   every f32/f64 in [lo, hi]
//!   timer_scan dec             keep values that fell by the wall time since the last read
//!   timer_scan inc             keep values that rose by the wall time since the last read
//!   timer_scan up              keep values that jumped up (a buff was refreshed)
//!   timer_scan same|changed    keep unchanged / changed values
//!   timer_scan list [n]        show candidates with their current values
//!   timer_scan hunt <lo> <hi>  all in one: scan, narrow to countdowns, then watch for the
//!                              one that jumps up when you recast and dump memory around it
//!   timer_scan peek <addr>     dump memory around an address
//!   timer_scan chase <addr> [depth]  follow pointers near an address, printing any text found
//!
//! ```
//!
//! Options: `--state <file>` (default `timer_scan.state`), `--tol <secs>` (default 0.25),
//! `--f64` to also scan doubles (f32 only by default).

use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::io::{BufReader, BufWriter, Read, Write as _};
use std::time::{SystemTime, UNIX_EPOCH};

use wf_mem::{MemoryReader, open_game};

type Res<T> = Result<T, Box<dyn Error>>;

const CHUNK: usize = 16 << 20;
const BLOCK: u64 = 64 << 10;
const MAX_CANDIDATES: usize = 30_000_000;

#[derive(Clone, Copy)]
struct Cand {
    addr: u64,
    is_f64: bool,
    val: f64,
    t: f64,
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

fn main() -> Res<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let state = take_opt(&mut args, "--state").unwrap_or_else(|| "timer_scan.state".into());
    let tol: f64 = take_opt(&mut args, "--tol").map_or(Ok(0.25), |s| s.parse())?;
    let wide = take_flag(&mut args, "--f64");
    let expect: Option<f64> = take_opt(&mut args, "--expect").map(|s| s.parse()).transpose()?;
    let cmd = args.first().cloned().unwrap_or_default();
    let reader = open_game()?;

    let cands = match cmd.as_str() {
        "new" => {
            let (Some(lo), Some(hi)) = (args.get(1), args.get(2)) else {
                return Err("usage: timer_scan new <lo> <hi>".into());
            };
            full_scan(&*reader, lo.parse()?, hi.parse()?, wide)?
        }
        "hunt" => {
            let (Some(lo), Some(hi)) = (args.get(1), args.get(2)) else {
                return Err("usage: timer_scan hunt <lo> <hi>".into());
            };
            return hunt(&*reader, lo.parse()?, hi.parse()?, wide, tol, expect, &state);
        }
        "peek" => {
            let addr = args.get(1).ok_or("usage: timer_scan peek <addr>")?;
            let addr = u64::from_str_radix(addr.trim_start_matches("0x"), 16)?;
            print!("{}", peek(&*reader, addr));
            return Ok(());
        }
        "chase" => {
            let addr = args.get(1).ok_or("usage: timer_scan chase <addr> [depth]")?;
            let addr = u64::from_str_radix(addr.trim_start_matches("0x"), 16)?;
            let depth: usize = args.get(2).map_or(Ok(3), |s| s.parse())?;
            chase(&*reader, addr & !7, depth);
            return Ok(());
        }
        "list" => {
            let n: usize = args.get(1).map_or(Ok(40), |s| s.parse())?;
            let c = load(&state)?;
            for (old, new) in c.iter().zip(reread(&*reader, &c)).take(n) {
                let v = new.map_or("-".into(), |n| format!("{:.3}", n.val));
                println!("{:#014x} {} was {:.3} now {v}", old.addr, width(old), old.val);
            }
            println!("{} candidates", c.len());
            return Ok(());
        }
        "dec" | "inc" | "up" | "same" | "changed" => {
            let c = load(&state)?;
            c.iter()
                .zip(reread(&*reader, &c))
                .filter_map(|(o, n)| {
                    let n = n?;
                    let dt = n.t - o.t;
                    let dv = n.val - o.val;
                    let keep = match cmd.as_str() {
                        "dec" => dv < 0.0 && (dv + dt).abs() <= tol,
                        "inc" => dv > 0.0 && (dv - dt).abs() <= tol,
                        "up" => dv > tol,
                        "same" => dv.to_bits() == 0.0f64.to_bits() || n.val.to_bits() == o.val.to_bits(),
                        _ => n.val.to_bits() != o.val.to_bits(),
                    };
                    keep.then_some(n)
                })
                .collect()
        }
        _ => return Err("usage: timer_scan new <lo> <hi> | dec | inc | up | same | changed | list [n]".into()),
    };
    save(&state, &cands)?;
    println!("{} candidates", cands.len());
    for c in cands.iter().take(20) {
        println!("  {:#014x} {} {:.3}", c.addr, width(c), c.val);
    }
    Ok(())
}

fn width(c: &Cand) -> &'static str {
    if c.is_f64 { "f64" } else { "f32" }
}

fn take_opt(args: &mut Vec<String>, name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    if i + 1 >= args.len() {
        return None;
    }
    let v = args.remove(i + 1);
    args.remove(i);
    Some(v)
}

fn full_scan(reader: &dyn MemoryReader, lo: f64, hi: f64, wide: bool) -> Res<Vec<Cand>> {
    let mut out = Vec::new();
    let mut buf = vec![0u8; CHUNK];
    for region in reader.regions()? {
        if !(region.readable && region.writable) {
            continue;
        }
        let mut pos = region.range.start;
        while pos < region.range.end {
            let len = usize::try_from((region.range.end - pos).min(CHUNK as u64))?;
            let n = reader.read(pos, &mut buf[..len]).unwrap_or(0);
            let t = now();
            let data = &buf[..n];
            for (i, w) in data.as_chunks::<4>().0.iter().enumerate() {
                let off = i * 4;
                let addr = pos + off as u64;
                let v = f64::from(f32::from_le_bytes(*w));
                if (lo..=hi).contains(&v) {
                    out.push(Cand { addr, is_f64: false, val: v, t });
                }
                if wide && off % 8 == 0
                    && let Some(b) = data.get(off..off + 8)
                    && let Ok(b) = <[u8; 8]>::try_from(b)
                {
                    let v = f64::from_le_bytes(b);
                    if (lo..=hi).contains(&v) {
                        out.push(Cand { addr, is_f64: true, val: v, t });
                    }
                }
            }
            if out.len() > MAX_CANDIDATES {
                return Err(format!("over {MAX_CANDIDATES} candidates; narrow the range").into());
            }
            pos += len as u64;
        }
    }
    Ok(out)
}

/// Re-reads candidates in address order, one 64 KB block at a time.
fn reread(reader: &dyn MemoryReader, cands: &[Cand]) -> Vec<Option<Cand>> {
    let mut out = vec![None; cands.len()];
    let mut order: Vec<usize> = (0..cands.len()).collect();
    order.sort_by_key(|&i| cands[i].addr);
    let mut block: Option<(u64, Vec<u8>, f64)> = None;
    for i in order {
        let c = cands[i];
        let base = c.addr & !(BLOCK - 1);
        if block.as_ref().is_none_or(|(b, _, _)| *b != base) {
            // Read a little past the block so an f64 at its end is whole.
            let data = reader.read_vec(base, usize::try_from(BLOCK).unwrap_or(0) + 8).unwrap_or_default();
            block = Some((base, data, now()));
        }
        let Some((b, data, t)) = &block else { continue };
        let Ok(o) = usize::try_from(c.addr - b) else { continue };
        let val = if c.is_f64 {
            data.get(o..o + 8)
                .and_then(|s| <[u8; 8]>::try_from(s).ok())
                .map(f64::from_le_bytes)
        } else {
            data.get(o..o + 4)
                .and_then(|s| <[u8; 4]>::try_from(s).ok())
                .map(|s| f64::from(f32::from_le_bytes(s)))
        };
        out[i] = val.map(|val| Cand { val, t: *t, ..c });
    }
    out
}

fn save(path: &str, cands: &[Cand]) -> Res<()> {
    let mut w = BufWriter::new(fs::File::create(path)?);
    for c in cands {
        w.write_all(&c.addr.to_le_bytes())?;
        w.write_all(&[u8::from(c.is_f64)])?;
        w.write_all(&c.val.to_le_bytes())?;
        w.write_all(&c.t.to_le_bytes())?;
    }
    Ok(())
}

fn load(path: &str) -> Res<Vec<Cand>> {
    let mut r = BufReader::new(fs::File::open(path).map_err(|e| format!("{path}: {e}; run `timer_scan new` first"))?);
    let mut out = Vec::new();
    let mut rec = [0u8; 25];
    while r.read_exact(&mut rec).is_ok() {
        let word = |at: usize| <[u8; 8]>::try_from(&rec[at..at + 8]).unwrap_or_default();
        out.push(Cand {
            addr: u64::from_le_bytes(word(0)),
            is_f64: rec[8] != 0,
            val: f64::from_le_bytes(word(9)),
            t: f64::from_le_bytes(word(17)),
        });
    }
    Ok(out)
}

fn take_flag(args: &mut Vec<String>, name: &str) -> bool {
    let found = args.iter().position(|a| a == name);
    if let Some(i) = found {
        args.remove(i);
    }
    found.is_some()
}

fn countdowns(reader: &dyn MemoryReader, cands: &[Cand], tol: f64) -> Vec<Cand> {
    cands
        .iter()
        .zip(reread(reader, cands))
        .filter_map(|(o, n)| {
            let n = n?;
            let dv = n.val - o.val;
            (dv < 0.0 && (dv + (n.t - o.t)).abs() <= tol).then_some(n)
        })
        .collect()
}

const NARROW_PASSES: usize = 3;
const WATCH_SECS: f64 = 240.0;
const REFRESH_JUMP: f64 = 1.0;
const MIN_LIVE: f64 = 1.0;
/// How far a refreshed value may land from `--expect`; the first read lags the reset.
const EXPECT_TOL: f64 = 1.0;

fn hunt(
    reader: &dyn MemoryReader,
    lo: f64,
    hi: f64,
    wide: bool,
    tol: f64,
    expect: Option<f64>,
    state: &str,
) -> Res<()> {
    println!("scanning...");
    let mut cands = full_scan(reader, lo, hi, wide)?;
    println!("{} values in [{lo}, {hi}]", cands.len());
    for _ in 0..NARROW_PASSES {
        std::thread::sleep(std::time::Duration::from_secs(1));
        cands = countdowns(reader, &cands, tol);
        println!("{} counting down", cands.len());
    }
    save(state, &cands)?;
    cands.sort_by(|a, b| b.val.total_cmp(&a.val));
    for c in cands.iter().take(10) {
        println!("  {:#014x} {:.2}", c.addr, c.val);
    }

    println!("\nwatching for a recast: {} timers for {WATCH_SECS:.0}s)", cands.len());
    let start = now();
    while now() - start < WATCH_SECS && !cands.is_empty() {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let fresh = reread(reader, &cands);
        let refreshed: Vec<(Cand, Cand)> = cands
            .iter()
            .zip(&fresh)
            .filter_map(|(o, n)| {
                // A recast lands mid-countdown; a jump from ~0 is an effect re-arming.
                n.filter(|n| {
                    o.val > MIN_LIVE
                        && n.val > o.val + REFRESH_JUMP
                        && expect.is_none_or(|e| (n.val - e).abs() <= EXPECT_TOL)
                })
                    .map(|n| (*o, n))
            })
            .collect();
        if !refreshed.is_empty() {
            let mut report = String::new();
            for (o, n) in &refreshed {
                let _ = writeln!(report, "REFRESHED {:#014x}: {:.2} -> {:.2}", n.addr, o.val, n.val);
            }
            for (_, n) in &refreshed {
                let _ = write!(report, "\n== around {:#014x}\n{}", n.addr, peek(reader, n.addr));
                let _ = write!(report, "\n== pointers into {:#014x} - {REF_SPAN:#x}\n", n.addr);
                report += &refs(reader, n.addr)?;
            }
            print!("{report}");
            let out = format!("{state}.hunt.txt");
            fs::write(&out, &report)?;
            println!("\nsaved to {out}");
            return Ok(());
        }
        cands = fresh.into_iter().flatten().filter(|c| c.val > 0.0).collect();
    }
    println!("no timer jumped up");
    Ok(())
}

const PEEK_BEFORE: u64 = 0x100;
const PEEK_AFTER: u64 = 0x100;

/// Hex dump around `addr`, decoding each word as floats and following pointers to text.
fn peek(reader: &dyn MemoryReader, addr: u64) -> String {
    let base = (addr - PEEK_BEFORE) & !7;
    let Ok(data) = reader.read_vec(base, usize::try_from(PEEK_BEFORE + PEEK_AFTER).unwrap_or(0)) else {
        return "unreadable\n".into();
    };
    let mut out = String::new();
    for (i, w) in data.as_chunks::<8>().0.iter().enumerate() {
        let at = base + (i as u64) * 8;
        let word = u64::from_le_bytes(*w);
        let lo = f32::from_le_bytes([w[0], w[1], w[2], w[3]]);
        let hi = f32::from_le_bytes([w[4], w[5], w[6], w[7]]);
        let mark = if (at..at + 8).contains(&addr) { ">>" } else { "  " };
        let off = i64::try_from(at).unwrap_or(0) - i64::try_from(addr & !7).unwrap_or(0);
        let sign = if off < 0 { '-' } else { '+' };
        let _ = write!(out, "{mark}{sign}{:03x} {word:016x}  f32 {lo:>12.3} {hi:>12.3}", off.unsigned_abs());
        if let Some(s) = text_at(reader, word) {
            let _ = write!(out, "  -> {s:?}");
        } else if let Some(s) = reader.read_u64(word).ok().and_then(|p| text_at(reader, p)) {
            let _ = write!(out, "  ->-> {s:?}");
        }
        out += "\n";
    }
    out
}

/// Printable text at `addr`, directly or as a Lua-style string header (text at +24).
fn text_at(reader: &dyn MemoryReader, addr: u64) -> Option<String> {
    if !(0x1_0000..0x7fff_ffff_ffff).contains(&addr) {
        return None;
    }
    let bytes = reader.read_vec(addr, 96).ok()?;
    [0usize, 24].into_iter().find_map(|skip| {
        let run: Vec<u8> = bytes
            .get(skip..)?
            .iter()
            .take_while(|b| b.is_ascii_graphic() || **b == b' ')
            .copied()
            .collect();
        (run.len() >= 6).then(|| String::from_utf8_lossy(&run).into_owned())
    })
}

const CHASE_SPAN: u64 = 0x100;

/// Follows every pointer in the 0x100 bytes either side of `addr`, `depth` levels deep,
/// printing the path to any text it reaches.
fn chase(reader: &dyn MemoryReader, addr: u64, depth: usize) {
    let mut seen = std::collections::HashSet::new();
    let base = addr.saturating_sub(CHASE_SPAN);
    walk(reader, base, CHASE_SPAN * 2, depth, &format!("{addr:#x}"), addr, &mut seen);
}

fn walk(
    reader: &dyn MemoryReader,
    base: u64,
    len: u64,
    depth: usize,
    path: &str,
    origin: u64,
    seen: &mut std::collections::HashSet<u64>,
) {
    let Ok(data) = reader.read_vec(base, usize::try_from(len).unwrap_or(0)) else {
        return;
    };
    for (i, w) in data.as_chunks::<8>().0.iter().enumerate() {
        let ptr = u64::from_le_bytes(*w);
        if !(0x1_0000..0x7fff_ffff_ffff).contains(&ptr) || !seen.insert(ptr) {
            continue;
        }
        let at = base + (i as u64) * 8;
        let off = i128::from(at) - i128::from(origin);
        let step = format!("{path}[{}{:#x}]", if off < 0 { "-" } else { "+" }, off.unsigned_abs());
        if let Some(text) = text_at(reader, ptr) {
            println!("{step} -> {text:?}");
        }
        if depth > 1 {
            walk(reader, ptr, 0x80, depth - 1, &step, ptr, seen);
        }
    }
}

/// How far before a timer its object may start.
const REF_SPAN: u64 = 0x200;
const MAX_REFS: usize = 40;

/// Every 8-byte-aligned word in writable memory pointing into `[addr - REF_SPAN, addr]`.
fn refs(reader: &dyn MemoryReader, addr: u64) -> Res<String> {
    let lo = addr - REF_SPAN;
    let mut out = String::new();
    let mut found = 0;
    let mut buf = vec![0u8; CHUNK];
    for region in reader.regions()? {
        if !(region.readable && region.writable) {
            continue;
        }
        let mut pos = region.range.start;
        while pos < region.range.end {
            let len = usize::try_from((region.range.end - pos).min(CHUNK as u64))?;
            let n = reader.read(pos, &mut buf[..len]).unwrap_or(0);
            for (i, w) in buf[..n].as_chunks::<8>().0.iter().enumerate() {
                let v = u64::from_le_bytes(*w);
                if (lo..=addr).contains(&v) {
                    let at = pos + (i as u64) * 8;
                    let _ = writeln!(out, "{at:#014x} -> {v:#014x} (timer at +{:#x})", addr - v);
                    found += 1;
                    if found >= MAX_REFS {
                        return Ok(out);
                    }
                }
            }
            pos += len as u64;
        }
    }
    Ok(out)
}
