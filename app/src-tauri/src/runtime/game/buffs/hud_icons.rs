//! Icons for the buffs an ability's own HUD lists, taken from the game's content cache.
//!
//! The game keeps them beside the ability's own icon, named for the ability and ending with
//! the name the ability's script gives the buff: Tribute's `thorns` is
//! `FaerieSoulBuffThorns.png` next to `FaerieSoul.png`. Each is decoded once into the app's
//! image folder.

use std::io::Read;
use std::path::Path;

use tracing::debug;
use wf_cache::Package;

const HEADERS: &str = "H.Texture";
const BODIES: &str = "B.Texture";
/// How much of `EE.log` to read for the cache folder, which the game names as it starts.
const LOG_HEAD: u64 = 64 * 1024;
/// Marks decoded icons in the image folder, apart from downloaded ones.
const PREFIX: &str = "cache-";

/// An ability's icon and the name its script gives one of its buffs.
pub(super) type Wanted = (String, String);

/// The image name of each wanted icon, or `None` where the cache has none.
pub(super) fn find(
    log: Option<&Path>,
    images: &Path,
    wanted: Vec<Wanted>,
) -> Vec<(Wanted, Option<String>)> {
    let packages = log.and_then(open);
    wanted
        .into_iter()
        .map(|key| {
            let found = packages
                .as_ref()
                .and_then(|(headers, bodies)| decode(headers, bodies, images, &key.0, &key.1));
            debug!(ability = key.0, buff = key.1, ?found, "Ability HUD icon");
            (key, found)
        })
        .collect()
}

/// The game's icon named `name`, like `FocusCleanMadurai.png`, decoded into `images` unless
/// it already is, for icons the item catalog doesn't carry, like the operator's focus school.
pub(super) fn find_named(log: Option<&Path>, images: &Path, name: &str) -> Option<String> {
    let (headers, bodies) = log.and_then(open)?;
    let wanted = name.to_lowercase();
    let path = headers
        .paths()
        .filter(|path| {
            path.rsplit('/')
                .next()
                .is_some_and(|file| file.to_lowercase() == wanted)
        })
        .min_by_key(|path| path.len())?
        .to_owned();
    let found = save(&bodies, images, &path);
    debug!(name, ?found, "Named game icon");
    found
}

fn open(log: &Path) -> Option<(Package, Package)> {
    let mut head = Vec::new();
    std::fs::File::open(log)
        .and_then(|file| file.take(LOG_HEAD).read_to_end(&mut head))
        .map_err(|error| debug!(%error, "EE.log not read for the game's cache"))
        .ok()?;
    let dir = wf_cache::cache_dir(&String::from_utf8_lossy(&head))?;
    let opened = Package::open(&dir, HEADERS).and_then(|headers| {
        let bodies = Package::open(&dir, BODIES)?;
        Ok((headers, bodies))
    });
    opened
        .map_err(|error| debug!(%error, dir = %dir.display(), "Game cache not opened"))
        .ok()
}

/// The icon for `buff` beside `ability_icon`, decoded into `images` unless it already is.
fn decode(
    headers: &Package,
    bodies: &Package,
    images: &Path,
    ability_icon: &str,
    buff: &str,
) -> Option<String> {
    let split = |path: &str| {
        path.rsplit_once('/')
            .map(|(dir, name)| (dir.to_owned(), name.to_lowercase()))
    };
    let own = ability_icon.to_lowercase();
    let (dir, _) = headers
        .paths()
        .filter_map(split)
        .find(|(_, name)| *name == own)?;
    let stem = own.strip_suffix(".png").unwrap_or(&own);
    let tail = format!("{}.png", buff.to_lowercase());
    let path = headers
        .paths()
        .filter(|path| {
            split(path).is_some_and(|(at, name)| {
                at == dir && name != own && name.starts_with(stem) && name.ends_with(&tail)
            })
        })
        .min_by_key(|path| path.len())?;
    save(bodies, images, path)
}

/// The icon at `path` in the cache, decoded into `images` unless it already is, by its name
/// there.
fn save(bodies: &Package, images: &Path, path: &str) -> Option<String> {
    let name = format!("{PREFIX}{}", path.rsplit('/').next()?);
    let file = images.join(&name);
    if !file.exists() {
        let png = bodies
            .read(path)
            .and_then(|body| body.map(|body| wf_cache::icon_png(&body)).transpose())
            .map_err(|error| debug!(%error, path, "Ability HUD icon not decoded"))
            .ok()??;
        std::fs::write(&file, png)
            .map_err(|error| debug!(%error, file = %file.display(), "Ability HUD icon not saved"))
            .ok()?;
    }
    Some(name)
}
