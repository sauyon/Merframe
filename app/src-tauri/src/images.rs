use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::sync::Semaphore;

use crate::error::{CommandError, CommandResult, cause_chain};

pub struct ImageCache {
    dir: PathBuf,
    http: reqwest::Client,
    slots: Semaphore,
}

impl ImageCache {
    pub fn new(data_dir: &Path, http: reqwest::Client) -> Self {
        Self {
            dir: data_dir.join("images"),
            http,
            slots: Semaphore::new(6),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub async fn prepare(&self) -> std::io::Result<()> {
        tokio::fs::create_dir_all(&self.dir).await
    }

    pub fn cached_path(&self, image_name: &str) -> Option<PathBuf> {
        let file = sanitise(image_name)?;
        Some(self.dir.join(file))
    }

    pub async fn get(&self, image_name: &str) -> CommandResult<String> {
        let path = self
            .cached_path(image_name)
            .ok_or_else(|| CommandError::from(format!("Not a usable image name: {image_name}")))?;
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return to_string(&path, image_name);
        }
        let _permit = self.slots.acquire().await;
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return to_string(&path, image_name);
        }
        let bytes = self.download(image_name).await?;
        let partial = path.with_extension("part");
        let write_failed = |source: std::io::Error| {
            CommandError::from(format!(
                "Writing the image cache for {image_name} failed: {source}"
            ))
        };
        tokio::fs::write(&partial, &bytes)
            .await
            .map_err(write_failed)?;
        tokio::fs::rename(&partial, &path)
            .await
            .map_err(write_failed)?;
        to_string(&path, image_name)
    }

    async fn download(&self, image_name: &str) -> CommandResult<Vec<u8>> {
        self.fetch(&source_url(image_name)).await.map_err(|source| {
            CommandError::from(format!(
                "Downloading {image_name} failed: {}",
                cause_chain(&source)
            ))
        })
    }

    async fn fetch(&self, url: &str) -> Result<Vec<u8>, reqwest::Error> {
        let response = self
            .http
            .get(url)
            .timeout(Duration::from_secs(10))
            .send()
            .await?
            .error_for_status()?;
        Ok(response.bytes().await?.to_vec())
    }
}

// Export icons arrive as their game path with dots for slashes, since cached
// names must be flat; anything else is a WFCD image.
fn source_url(image_name: &str) -> String {
    match image_name
        .strip_prefix("Lotus.")
        .and_then(|path| path.strip_suffix(".png"))
    {
        Some(path) => format!("https://browse.wf/Lotus/{}.png", path.replace('.', "/")),
        None => format!("https://cdn.warframestat.us/img/{image_name}"),
    }
}

fn to_string(path: &Path, image_name: &str) -> CommandResult<String> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        CommandError::from(format!(
            "Image cache path for {image_name} is not valid UTF-8"
        ))
    })
}

fn sanitise(image_name: &str) -> Option<&str> {
    let trimmed = image_name.trim();
    if trimmed.is_empty() || trimmed.len() > 128 {
        return None;
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    {
        return None;
    }
    if trimmed.starts_with('.') || trimmed.contains("..") {
        return None;
    }
    Some(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn plain_cdn_file_names() {
        assert_eq!(sanitise("Ash.png"), Some("Ash.png"));
        assert_eq!(
            sanitise("braton-prime-barrel.png"),
            Some("braton-prime-barrel.png")
        );
        assert_eq!(sanitise(" Ash.png "), Some("Ash.png"));
    }

    #[test]
    fn export_icons_come_from_browse_wf() {
        let icon = "Lotus.Interface.Icons.StoreIcons.Captura.Deimos.png";
        assert_eq!(sanitise(icon), Some(icon));
        assert_eq!(
            source_url(icon),
            "https://browse.wf/Lotus/Interface/Icons/StoreIcons/Captura/Deimos.png"
        );
        assert_eq!(
            source_url("Ash.png"),
            "https://cdn.warframestat.us/img/Ash.png"
        );
    }

    #[test]
    fn path_escapes_rejected() {
        assert_eq!(sanitise(""), None);
        assert_eq!(sanitise("../../etc/passwd"), None);
        assert_eq!(sanitise("sub/dir/Ash.png"), None);
        assert_eq!(sanitise(".hidden"), None);
        assert_eq!(sanitise("Ash.png?query=1"), None);
        assert_eq!(sanitise(&"a".repeat(200)), None);
    }

    #[test]
    fn cached_path_under_images_dir() {
        let cache = ImageCache::new(Path::new("/tmp/merframe-test"), reqwest::Client::new());
        assert_eq!(cache.dir(), Path::new("/tmp/merframe-test/images"));
        assert_eq!(
            cache.cached_path("Ash.png"),
            Some(Path::new("/tmp/merframe-test/images/Ash.png").to_path_buf())
        );
        assert_eq!(cache.cached_path("../escape.png"), None);
    }
}
