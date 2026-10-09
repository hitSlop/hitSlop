//! Acceptance of a compiler's explicit resource inventory. The accepted result owns
//! every byte that packing will write; no source is opened again after validation.
use super::Artwork;
use crate::app::{AppDefinition, WindowFrame};
use crate::build::{BuildInput, ResourceKind};
use crate::error::{Error, Result, invalid};
use crate::media;
use std::collections::BTreeSet;
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// Ready for packing, with no borrowed paths or unchecked declaration values.
pub struct AcceptedBuild {
    pub(crate) package_format: u64,
    pub(crate) runtime_abi: u64,
    pub(crate) app: AppDefinition,
    pub(crate) checkpoint: Vec<u8>,
    pub(crate) assets: Vec<BuildAsset>,
    pub(crate) artwork: Vec<(Artwork, Vec<u8>)>,
}
pub struct BuildAsset {
    pub key: String,
    pub media_type: &'static str,
    pub bytes: Vec<u8>,
}
impl AcceptedBuild {
    pub fn app(&self) -> &AppDefinition {
        &self.app
    }
    pub fn assets(&self) -> &[BuildAsset] {
        &self.assets
    }
    pub fn artwork(&self) -> &[(Artwork, Vec<u8>)] {
        &self.artwork
    }
    pub fn checkpoint(&self) -> &[u8] {
        &self.checkpoint
    }
    pub fn requirements(&self) -> (u64, u64) {
        (self.package_format, self.runtime_abi)
    }
}

/// Markers precede payload interpretation and filesystem access. The inventory is
/// the only source of files: unlisted files in the private stage are irrelevant.
pub fn accept(input: &str, stage: &Path) -> Result<AcceptedBuild> {
    let input = BuildInput::decode(input).map_err(Error::Rejected)?;
    let app =
        AppDefinition::from_declaration(&input.declaration, input.roles.skin.as_deref()).map_err(Error::Rejected)?;
    if input.roles.ui != "ui.js"
        || input.roles.style.as_deref().is_some_and(|key| key != "ui.css")
        || input.roles.commands.as_deref().is_some_and(|key| key != "commands.js")
    {
        return Err(invalid("Build roles must use the fixed UI, stylesheet and command keys"));
    }
    if input.roles.commands.is_some() == app.commands().is_empty() {
        return Err(invalid("Commands and their private program must be present together"));
    }
    super::assets::assets_within(input.resources.len(), 0, 0)?;
    let root = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(stage)
        .map_err(|e| invalid(format!("Cannot open build stage: {e}")))?;
    let mut keys = BTreeSet::new();
    let mut assets = Vec::with_capacity(input.resources.len());
    let mut total = 0;
    for resource in &input.resources {
        let kind = media::asset_key(&resource.key).ok_or_else(|| invalid("Invalid packaged resource key"))?;
        if !keys.insert(resource.key.as_str()) {
            return Err(invalid("Duplicate packaged resource key"));
        }
        if resource.media_type != kind.media_type {
            return Err(invalid(format!("Resource {} has the wrong media type", resource.key)));
        }
        if matches!(resource.kind, ResourceKind::Command) != (resource.key == "commands.js") {
            return Err(invalid("Only commands.js may be a command resource"));
        }
        let file = regular_file(&root, &resource.path)?;
        let length = file.metadata().map_err(|e| invalid(format!("Resource metadata: {e}")))?.len();
        let length = usize::try_from(length).map_err(|_| invalid("Resource is too large"))?;
        total = usize::checked_add(total, length).ok_or_else(|| invalid("Resources exceed their byte limit"))?;
        super::assets::assets_within(input.resources.len(), length, total)?;
        let bytes = read(file, length)?;
        media::check_asset(kind.media_type, &bytes).map_err(Error::Rejected)?;
        super::assets::check_resource(&resource.key, &bytes, true)?;
        assets.push(BuildAsset { key: resource.key.clone(), media_type: kind.media_type, bytes });
    }
    if !keys.contains("ui.js")
        || keys.contains("ui.css") != input.roles.style.is_some()
        || keys.contains("commands.js") != input.roles.commands.is_some()
    {
        return Err(invalid("Build roles do not match the emitted programs and stylesheet"));
    }
    if let WindowFrame::Skin { skin } = &app.window().frame {
        let asset = assets
            .iter()
            .find(|asset| &asset.key == skin)
            .ok_or_else(|| invalid("The window skin is missing from the resource inventory"))?;
        if asset.media_type != "image/png" {
            return Err(invalid("The window skin must be a PNG app resource"));
        }
        crate::images::check(
            &asset.bytes,
            crate::images::Purpose::Skin { width: app.window().width, height: app.window().height },
        )
        .map_err(Error::Rejected)?;
    }
    let mut artwork = Vec::new();
    for (name, path) in [(Artwork::Preview, input.artwork.preview), (Artwork::Icon, input.artwork.icon)] {
        if let Some(path) = path {
            let file = regular_file(&root, &path)?;
            let length = file.metadata().map_err(|e| invalid(format!("Artwork metadata: {e}")))?.len();
            if length > crate::ASSET_FILE_BYTES as u64 {
                return Err(invalid("Artwork exceeds its byte limit"));
            }
            let bytes = read(file, length as usize)?;
            super::check_artwork(name, &bytes)?;
            artwork.push((name, super::optimize_png(bytes, 2)));
        }
    }
    assets.sort_by(|a, b| a.key.cmp(&b.key));
    let initial = serde_json::to_string(&input.declaration.initial).expect("JSON value");
    let checkpoint = super::pack::initial_checkpoint(app.spec(), &initial)?;
    Ok(AcceptedBuild {
        package_format: input.package_format,
        runtime_abi: input.runtime_abi,
        app,
        checkpoint,
        assets,
        artwork,
    })
}

/// Walk from a held directory descriptor. O_NOFOLLOW applies to every component,
/// including intermediate directories; a replaced path cannot redirect a later read.
fn regular_file(root: &File, path: &str) -> Result<File> {
    if !super::valid_asset_path(path) {
        return Err(invalid("Resource paths must stay beneath the build stage"));
    }
    let mut directory = root.try_clone().map_err(|e| invalid(format!("Build stage: {e}")))?;
    let mut components = path.split('/').peekable();
    while let Some(component) = components.next() {
        let name = CString::new(component).map_err(|_| invalid("Invalid resource path"))?;
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | if components.peek().is_some() { libc::O_DIRECTORY } else { 0 };
        // SAFETY: a live directory descriptor and a NUL-terminated component name.
        let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(invalid(format!(
                "Resources must be regular files beneath the stage, without symbolic links: {}",
                std::io::Error::last_os_error()
            )));
        }
        // SAFETY: openat returned a new, owned descriptor.
        directory = unsafe { File::from_raw_fd(fd) };
    }
    if !directory.metadata().map_err(|e| invalid(format!("Resource metadata: {e}")))?.is_file() {
        return Err(invalid("Resources must be regular files"));
    }
    Ok(directory)
}
fn read(file: File, length: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(length);
    file.take(length as u64 + 1).read_to_end(&mut bytes).map_err(|e| invalid(format!("Cannot read resource: {e}")))?;
    if bytes.len() != length {
        return Err(invalid("A resource changed while the build was being accepted"));
    }
    Ok(bytes)
}
