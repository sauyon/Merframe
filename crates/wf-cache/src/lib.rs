//! Files out of the game's local content cache, `Cache.Windows` in its install folder.
//!
//! The cache is split into packages, each a `.toc` index and a `.cache` blob: `H.Texture`
//! holds texture headers, `B.Texture` their bodies, and so on. An index entry gives a file's
//! path, where its bytes start in the blob and their stored and full lengths. Stored bytes are
//! a run of blocks, each with an 8-byte big-endian header of its stored and full lengths, and
//! Oodle-compressed unless the two match.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use oozextract::Extractor;

const MAGIC: u32 = 0x1867_C64E;
const HEADER: usize = 8;
const ENTRY: usize = 96;
const NAME_AT: usize = 32;
const BLOCK_HEADER: usize = 8;
/// Texture bodies are BC3: 16-byte blocks of 4×4 pixels.
const BC3_BLOCK: usize = 16;
const MAX_ICON: usize = 4096;

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("reading the cache failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a cache index")]
    NotIndex,
    #[error("a cache block is damaged")]
    Block,
    #[error("decompressing a cache block failed: {0}")]
    Decompress(String),
    #[error("not a texture this can decode")]
    Texture,
    #[error("encoding the image failed: {0}")]
    Png(#[from] png::EncodingError),
}

pub type Result<T> = std::result::Result<T, CacheError>;

/// The cache folder the game says it scans, from the start of its `EE.log`.
pub fn cache_dir(log: &str) -> Option<PathBuf> {
    log.lines().find_map(|line| {
        let (_, rest) = line.split_once("Scanning ")?;
        let (dir, _) = rest.split_once(" for cache blocks")?;
        Some(PathBuf::from(dir))
    })
}

/// Where a file's bytes are in a package's blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    offset: u64,
    stored: usize,
    len: usize,
}

/// One package: its index, and the blob the index points into.
#[derive(Debug)]
pub struct Package {
    entries: Vec<Entry>,
    blob: PathBuf,
}

impl Package {
    /// The package `name`, like `B.Texture`, in the cache folder `dir`.
    pub fn open(dir: &Path, name: &str) -> Result<Self> {
        let index = std::fs::read(dir.join(format!("{name}.toc")))?;
        Ok(Self {
            entries: entries(&index)?,
            blob: dir.join(format!("{name}.cache")),
        })
    }

    /// Every file's path. A path can be listed more than once; the last one is current.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|entry| entry.path.as_str())
    }

    /// The file at `path`, whole, or `None` when the package has no such file.
    pub fn read(&self, path: &str) -> Result<Option<Vec<u8>>> {
        let Some(entry) = self.entries.iter().rev().find(|entry| entry.path == path) else {
            return Ok(None);
        };
        let mut stored = vec![0u8; entry.stored];
        let mut blob = File::open(&self.blob)?;
        blob.seek(SeekFrom::Start(entry.offset))?;
        blob.read_exact(&mut stored)?;
        unpack(&stored, entry.len).map(Some)
    }
}

/// The entries of a package index. Folders are entries too, with no bytes, that later
/// entries name as their parent, counted from 1 in the order they appear.
fn entries(index: &[u8]) -> Result<Vec<Entry>> {
    if index.len() < HEADER || u32_at(index, 0) != Some(MAGIC) {
        return Err(CacheError::NotIndex);
    }
    let mut folders = vec![String::new()];
    let mut files = Vec::new();
    for entry in index[HEADER..].as_chunks::<ENTRY>().0 {
        let field = |at: usize| u32_at(entry, at).ok_or(CacheError::NotIndex);
        let offset = i64::from_le_bytes(entry[..8].try_into().map_err(|_| CacheError::NotIndex)?);
        let parent = usize::try_from(field(28)?).map_err(|_| CacheError::NotIndex)?;
        let name = &entry[NAME_AT..];
        let name = &name[..name.iter().position(|&b| b == 0).unwrap_or(name.len())];
        let path = format!(
            "{}/{}",
            folders.get(parent).ok_or(CacheError::NotIndex)?,
            String::from_utf8_lossy(name)
        );
        match u64::try_from(offset) {
            Ok(offset) => files.push(Entry {
                path,
                offset,
                stored: usize::try_from(field(16)?).map_err(|_| CacheError::NotIndex)?,
                len: usize::try_from(field(20)?).map_err(|_| CacheError::NotIndex)?,
            }),
            Err(_) => folders.push(path),
        }
    }
    Ok(files)
}

/// A file's bytes from how they are stored: as they are when nothing was compressed,
/// otherwise block by block.
fn unpack(stored: &[u8], len: usize) -> Result<Vec<u8>> {
    if stored.len() == len {
        return Ok(stored.to_vec());
    }
    let mut out = Vec::with_capacity(len);
    let mut rest = stored;
    while out.len() < len {
        let header = rest.get(..BLOCK_HEADER).ok_or(CacheError::Block)?;
        let high = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        let low = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);
        let size = usize::try_from((high & 0x7FFF_FFFF) >> 2).map_err(|_| CacheError::Block)?;
        let full = usize::try_from(low >> 5).map_err(|_| CacheError::Block)?;
        let body = rest
            .get(BLOCK_HEADER..BLOCK_HEADER + size)
            .ok_or(CacheError::Block)?;
        if size == full {
            out.extend_from_slice(body);
        } else {
            let at = out.len();
            out.resize(at + full, 0);
            let done = Extractor::new()
                .read_from_slice(body, &mut out[at..])
                .map_err(|error| CacheError::Decompress(error.to_string()))?;
            if done != full {
                return Err(CacheError::Block);
            }
        }
        rest = &rest[BLOCK_HEADER + size..];
    }
    out.truncate(len);
    Ok(out)
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// An icon's texture body as a PNG. Icons are square BC3 with every mip level, stored
/// smallest first, so the full-size image is the last `side²` bytes.
pub fn icon_png(body: &[u8]) -> Result<Vec<u8>> {
    let side = icon_side(body.len()).ok_or(CacheError::Texture)?;
    let top = &body[body.len() - side * side..];
    let rgba = bc3_rgba(top, side);
    let mut png = Vec::new();
    let side = u32::try_from(side).map_err(|_| CacheError::Texture)?;
    let mut encoder = png::Encoder::new(&mut png, side, side);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&rgba)?;
    Ok(png)
}

/// The side of the square BC3 texture whose full mip chain is `len` bytes.
fn icon_side(len: usize) -> Option<usize> {
    (2..=MAX_ICON.trailing_zeros())
        .map(|bits| 1usize << bits)
        .find(|&side| {
            let mut level = side;
            let mut total = 0;
            loop {
                let blocks = level.div_ceil(4);
                total += blocks * blocks * BC3_BLOCK;
                if level == 1 {
                    break;
                }
                level /= 2;
            }
            total == len
        })
}

/// RGBA pixels of a `side`×`side` BC3 image.
fn bc3_rgba(data: &[u8], side: usize) -> Vec<u8> {
    let across = side / 4;
    let mut out = vec![0u8; side * side * 4];
    for (index, block) in data.as_chunks::<BC3_BLOCK>().0.iter().enumerate() {
        let (bx, by) = (index % across * 4, index / across * 4);
        let alphas = alpha_palette(block[0], block[1]);
        let alpha_bits = block[2..8]
            .iter()
            .rev()
            .fold(0u64, |bits, &byte| bits << 8 | u64::from(byte));
        let colours = colour_palette(
            u16::from_le_bytes([block[8], block[9]]),
            u16::from_le_bytes([block[10], block[11]]),
        );
        let colour_bits = u32::from_le_bytes([block[12], block[13], block[14], block[15]]);
        for pixel in 0..16 {
            let at = ((by + pixel / 4) * side + bx + pixel % 4) * 4;
            let colour = colours[(colour_bits >> (pixel * 2) & 3) as usize];
            out[at..at + 3].copy_from_slice(&colour);
            out[at + 3] = alphas[(alpha_bits >> (pixel * 3) & 7) as usize];
        }
    }
    out
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "a weighted mean of two bytes is a byte"
)]
fn mix(a: u32, b: u32, wa: u32, wb: u32) -> u8 {
    ((a * wa + b * wb) / (wa + wb)) as u8
}

fn alpha_palette(a0: u8, a1: u8) -> [u8; 8] {
    let (a, b) = (u32::from(a0), u32::from(a1));
    let mut palette = [a0, a1, 0, 0, 0, 0, 0, 255];
    let between: u32 = if a0 > a1 { 7 } else { 5 };
    for (slot, step) in palette[2..].iter_mut().zip(1..between) {
        *slot = mix(a, b, between - step, step);
    }
    palette
}

fn colour_palette(c0: u16, c1: u16) -> [[u8; 3]; 4] {
    // A 5- or 6-bit channel stretched to 0..=255.
    let expand = |value: u16, max: u32| mix(u32::from(value) * 255 / max, 0, 1, 0);
    let rgb = |c: u16| {
        [
            expand(c >> 11, 31),
            expand(c >> 5 & 0x3F, 63),
            expand(c & 0x1F, 31),
        ]
    };
    let (a, b) = (rgb(c0), rgb(c1));
    let blend = |wa: u32, wb: u32| [0, 1, 2].map(|i| mix(u32::from(a[i]), u32::from(b[i]), wa, wb));
    [a, b, blend(2, 1), blend(1, 2)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(offset: i64, stored: u32, len: u32, parent: u32, name: &str) -> Vec<u8> {
        let mut bytes = vec![0u8; ENTRY];
        bytes[..8].copy_from_slice(&offset.to_le_bytes());
        bytes[16..20].copy_from_slice(&stored.to_le_bytes());
        bytes[20..24].copy_from_slice(&len.to_le_bytes());
        bytes[28..32].copy_from_slice(&parent.to_le_bytes());
        bytes[NAME_AT..NAME_AT + name.len()].copy_from_slice(name.as_bytes());
        bytes
    }

    #[test]
    fn index_paths_follow_their_folders() {
        let mut index = MAGIC.to_le_bytes().to_vec();
        index.extend(20u32.to_le_bytes());
        index.extend(entry(-1, 0, 0, 0, "Lotus"));
        index.extend(entry(-1, 0, 0, 1, "Icons"));
        index.extend(entry(64, 10, 20, 2, "Soul.png"));
        index.extend(entry(0, 5, 5, 1, "Top.txt"));
        let files = entries(&index).expect("index");
        let paths: Vec<_> = files.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["/Lotus/Icons/Soul.png", "/Lotus/Top.txt"]);
        assert_eq!(
            (files[0].offset, files[0].stored, files[0].len),
            (64, 10, 20)
        );
        assert!(matches!(entries(b"nope"), Err(CacheError::NotIndex)));
    }

    #[test]
    fn cache_folder_comes_from_the_log() {
        let log = "0.170 Sys [Info]: Starting\n\
            0.180 Sys [Info]: Scanning C:\\Games\\Warframe\\Cache.Windows\\ for cache blocks...\n";
        assert_eq!(
            cache_dir(log),
            Some(PathBuf::from("C:\\Games\\Warframe\\Cache.Windows\\"))
        );
        assert_eq!(cache_dir("0.1 Sys [Info]: Starting"), None);
    }

    #[test]
    fn stored_blocks_are_joined() {
        let block = |body: &[u8]| {
            let len = u32::try_from(body.len()).expect("len");
            let mut out = (0x8000_0000 | len << 2).to_be_bytes().to_vec();
            out.extend((len << 5 | 1).to_be_bytes());
            out.extend_from_slice(body);
            out
        };
        let mut stored = block(b"abc");
        stored.extend(block(b"de"));
        assert_eq!(unpack(&stored, 5).expect("blocks"), b"abcde");
        assert_eq!(unpack(b"plain", 5).expect("raw"), b"plain");
        assert!(matches!(unpack(&stored[..6], 5), Err(CacheError::Block)));
    }

    #[test]
    fn icon_sizes_come_from_their_mip_chain() {
        assert_eq!(icon_side(21_872), Some(128));
        assert_eq!(icon_side(16 * 3), Some(4));
        assert_eq!(icon_side(21_871), None);
    }

    #[test]
    fn bc3_blocks_decode() {
        // Opaque white top row and see-through black elsewhere: alpha 255/0 picking index 0
        // for the top row and 1 below, colour white/black picking 0 and 1 the same way.
        let mut block = [0u8; 16];
        block[0] = 255;
        block[1] = 0;
        let alpha_bits: u64 = (4..16).fold(0, |bits, pixel| bits | 1 << (pixel * 3));
        block[2..8].copy_from_slice(&alpha_bits.to_le_bytes()[..6]);
        block[8..10].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let colour_bits: u32 = (4..16).fold(0, |bits, pixel| bits | 1 << (pixel * 2));
        block[12..16].copy_from_slice(&colour_bits.to_le_bytes());
        let rgba = bc3_rgba(&block, 4);
        assert_eq!(rgba[..4], [255, 255, 255, 255]);
        assert_eq!(rgba[16 * 4 - 4..], [0, 0, 0, 0]);
        assert_eq!(alpha_palette(0, 255)[6..], [0, 255]);
        assert_eq!(alpha_palette(255, 0)[..3], [255, 0, 218]);
    }
}
