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
//!
//! ```
//!
//! Options: `--state <file>` (default `timer_scan.state`), `--tol <secs>` (default 0.25).

use std::error::Error;
use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};
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
    let cmd = args.first().cloned().unwrap_or_default();
    let reader = open_game()?;

    let cands = match cmd.as_str() {
        "new" => {
            let (Some(lo), Some(hi)) = (args.get(1), args.get(2)) else {
                return Err("usage: timer_scan new <lo> <hi>".into());
            };
            full_scan(&*reader, lo.parse()?, hi.parse()?)?
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

fn full_scan(reader: &dyn MemoryReader, lo: f64, hi: f64) -> Res<Vec<Cand>> {
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
                if off % 8 == 0
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
