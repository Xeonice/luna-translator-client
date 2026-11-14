//! Bounded local image discovery and profile-owned, content-addressed JPEG files.
use app_core::{contracts::*, domain::content_hash, ports::GameCoverAssets};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
pub const SOURCE_LIMIT: usize = 8 * 1024 * 1024;
pub const COVER_LIMIT: usize = 524_288;
const STORE_LIMIT: u64 = 256 * 1024 * 1024;
pub struct NormalizedCover {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}
/// Injected OS codec; the core and this adapter do not depend on Tauri/ImageIO.
pub trait CoverImageCodec: Send + Sync {
    fn normalize(&self, source: &[u8]) -> AppResult<NormalizedCover>;
    fn dimensions(&self, jpeg: &[u8]) -> AppResult<(u32, u32)>;
}
fn error(code: &str) -> AppError {
    AppError::new(code, "library", "本机封面无法读取或保存，取文不受影响")
}
pub struct ManagedGameAssets {
    root: PathBuf,
    codec: Arc<dyn CoverImageCodec>,
    writes: Mutex<()>,
}
impl ManagedGameAssets {
    pub fn new(profile_root: PathBuf, codec: Arc<dyn CoverImageCodec>) -> AppResult<Self> {
        if std::fs::symlink_metadata(&profile_root).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(error("cover_store_symlink"));
        }
        std::fs::create_dir_all(&profile_root).map_err(|_| error("cover_store_unavailable"))?;
        let profile = profile_root
            .canonicalize()
            .map_err(|_| error("cover_store_unavailable"))?;
        let root = profile.join("assets/covers/v1");
        for part in [
            profile.join("assets"),
            profile.join("assets/covers"),
            root.clone(),
        ] {
            if std::fs::symlink_metadata(&part).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(error("cover_store_symlink"));
            }
            std::fs::create_dir_all(&part).map_err(|_| error("cover_store_unavailable"))?;
        }
        if !root
            .canonicalize()
            .map_err(|_| error("cover_store_unavailable"))?
            .starts_with(&profile)
        {
            return Err(error("cover_store_symlink"));
        }
        Ok(Self {
            root,
            codec,
            writes: Mutex::new(()),
        })
    }
    fn persist(&self, bytes: &[u8], origin: GameCoverOrigin) -> AppResult<PreparedGameCover> {
        let image = self.codec.normalize(bytes)?;
        if image.bytes.is_empty()
            || image.bytes.len() > COVER_LIMIT
            || image.width < 160
            || image.height < 90
            || image.width > 960
            || image.height > 960
        {
            return Err(error("cover_image_bounds"));
        }
        let hash = content_hash(&image.bytes);
        let target = self.root.join(format!("{hash}.jpg"));
        let _guard = self.writes.lock().map_err(|_| error("cover_store_state"))?;
        if target.exists() {
            let existing = bounded_read(&target, &self.root, COVER_LIMIT)?;
            if content_hash(&existing) != hash {
                return Err(error("cover_hash_mismatch"));
            }
        } else {
            let mut total = 0u64;
            for (count, entry) in std::fs::read_dir(&self.root)
                .map_err(|_| error("cover_store_unavailable"))?
                .enumerate()
            {
                if count >= 4096 {
                    return Err(error("cover_store_limit"));
                }
                let meta = entry
                    .map_err(|_| error("cover_store_unavailable"))?
                    .metadata()
                    .map_err(|_| error("cover_store_unavailable"))?;
                total = total
                    .checked_add(meta.len())
                    .ok_or_else(|| error("cover_store_limit"))?;
            }
            if total.saturating_add(image.bytes.len() as u64) > STORE_LIMIT {
                return Err(error("cover_store_limit"));
            }
            let pending = self.root.join(format!("{}.pending", uuid::Uuid::new_v4()));
            let written = (|| {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&pending)
                    .map_err(|_| error("cover_write_failed"))?;
                file.write_all(&image.bytes)
                    .and_then(|_| file.sync_all())
                    .map_err(|_| error("cover_write_failed"))?;
                // hard_link is atomic and never replaces an existing content-addressed file.
                std::fs::hard_link(&pending, &target).map_err(|_| error("cover_write_failed"))?;
                Ok(())
            })();
            let _ = std::fs::remove_file(pending);
            written?;
        }
        Ok(PreparedGameCover {
            asset_ref: format!("cover:v1:{}:{hash}", origin.key()),
            origin,
            content_hash: hash,
            width: image.width,
            height: image.height,
        })
    }
}
impl GameCoverAssets for ManagedGameAssets {
    fn import_local_cover(&self, game: &Game) -> AppResult<Option<PreparedGameCover>> {
        let executable = match super::game_paths::executable(game) {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };
        for (path, allowed, origin) in
            local_candidates(&executable, game.target.bottle_locator.as_deref())?
        {
            let Ok(bytes) = bounded_read(&path, &allowed, SOURCE_LIMIT) else {
                continue;
            };
            match self.persist(&bytes, origin) {
                Ok(cover) => return Ok(Some(cover)),
                Err(e)
                    if matches!(
                        e.code.as_str(),
                        "cover_image_invalid" | "cover_image_bounds" | "cover_image_format"
                    ) =>
                {
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(None)
    }
    fn import_session_frame(&self, jpeg: &[u8]) -> AppResult<PreparedGameCover> {
        if jpeg.is_empty() || jpeg.len() > COVER_LIMIT {
            return Err(error("cover_image_bounds"));
        }
        self.persist(jpeg, GameCoverOrigin::SessionFrame)
    }
    fn read_cover(&self, reference: &str) -> AppResult<GameCoverBytes> {
        let (origin, hash) = parse_reference(reference)?;
        let bytes = bounded_read(
            &self.root.join(format!("{hash}.jpg")),
            &self.root,
            COVER_LIMIT,
        )?;
        if content_hash(&bytes) != hash {
            return Err(error("cover_hash_mismatch"));
        }
        let (width, height) = self.codec.dimensions(&bytes)?;
        if width < 160 || height < 90 || width > 960 || height > 960 {
            return Err(error("cover_image_bounds"));
        }
        Ok(GameCoverBytes {
            prepared: PreparedGameCover {
                asset_ref: reference.into(),
                origin,
                content_hash: hash.into(),
                width,
                height,
            },
            bytes,
        })
    }
}
pub fn parse_reference(reference: &str) -> AppResult<(GameCoverOrigin, &str)> {
    let values: Vec<_> = reference.split(':').collect();
    if values.len() != 4
        || values[0] != "cover"
        || values[1] != "v1"
        || values[3].len() != 64
        || !values[3]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(error("cover_reference_invalid"));
    }
    let origin = match values[2] {
        "steam_library" => GameCoverOrigin::SteamLibrary,
        "game_resource" => GameCoverOrigin::GameResource,
        "session_frame" => GameCoverOrigin::SessionFrame,
        _ => return Err(error("cover_reference_invalid")),
    };
    Ok((origin, values[3]))
}
fn bounded_read(path: &Path, allowed: &Path, limit: usize) -> AppResult<Vec<u8>> {
    let root = allowed
        .canonicalize()
        .map_err(|_| error("cover_source_unavailable"))?;
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| error("cover_source_unavailable"))?;
    if !metadata.file_type().is_file() || metadata.len() == 0 || metadata.len() > limit as u64 {
        return Err(error("cover_source_bounds"));
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| error("cover_source_unavailable"))?;
    if !canonical.starts_with(&root) {
        return Err(error("cover_source_escape"));
    }
    let file = File::open(canonical).map_err(|_| error("cover_source_unavailable"))?;
    if !file
        .metadata()
        .map_err(|_| error("cover_source_unavailable"))?
        .is_file()
    {
        return Err(error("cover_source_bounds"));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| error("cover_source_unavailable"))?;
    if bytes.is_empty() || bytes.len() > limit {
        return Err(error("cover_source_bounds"));
    }
    Ok(bytes)
}
fn local_candidates(
    executable: &Path,
    bottle: Option<&str>,
) -> AppResult<Vec<(PathBuf, PathBuf, GameCoverOrigin)>> {
    let mut out = Vec::new();
    // The directory identity and manifest establish AppID, never display name or EXE basename.
    for install in executable.ancestors().skip(1).take(10) {
        let Some(common) = install.parent() else {
            continue;
        };
        if !common
            .file_name()
            .is_some_and(|v| v.eq_ignore_ascii_case("common"))
        {
            continue;
        }
        let Some(steamapps) = common.parent().filter(|p| {
            p.file_name()
                .is_some_and(|v| v.eq_ignore_ascii_case("steamapps"))
        }) else {
            continue;
        };
        let install_name = install
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let mut appid = None;
        for entry in std::fs::read_dir(steamapps)
            .map_err(|_| error("cover_steam_unavailable"))?
            .take(512)
        {
            let entry = entry.map_err(|_| error("cover_steam_unavailable"))?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("appmanifest_") || !name.ends_with(".acf") {
                continue;
            }
            let Ok(bytes) = bounded_read(&entry.path(), steamapps, 65_536) else {
                continue;
            };
            let Ok(fields) = manifest_fields(&bytes) else {
                continue;
            };
            if fields
                .get("installdir")
                .is_some_and(|v| v.eq_ignore_ascii_case(install_name))
                && let Some(id) = fields.get("appid").filter(|v| {
                    !v.is_empty() && v.len() <= 12 && v.bytes().all(|b| b.is_ascii_digit())
                })
            {
                if name != format!("appmanifest_{id}.acf") {
                    continue;
                }
                if appid.is_some() {
                    return Err(error("cover_steam_identity_ambiguous"));
                }
                appid = Some(id.clone());
            }
        }
        if let Some(id) = appid {
            let cache = steamapps
                .parent()
                .unwrap_or(steamapps)
                .join("appcache/librarycache");
            append_steam_candidates(&mut out, &cache, &id);
            // External Steam library manifests live with the game, while downloaded art
            // remains under the same verified Bottle's Steam installation.
            if let Some(bottle) = bottle {
                for folder in ["Program Files (x86)", "Program Files"] {
                    let installed_cache = Path::new(bottle)
                        .join("drive_c")
                        .join(folder)
                        .join("Steam/appcache/librarycache");
                    if installed_cache != cache {
                        append_steam_candidates(&mut out, &installed_cache, &id);
                    }
                }
            }
        }
        break;
    }
    if let Some(parent) = executable.parent() {
        for folder in [parent.to_path_buf(), parent.join("assets")] {
            for stem in ["cover", "folder", "boxart", "header"] {
                for extension in ["jpg", "png", "jpeg"] {
                    out.push((
                        folder.join(format!("{stem}.{extension}")),
                        parent.to_path_buf(),
                        GameCoverOrigin::GameResource,
                    ));
                }
            }
        }
    }
    out.truncate(96);
    Ok(out)
}
fn append_steam_candidates(
    out: &mut Vec<(PathBuf, PathBuf, GameCoverOrigin)>,
    cache: &Path,
    id: &str,
) {
    let allowed = cache
        .parent()
        .and_then(Path::parent)
        .unwrap_or(cache)
        .to_path_buf();
    for name in [
        "library_600x900.jpg",
        "library_600x900_schinese.jpg",
        "library_600x900_japanese.jpg",
        "library_600x900_english.jpg",
        "header.jpg",
        "header_schinese.jpg",
    ] {
        out.push((
            cache.join(id).join(name),
            allowed.clone(),
            GameCoverOrigin::SteamLibrary,
        ));
    }
    for suffix in [
        "library_600x900.jpg",
        "library_600x900_schinese.jpg",
        "header.jpg",
    ] {
        out.push((
            cache.join(format!("{id}_{suffix}")),
            allowed.clone(),
            GameCoverOrigin::SteamLibrary,
        ));
    }
}
/// Bounded quoted KeyValues parser, retaining only direct AppState fields.
pub fn manifest_fields(bytes: &[u8]) -> AppResult<BTreeMap<String, String>> {
    if bytes.len() > 65_536 {
        return Err(error("cover_manifest_invalid"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| error("cover_manifest_invalid"))?;
    let mut chars = text.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            for value in chars.by_ref() {
                if value == '\n' {
                    break;
                }
            }
            continue;
        }
        if c == '{' || c == '}' {
            tokens.push(c.to_string());
        } else if c == '"' {
            let mut value = String::new();
            let mut closed = false;
            while let Some(c) = chars.next() {
                if c == '"' {
                    closed = true;
                    break;
                }
                if c == '\\' {
                    value.push(
                        chars
                            .next()
                            .ok_or_else(|| error("cover_manifest_invalid"))?,
                    );
                } else {
                    value.push(c);
                }
                if value.len() > 4096 {
                    return Err(error("cover_manifest_invalid"));
                }
            }
            if !closed || value.contains('\0') {
                return Err(error("cover_manifest_invalid"));
            }
            tokens.push(value);
        } else {
            return Err(error("cover_manifest_invalid"));
        }
        if tokens.len() > 4096 {
            return Err(error("cover_manifest_invalid"));
        }
    }
    if tokens.first().map(String::as_str) != Some("AppState")
        || tokens.get(1).map(String::as_str) != Some("{")
    {
        return Err(error("cover_manifest_invalid"));
    }
    let mut fields = BTreeMap::new();
    let mut depth: u32 = 0;
    let mut index = 1;
    while index < tokens.len() {
        match tokens[index].as_str() {
            "{" => {
                depth += 1;
                if depth > 16 {
                    return Err(error("cover_manifest_invalid"));
                }
                index += 1;
            }
            "}" => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| error("cover_manifest_invalid"))?;
                index += 1;
                if depth == 0 && index != tokens.len() {
                    return Err(error("cover_manifest_invalid"));
                }
            }
            key => {
                let value = tokens
                    .get(index + 1)
                    .ok_or_else(|| error("cover_manifest_invalid"))?;
                if value == "{" {
                    index += 1;
                    continue;
                }
                if value == "}" {
                    return Err(error("cover_manifest_invalid"));
                }
                if depth == 1
                    && ["appid", "installdir"].contains(&key)
                    && fields.insert(key.into(), value.clone()).is_some()
                {
                    return Err(error("cover_manifest_invalid"));
                }
                index += 2;
            }
        }
    }
    if depth != 0 {
        return Err(error("cover_manifest_invalid"));
    }
    Ok(fields)
}
