//! Names native objects by their MSVC run-time type information.
//!
//! ```text
//! rtti obj <addr>     class of the object at <addr>, and of objects it points to
//! rtti lua            class of every native object Lua scripts hold
//! rtti img <text>     occurrences of <text> in the game's executable image
//! ```

use std::error::Error;

use wf_mem::{GAME_PROCESS, MemoryReader, open_game};
use wf_scan::LuaState;
use wf_scan::lua::LuaValue;

type Res<T> = Result<T, Box<dyn Error>>;

/// Words after an object's start to check for nested objects.
const FIELDS: u64 = 0x200;
/// Words at the start of a Lua userdata to try as object pointers.
const USERDATA_WORDS: u64 = 8;

struct Image {
    base: u64,
    end: u64,
}

fn main() -> Res<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let reader = open_game()?;
    let module = reader.module(GAME_PROCESS)?;
    let image = Image {
        base: module.base,
        end: module.base + module.size,
    };
    match args.first().map(String::as_str) {
        Some("obj") => {
            let addr = args.get(1).ok_or("usage: rtti obj <addr>")?;
            let addr = u64::from_str_radix(addr.trim_start_matches("0x"), 16)?;
            println!("{addr:#x}: {}", class_name(&*reader, &image, addr).unwrap_or_else(|| "?".into()));
            for off in (0..FIELDS).step_by(8) {
                let Ok(ptr) = reader.read_u64(addr + off) else { break };
                if let Some(name) = class_name(&*reader, &image, ptr) {
                    println!("  +{off:#05x} {ptr:#x} {name}");
                }
            }
        }
        Some("lua") => {
            let state = LuaState::locate(&*reader)?.ok_or("no Lua state found")?;
            for (i, script) in state.scripts(&*reader).into_iter().enumerate() {
                let mut tables = vec![script];
                tables.extend(script.locals(&*reader));
                for table in tables {
                    for (key, value) in table.entries(&*reader) {
                        let LuaValue::Userdata(ud) = value else { continue };
                        if let Some((at, name)) = userdata_class(&*reader, &image, ud) {
                            println!("script {i} {key}: {at:#x} {name}");
                        }
                    }
                }
            }
        }
        Some("img") => {
            let pattern = args.get(1).ok_or("usage: rtti img <text> [max]")?;
            let max: usize = args.get(2).map_or(Ok(20), |s| s.parse())?;
            let data = reader.read_vec(image.base, usize::try_from(image.end - image.base)?)?;
            let hits: Vec<usize> = memchr::memmem::find_iter(&data, pattern.as_bytes()).collect();
            println!("{} hits in {} MB image", hits.len(), data.len() >> 20);
            for at in hits.into_iter().take(max) {
                let end = data[at..].iter().position(|&b| b == 0).map_or(at, |n| at + n);
                println!("{:#x} {}", image.base + at as u64, String::from_utf8_lossy(&data[at..end.min(at + 120)]));
            }
        }
        _ => return Err("usage: rtti obj <addr> | rtti lua | rtti img <text>".into()),
    }
    Ok(())
}

fn userdata_class(reader: &dyn MemoryReader, image: &Image, ud: u64) -> Option<(u64, String)> {
    (0..USERDATA_WORDS).find_map(|i| {
        let ptr = reader.read_u64(ud + i * 8).ok()?;
        class_name(reader, image, ptr).map(|n| (ptr, n))
    })
}

/// `obj -> vftable -> [-1] CompleteObjectLocator -> TypeDescriptor -> ".?AVName@@"`.
fn class_name(reader: &dyn MemoryReader, image: &Image, obj: u64) -> Option<String> {
    let in_image = |a: u64| (image.base..image.end).contains(&a);
    let vtable = reader.read_u64(obj).ok()?;
    if !in_image(vtable) {
        return None;
    }
    let locator = reader.read_u64(vtable - 8).ok()?;
    if !in_image(locator) {
        return None;
    }
    let col = reader.read_vec(locator, 24).ok()?;
    // x64 locators have signature 1 and store RVAs.
    if col.get(0..4)? != [1, 0, 0, 0] {
        return None;
    }
    let rva = u32::from_le_bytes(col.get(12..16)?.try_into().ok()?);
    let name = reader.read_vec(image.base + u64::from(rva) + 16, 128).ok()?;
    let end = name.iter().position(|&b| b == 0)?;
    let raw = std::str::from_utf8(&name[..end]).ok()?;
    let class = raw.strip_prefix(".?AV").or_else(|| raw.strip_prefix(".?AU"))?;
    Some(class.trim_end_matches("@@").to_owned())
}
