use crate::contract::Contract;
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
/// Wallpapers sampled per audit; extra files become a visible limitation.
pub const BACKGROUND_LIMIT: usize = 32;
const IMAGE_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];
const VIDEO_EXTENSIONS: [&str; 6] = ["mp4", "mov", "m4v", "webm", "mkv", "avi"];
// Recognized wallpaper formats that cannot be sampled yet; they stay limitations.
const OTHER_MEDIA_EXTENSIONS: [&str; 9] = [
    "gif", "bmp", "avif", "heic", "heif", "tif", "tiff", "jxl", "svg",
];
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
    /// Entries in backgrounds/ that are not wallpaper media (directories, notes).
    pub ignored_backgrounds: Vec<String>,
    pub videos: BTreeMap<String, MediaInput>,
    pub large_images: BTreeMap<String, MediaInput>,
    capture_started: Instant,
    pub fingerprint: String,
    /// Cheap metadata manifest taken before any content was read.
    pub manifest: String,
    pub theme: String,
    pub contract: Contract,
    pub config: Config,
}
pub fn hash(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
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

enum Kind {
    Text,
    Background,
}
struct Input {
    role: String,
    path: PathBuf,
    kind: Kind,
    root: Option<PathBuf>,
}
/// Every input an audit reads, discovered without reading file contents.
struct Plan {
    root: PathBuf,
    inputs: Vec<Input>,
    ignored: Vec<String>,
    over_limit: Vec<String>,
    escaped_backgrounds: bool,
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}
fn sniff(path: &Path) -> Vec<u8> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let mut header = vec![0u8; 16];
    let count = options
        .open(path)
        .and_then(|mut f| f.read(&mut header))
        .unwrap_or(0);
    header.truncate(count);
    header
}
fn video_magic(header: &[u8]) -> Option<&'static str> {
    if header.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        Some("matroska")
    } else if header.len() >= 8 && &header[4..8] == b"ftyp" {
        Some("mov")
    } else {
        None
    }
}
/// Wallpaper media by extension, or by content for extension-less files.
fn is_media(path: &Path) -> bool {
    let ext = extension(path);
    if IMAGE_EXTENSIONS.contains(&ext.as_str())
        || VIDEO_EXTENSIONS.contains(&ext.as_str())
        || OTHER_MEDIA_EXTENSIONS.contains(&ext.as_str())
    {
        return true;
    }
    let header = sniff(path);
    image::guess_format(&header).is_ok() || video_magic(&header).is_some()
}

fn plan(config: &Config) -> Result<Plan> {
    let root = config
        .theme
        .canonicalize()
        .context("Theme directory is unavailable")?;
    let mut plan = Plan {
        inputs: vec![],
        ignored: vec![],
        over_limit: vec![],
        escaped_backgrounds: false,
        root: root.clone(),
    };
    let mut push = |role: &str, path: PathBuf, kind: Kind, root: Option<&Path>| {
        plan.inputs.push(Input {
            role: role.into(),
            path,
            kind,
            root: root.map(Path::to_path_buf),
        })
    };
    for name in ["colors.toml", "shell.toml", "hyprland.lua"] {
        push(name, root.join(name), Kind::Text, Some(&root));
    }
    if config.current {
        let base = config.home.join(".local/state/omarchy/current");
        push("theme.name", base.join("theme.name"), Kind::Text, None);
        push(
            "user.shell.toml",
            config.home.join(".config/omarchy/shell.toml"),
            Kind::Text,
            None,
        );
        // The explicit current background is an authorized input, not a directory traversal root.
        push(
            "current.background",
            base.join("background"),
            Kind::Background,
            None,
        );
    }
    let backgrounds = root.join("backgrounds");
    if backgrounds.exists() {
        if !backgrounds.canonicalize()?.starts_with(&root) {
            plan.escaped_backgrounds = true;
        } else {
            let mut entries = fs::read_dir(&backgrounds)?.collect::<std::io::Result<Vec<_>>>()?;
            entries.sort_by_key(|e| e.file_name());
            let mut media = Vec::new();
            for entry in entries {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') {
                    continue;
                }
                let path = entry.path();
                match fs::metadata(&path) {
                    // Omarchy themes ship backgrounds/intros/ and similar folders; never recurse.
                    Ok(m) if m.is_dir() => plan.ignored.push(format!("backgrounds/{name}/")),
                    Ok(m) if !m.is_file() => plan.ignored.push(format!("backgrounds/{name}")),
                    Ok(_) if !is_media(&path) => plan.ignored.push(format!("backgrounds/{name}")),
                    // A broken link stays a (missing) background role.
                    Ok(_) => media.push((name, path)),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => media.push((name, path)),
                    Err(e) => return Err(e.into()),
                }
            }
            for (i, (name, path)) in media.into_iter().enumerate() {
                if i >= BACKGROUND_LIMIT {
                    plan.over_limit.push(format!("backgrounds/{name}"));
                } else {
                    plan.inputs.push(Input {
                        role: format!("backgrounds/{name}"),
                        path,
                        kind: Kind::Background,
                        root: Some(root.clone()),
                    });
                }
            }
        }
    }
    if config.current {
        for (role, path) in runtime_inputs(&config.omarchy) {
            plan.inputs.push(Input {
                role: role.into(),
                path,
                kind: Kind::Text,
                root: None,
            });
        }
    }
    Ok(plan)
}
fn runtime_inputs(omarchy: &Path) -> [(&'static str, PathBuf); 5] {
    let color = if omarchy.join("shell/Commons/ShellColor.qml").exists() {
        "shell/Commons/ShellColor.qml"
    } else {
        "shell/Commons/Color.qml"
    };
    [
        ("runtime.color", omarchy.join(color)),
        ("runtime.style", omarchy.join("shell/Commons/Style.qml")),
        (
            "runtime.template",
            omarchy.join("default/themed/shell.toml.tpl"),
        ),
        (
            "runtime.geometry",
            omarchy.join("shell/Commons/BorderGeometry.js"),
        ),
        (
            "runtime.palette-fallbacks",
            omarchy.join("bin/omarchy-theme-color"),
        ),
    ]
}
/// The runtime contract files of an Omarchy root, by snapshot role (missing files omitted).
pub fn runtime_files(omarchy: &Path) -> BTreeMap<String, Vec<u8>> {
    runtime_inputs(omarchy)
        .into_iter()
        .filter_map(|(role, path)| {
            bounded(&path.canonicalize().ok()?, TEXT_LIMIT)
                .ok()
                .map(|bytes| (role.to_string(), bytes))
        })
        .collect()
}

fn manifest_of(plan: &Plan) -> Result<String> {
    use std::os::unix::fs::MetadataExt;
    let mut digest = Sha256::new();
    digest.update(b"manifest-v1\0");
    digest.update(env!("CARGO_PKG_VERSION"));
    digest.update(plan.root.as_os_str().as_encoded_bytes());
    let stat = |digest: &mut Sha256, m: &fs::Metadata| {
        for n in [
            m.len() as i64,
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
            m.ino() as i64,
            m.dev() as i64,
        ] {
            digest.update(n.to_be_bytes());
        }
    };
    for input in &plan.inputs {
        digest.update(b"\0role\0");
        digest.update(input.role.as_bytes());
        // The link itself (retargeting) and what it resolves to.
        if let Ok(link) = fs::symlink_metadata(&input.path) {
            stat(&mut digest, &link);
        }
        match fs::metadata(&input.path) {
            Ok(m) => {
                stat(&mut digest, &m);
                if let Ok(target) = input.path.canonicalize() {
                    digest.update(target.as_os_str().as_encoded_bytes());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => digest.update(b"missing"),
            Err(e) => return Err(e.into()),
        }
    }
    for name in plan.ignored.iter().chain(&plan.over_limit) {
        digest.update(b"\0listed\0");
        digest.update(name.as_bytes());
    }
    digest.update([u8::from(plan.escaped_backgrounds)]);
    // Decoder upgrades change samples; their executables stand in for version probes.
    for tool in [crate::wallpaper::thumbnail_tool(), "ffmpeg", "ffprobe"] {
        digest.update(tool.as_bytes());
        if let Some(path) = std::env::var_os("PATH").and_then(|p| {
            std::env::split_paths(&p)
                .map(|dir| dir.join(tool))
                .find(|path| path.is_file())
        }) && let Ok(m) = fs::metadata(&path)
        {
            stat(&mut digest, &m);
        }
    }
    Ok(format!("{:x}", digest.finalize()))
}
/// Metadata-only manifest of the current inputs: cheap enough to compute repeatedly.
pub fn manifest(config: &Config) -> Result<String> {
    manifest_of(&plan(config)?)
}

impl Snapshot {
    pub fn capture(config: &Config) -> Result<Self> {
        let plan = plan(config)?;
        let manifest = manifest_of(&plan)?;
        let mut s = Self {
            files: BTreeMap::new(),
            paths: BTreeMap::new(),
            unsupported_backgrounds: BTreeMap::new(),
            ignored_backgrounds: plan.ignored.clone(),
            videos: BTreeMap::new(),
            large_images: BTreeMap::new(),
            capture_started: Instant::now(),
            fingerprint: String::new(),
            manifest,
            theme: plan
                .root
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            contract: Contract::profile(),
            config: config.clone(),
        };
        for input in &plan.inputs {
            match input.kind {
                Kind::Text => s.add(&input.role, &input.path, TEXT_LIMIT, input.root.as_deref())?,
                Kind::Background => {
                    s.add_background(&input.role, &input.path, input.root.as_deref())?
                }
            }
        }
        if let Some(v) = s.files.get("theme.name") {
            s.theme = String::from_utf8_lossy(v).trim().into();
        }
        if plan.escaped_backgrounds {
            s.unsupported_backgrounds.insert(
                "backgrounds/".into(),
                "backgrounds directory resolves outside the theme; not read".into(),
            );
        }
        for role in &plan.over_limit {
            s.unsupported_backgrounds.insert(
                role.clone(),
                format!("not sampled: more than {BACKGROUND_LIMIT} wallpapers in the set"),
            );
        }
        s.contract = if config.current {
            Contract::evaluate(&s.files)
        } else if config.profile.as_deref() == Some(PROFILE) {
            Contract::profile()
        } else {
            Contract::evaluate(&BTreeMap::new())
        };
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
        for name in &s.ignored_backgrounds {
            digest.update(b"\0ignored-background\0");
            digest.update(name.as_bytes());
        }
        // Missing inputs have explicit stable markers, even when a file is absent.
        for role in s.paths.keys() {
            if !s.files.contains_key(role)
                && !s.unsupported_backgrounds.contains_key(role)
                && !s.videos.contains_key(role)
                && !s.large_images.contains_key(role)
            {
                digest.update(role.as_bytes());
                digest.update(b"\0missing\0");
            }
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
        // Never read outside the theme; an escaping link is a visible limitation, not an abort.
        if root.is_some_and(|r| !resolved.starts_with(r)) {
            self.unsupported_backgrounds.insert(
                role.into(),
                "symlink resolves outside the theme; not read".into(),
            );
            return Ok(());
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
            self.unsupported_backgrounds
                .insert(role.into(), "not a regular file".into());
            return Ok(());
        }
        let extension = extension(&resolved);
        let mut header = [0u8; 16];
        let count = file.read(&mut header)?;
        let format = image::guess_format(&header[..count]).ok();
        let is_image = IMAGE_EXTENSIONS.contains(&extension.as_str())
            || matches!(
                format,
                Some(image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::WebP)
            );
        if is_image && metadata.len() <= IMAGE_LIMIT {
            self.add(role, &resolved, IMAGE_LIMIT, root)?;
            return Ok(());
        }
        let video_format = match extension.as_str() {
            "mp4" | "mov" | "m4v" => Some("mov"),
            "webm" | "mkv" => Some("matroska"),
            "avi" => Some("avi"),
            _ => video_magic(&header[..count]),
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
            let mut snapshot =
                tempfile::NamedTempFile::new_in(crate::cache::staging(&self.config.cache)?)?;
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
    /// Inputs are unchanged since this snapshot began reading them.
    pub fn stable(&self) -> Result<bool> {
        Ok(manifest(&self.config)? == self.manifest)
    }
}
