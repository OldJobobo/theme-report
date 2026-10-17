use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

pub const PROFILE: &str = "quattro-shell-2026-v1";
pub const TEXT_LIMIT: u64 = 1_048_576;
pub const IMAGE_LIMIT: u64 = 16 * 1024 * 1024;
pub const IMAGE_DISK_LIMIT: u64 = 256 * 1024 * 1024;
pub const TOTAL_LIMIT: usize = 128 * 1024 * 1024;
pub const VIDEO_LIMIT: u64 = 256 * 1024 * 1024;
pub const VIDEO_TOTAL_LIMIT: u64 = 1024 * 1024 * 1024;
#[derive(Clone)]
pub struct MediaInput {
    pub file: Arc<tempfile::NamedTempFile>,
    pub hash: String,
    pub format: String,
    pub size: u64,
}
#[derive(Clone)]
pub struct Config {
    pub theme: PathBuf,
    pub current: bool,
    pub home: PathBuf,
    pub omarchy: PathBuf,
    pub cache: PathBuf,
    pub profile: Option<String>,
}
#[derive(Clone)]
pub struct Snapshot {
    pub files: BTreeMap<String, Vec<u8>>,
    pub paths: BTreeMap<String, String>,
    pub unsupported_backgrounds: BTreeMap<String, String>,
    pub videos: BTreeMap<String, MediaInput>,
    pub large_images: BTreeMap<String, MediaInput>,
    capture_started: Instant,
    pub fingerprint: String,
    pub theme: String,
    pub profile_supported: bool,
    pub config: Config,
}
pub fn hash(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

// Exact reviewed contracts, kept as complete tuples to reject mixed generations.
// Qt 6.12 variants qualify Color references or rename the singleton to ShellColor.
// r6866 removes the image-picker scrim and adds bar notch geometry; neither
// changes the palette/state defaults or surfaces assessed by this profile.
const ORIGINAL_TEMPLATE: &str = "bdc8e76a5a9a700adaca0d2fa44a2a340584892fd507e3fcbd4f9a4843765242";
const PALETTE_FALLBACKS: &str = "a429ed1b18114ff3e784fc4479c6b829e9effb64c0f9d22d3b15eda4c62d86b5";
const RUNTIME_CONTRACTS: &[(&str, &str, &str, &str)] = &[
    (
        "b7b0f7df6e820851a50ddc641e1a87e33b8a6adbc14149fddf2d1e4f879dd163",
        "3d09947d917633b167f08d250696302e126e608c47991c604dca969d692f6221",
        ORIGINAL_TEMPLATE,
        PALETTE_FALLBACKS,
    ),
    (
        "cc9b62e2eaf8c307aa02206d9320c3d0ca349cf8feed452a6eddf387adaa5320",
        "f5901092617ae62645597199270afd4dfb0644d596bdfff16e1834fca6ad7243",
        ORIGINAL_TEMPLATE,
        PALETTE_FALLBACKS,
    ),
    (
        "b7b0f7df6e820851a50ddc641e1a87e33b8a6adbc14149fddf2d1e4f879dd163",
        "0b2aec0b87ee0c895cedd93c4357b46c1009b7302ddc75aa6d99ad5b259e1b02",
        ORIGINAL_TEMPLATE,
        PALETTE_FALLBACKS,
    ),
    (
        "b7b0f7df6e820851a50ddc641e1a87e33b8a6adbc14149fddf2d1e4f879dd163",
        "d0a7ab5222931313b4b3831d3cc3a1d5c6c6ad9e8fcbb41b96e8af0402c05a92",
        ORIGINAL_TEMPLATE,
        PALETTE_FALLBACKS,
    ),
    (
        "08af401286831759b63279cf6d6732d666256c751864a03bd5594ea29a29f0e5",
        "f84c801f2457a93130c99cb284de2615baafea6bd15de47cf190f0b11e5598a0",
        "38461c50c432b7332a3f59673536e8a784745bf678c58dfa3dcae7e4a91196b3",
        PALETTE_FALLBACKS,
    ),
];
fn supported_contract(color: &str, style: &str, template: &str, palette: &str) -> bool {
    RUNTIME_CONTRACTS.contains(&(color, style, template, palette))
}
pub(crate) fn bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    // Nonblocking/no-follow open prevents a file-to-FIFO/symlink swap from hanging capture.
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    let f = options
        .open(path)
        .with_context(|| format!("Cannot read {}", path.display()))?;
    if !f.metadata()?.is_file() {
        bail!("Not a regular file: {}", path.display());
    }
    let mut v = Vec::new();
    f.take(limit + 1).read_to_end(&mut v)?;
    if v.len() as u64 > limit {
        bail!("Input exceeds {limit} bytes: {}", path.display())
    }
    Ok(v)
}
impl Snapshot {
    pub fn capture(config: &Config) -> Result<Self> {
        let root = config
            .theme
            .canonicalize()
            .context("Theme directory is unavailable")?;
        let mut s = Self {
            files: BTreeMap::new(),
            paths: BTreeMap::new(),
            unsupported_backgrounds: BTreeMap::new(),
            videos: BTreeMap::new(),
            large_images: BTreeMap::new(),
            capture_started: Instant::now(),
            fingerprint: String::new(),
            theme: root
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            profile_supported: false,
            config: config.clone(),
        };
        for name in ["colors.toml", "shell.toml", "hyprland.lua"] {
            s.add(name, &root.join(name), TEXT_LIMIT, Some(&root))?;
        }
        if config.current {
            let base = config.home.join(".local/state/omarchy/current");
            s.add("theme.name", &base.join("theme.name"), TEXT_LIMIT, None)?;
            if let Some(v) = s.files.get("theme.name") {
                s.theme = String::from_utf8_lossy(v).trim().into();
            }
            s.add(
                "user.shell.toml",
                &config.home.join(".config/omarchy/shell.toml"),
                TEXT_LIMIT,
                None,
            )?;
            // The explicit current background is an authorized input, not a directory traversal root.
            s.add_background("current.background", &base.join("background"), None)?;
        }
        let backgrounds = root.join("backgrounds");
        if backgrounds.exists() {
            let b = backgrounds.canonicalize()?;
            if !b.starts_with(&root) {
                bail!("Background directory escapes theme root")
            }
            let mut entries = fs::read_dir(&b)?.collect::<std::io::Result<Vec<_>>>()?;
            entries.sort_by_key(|e| e.file_name());
            if entries.len() > 16 {
                bail!("Background set exceeds 16 entries")
            }
            for entry in entries {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') {
                    continue;
                }
                s.add_background(&format!("backgrounds/{name}"), &entry.path(), Some(&root))?;
            }
        }
        if config.current {
            let color_relative = if config.omarchy.join("shell/Commons/ShellColor.qml").exists() {
                "shell/Commons/ShellColor.qml"
            } else {
                "shell/Commons/Color.qml"
            };
            for (role, relative) in [
                ("runtime.color", color_relative),
                ("runtime.style", "shell/Commons/Style.qml"),
                ("runtime.template", "default/themed/shell.toml.tpl"),
                ("runtime.geometry", "shell/Commons/BorderGeometry.js"),
                ("runtime.palette-fallbacks", "bin/omarchy-theme-color"),
            ] {
                s.add(role, &config.omarchy.join(relative), TEXT_LIMIT, None)?;
            }
            let color = s
                .files
                .get("runtime.color")
                .map(|v| hash(v))
                .unwrap_or_default();
            let style = s
                .files
                .get("runtime.style")
                .map(|v| hash(v))
                .unwrap_or_default();
            let tpl = s
                .files
                .get("runtime.template")
                .map(|v| hash(v))
                .unwrap_or_default();
            let palette_contract = s
                .files
                .get("runtime.palette-fallbacks")
                .map(|v| hash(v))
                .unwrap_or_default();
            s.profile_supported = supported_contract(&color, &style, &tpl, &palette_contract);
        } else {
            s.profile_supported = config.profile.as_deref() == Some(PROFILE);
        }
        if s.files.values().map(Vec::len).sum::<usize>() > TOTAL_LIMIT {
            bail!("Snapshot exceeds 128 MiB")
        }
        let mut digest = Sha256::new();
        digest.update(b"manifest-v4\0rules-v4\0model-v4\0");
        digest.update(env!("CARGO_PKG_VERSION"));
        digest.update(PROFILE);
        for (role, bytes) in &s.files {
            digest.update((role.len() as u64).to_be_bytes());
            digest.update(role.as_bytes());
            digest.update((bytes.len() as u64).to_be_bytes());
            digest.update(bytes);
        }
        if !s.large_images.is_empty()
            || s.files
                .keys()
                .any(|k| k.starts_with("backgrounds/") || k == "current.background")
        {
            let identity = crate::wallpaper::backend_identity();
            digest.update(identity.as_bytes());
            s.files
                .insert("runtime.image-decoder".into(), identity.into_bytes());
        }
        if !s.videos.is_empty() {
            let identity = crate::video::backend_identity();
            digest.update(identity.as_bytes());
            s.files
                .insert("runtime.video-decoder".into(), identity.into_bytes());
        }
        for (role, video) in s.videos.iter().chain(&s.large_images) {
            digest.update(b"\0video-content\0");
            digest.update((role.len() as u64).to_be_bytes());
            digest.update(role.as_bytes());
            digest.update(video.hash.as_bytes());
            digest.update(video.format.as_bytes());
        }
        for (role, descriptor) in &s.unsupported_backgrounds {
            digest.update(b"\0unsupported-background\0");
            digest.update((role.len() as u64).to_be_bytes());
            digest.update(role.as_bytes());
            digest.update((descriptor.len() as u64).to_be_bytes());
            digest.update(descriptor.as_bytes());
        }
        // Missing inputs have explicit stable markers, even when a file is absent.
        for (role, path) in &s.paths {
            if !s.files.contains_key(role)
                && !s.unsupported_backgrounds.contains_key(role)
                && !s.videos.contains_key(role)
                && !s.large_images.contains_key(role)
            {
                digest.update(role.as_bytes());
                digest.update(b"\0missing\0");
            }
            let _ = path;
        }
        s.fingerprint = format!("{:x}", digest.finalize());
        Ok(s)
    }
    fn add_background(&mut self, role: &str, path: &Path, root: Option<&Path>) -> Result<()> {
        self.paths
            .insert(role.into(), path.to_string_lossy().into());
        let resolved = match path.canonicalize() {
            Ok(path) => path,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        if root.is_some_and(|r| !resolved.starts_with(r)) {
            bail!("Input symlink escapes theme root: {}", path.display())
        }
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
        }
        let mut file = options.open(&resolved)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            bail!("Not a regular file: {}", path.display())
        }
        let extension = resolved
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let mut header = [0u8; 16];
        let count = file.read(&mut header)?;
        let format = image::guess_format(&header[..count]).ok();
        let is_image = matches!(extension.as_str(), "png" | "jpg" | "jpeg")
            || matches!(
                format,
                Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
            );
        if is_image && metadata.len() <= IMAGE_LIMIT {
            self.add(role, &resolved, IMAGE_LIMIT, root)?;
            return Ok(());
        }
        let video_format = match extension.as_str() {
            "mp4" | "mov" | "m4v" => Some("mov"),
            "webm" | "mkv" => Some("matroska"),
            "avi" => Some("avi"),
            _ if header[..count].starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) => Some("matroska"),
            _ if count >= 8 && &header[4..8] == b"ftyp" => Some("mov"),
            _ => None,
        };
        let disk_format = if is_image {
            Some("image")
        } else {
            video_format
        };
        if let Some(format) = disk_format {
            let input_limit = if is_image {
                IMAGE_DISK_LIMIT
            } else {
                VIDEO_LIMIT
            };
            if metadata.len() > input_limit
                || self
                    .videos
                    .values()
                    .chain(self.large_images.values())
                    .map(|v| v.size)
                    .sum::<u64>()
                    + metadata.len()
                    > VIDEO_TOTAL_LIMIT
            {
                self.unsupported_backgrounds.insert(
                    role.into(),
                    format!(
                        "media snapshot byte limit: file={}; bytes={}; modified={:?}",
                        resolved.file_name().unwrap_or_default().to_string_lossy(),
                        metadata.len(),
                        metadata.modified()?
                    ),
                );
                return Ok(());
            }
            fs::create_dir_all(&self.config.cache)?;
            let mut snapshot = tempfile::NamedTempFile::new_in(&self.config.cache)?;
            file.rewind()?;
            let mut hash = Sha256::new();
            let mut buffer = [0u8; 65536];
            let mut size = 0u64;
            loop {
                if self.capture_started.elapsed() > Duration::from_secs(10) {
                    bail!("Media capture deadline exceeded")
                }
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                size += n as u64;
                if size > input_limit
                    || self
                        .videos
                        .values()
                        .chain(self.large_images.values())
                        .map(|v| v.size)
                        .sum::<u64>()
                        + size
                        > VIDEO_TOTAL_LIMIT
                {
                    bail!("Media changed beyond snapshot byte limit")
                }
                hash.update(&buffer[..n]);
                snapshot.write_all(&buffer[..n])?;
            }
            snapshot.flush()?;
            let after = file.metadata()?;
            if size != metadata.len()
                || after.len() != metadata.len()
                || after.modified()? != metadata.modified()?
            {
                bail!("unstable-input: media changed during capture")
            }
            let hash = format!("{:x}", hash.finalize());
            let shared = self
                .videos
                .values()
                .chain(self.large_images.values())
                .find(|v| v.hash == hash)
                .map(|v| v.file.clone())
                .unwrap_or_else(|| Arc::new(snapshot));
            let target = if is_image {
                &mut self.large_images
            } else {
                &mut self.videos
            };
            target.insert(
                role.into(),
                MediaInput {
                    file: shared,
                    hash,
                    format: format.into(),
                    size,
                },
            );
            return Ok(());
        }
        let descriptor = format!(
            "unsupported format: {}; file={}; bytes={}; modified={:?}",
            extension,
            resolved.file_name().unwrap_or_default().to_string_lossy(),
            metadata.len(),
            metadata.modified()?
        );
        self.unsupported_backgrounds.insert(role.into(), descriptor);
        Ok(())
    }
    fn add(&mut self, role: &str, path: &Path, limit: u64, root: Option<&Path>) -> Result<()> {
        self.paths
            .insert(role.into(), path.to_string_lossy().into());
        match fs::symlink_metadata(path) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        }
        // A broken link is a missing asset; its role remains in the manifest.
        let resolved = match path.canonicalize() {
            Ok(p) => p,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        if root.is_some_and(|r| !resolved.starts_with(r)) {
            bail!("Input symlink escapes theme root: {}", path.display())
        }
        let bytes = bounded(&resolved, limit)?;
        if self.files.values().map(Vec::len).sum::<usize>() + bytes.len() > TOTAL_LIMIT {
            bail!("Snapshot exceeds 128 MiB")
        }
        self.files.insert(role.into(), bytes);
        Ok(())
    }
    pub fn text(&self, role: &str) -> Option<&str> {
        self.files
            .get(role)
            .and_then(|b| std::str::from_utf8(b).ok())
    }
    pub fn stable(&self) -> Result<bool> {
        let next = Self::capture(&self.config)?;
        Ok(next.fingerprint == self.fingerprint && next.paths == self.paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reviewed_runtime_variants_do_not_disable_verification() {
        for &(color, style, template, palette) in RUNTIME_CONTRACTS {
            assert!(supported_contract(color, style, template, palette));
            assert!(!supported_contract("unknown", style, template, palette));
            assert!(!supported_contract(color, "unknown", template, palette));
            assert!(!supported_contract(color, style, "unknown", palette));
            assert!(!supported_contract(color, style, template, "unknown"));
        }
        assert!(!supported_contract(
            RUNTIME_CONTRACTS[0].0,
            RUNTIME_CONTRACTS[1].1,
            ORIGINAL_TEMPLATE,
            PALETTE_FALLBACKS
        ));
    }

    #[test]
    fn r6866_contract_requires_the_reviewed_combination() {
        let color = "08af401286831759b63279cf6d6732d666256c751864a03bd5594ea29a29f0e5";
        let style = "f84c801f2457a93130c99cb284de2615baafea6bd15de47cf190f0b11e5598a0";
        let template = "38461c50c432b7332a3f59673536e8a784745bf678c58dfa3dcae7e4a91196b3";
        let palette = "a429ed1b18114ff3e784fc4479c6b829e9effb64c0f9d22d3b15eda4c62d86b5";
        assert!(supported_contract(color, style, template, palette));
        for &(old_color, old_style, old_template, _) in &RUNTIME_CONTRACTS[..4] {
            assert!(!supported_contract(old_color, style, template, palette));
            assert!(!supported_contract(color, old_style, template, palette));
            assert!(!supported_contract(color, style, old_template, palette));
            assert!(!supported_contract(old_color, old_style, template, palette));
        }
    }
}
