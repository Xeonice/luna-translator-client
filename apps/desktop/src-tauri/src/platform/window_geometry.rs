//! Platform geometry preferences are not business database revisions.
use app_core::contracts::window_capabilities::{GeometryProjection, GeometryState, LogicalBounds};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DisplayArea {
    pub id: String,
    pub work_area: LogicalBounds,
    pub scale_factor: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredGeometry {
    pub schema_version: u32,
    pub bounds: LogicalBounds,
    pub monitor_id: String,
    pub work_area: LogicalBounds,
    pub scale_factor: f64,
}
pub fn valid_bounds(b: &LogicalBounds) -> bool {
    [b.x, b.y, b.width, b.height]
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 1_000_000.)
        && b.width > 0.
        && b.height > 0.
}
impl StoredGeometry {
    pub fn validate(&self) -> bool {
        self.schema_version == 1
            && valid_bounds(&self.bounds)
            && valid_bounds(&self.work_area)
            && self.scale_factor.is_finite()
            && self.scale_factor > 0.
            && self.scale_factor <= 16.
            && !self.monitor_id.is_empty()
            && self.monitor_id.len() <= 512
    }
    pub fn projection(
        &self,
        state: GeometryState,
        error_code: Option<String>,
    ) -> GeometryProjection {
        GeometryProjection {
            state,
            logical_bounds: Some(self.bounds.clone()),
            monitor_id: Some(self.monitor_id.clone()),
            scale_factor: Some(self.scale_factor),
            error_code,
        }
    }
}
/// Preserve signed display origins. Oversized windows retain a reachable title/tool area.
pub fn constrain(
    bounds: &LogicalBounds,
    preferred: Option<&str>,
    displays: &[DisplayArea],
) -> Option<StoredGeometry> {
    if !valid_bounds(bounds) {
        return None;
    }
    let displays: Vec<_> = displays
        .iter()
        .filter(|d| valid_bounds(&d.work_area) && d.scale_factor.is_finite() && d.scale_factor > 0.)
        .collect();
    let display = displays
        .iter()
        .find(|d| Some(d.id.as_str()) == preferred)
        .copied()
        .or_else(|| {
            displays
                .iter()
                .find(|d| {
                    let a = &d.work_area;
                    bounds.x + bounds.width / 2. >= a.x
                        && bounds.x + bounds.width / 2. < a.x + a.width
                        && bounds.y >= a.y
                        && bounds.y < a.y + a.height
                })
                .copied()
        })
        .or_else(|| displays.first().copied())?;
    let a = &display.work_area;
    let width = bounds.width.min(a.width).max(1.);
    let height = bounds.height.min(a.height).max(1.);
    let x = bounds.x.clamp(a.x, (a.x + a.width - width).max(a.x));
    let y = bounds.y.clamp(a.y, (a.y + a.height - height).max(a.y));
    Some(StoredGeometry {
        schema_version: 1,
        bounds: LogicalBounds {
            x,
            y,
            width,
            height,
        },
        monitor_id: display.id.clone(),
        work_area: a.clone(),
        scale_factor: display.scale_factor,
    })
}
pub struct GeometryFile {
    pub path: PathBuf,
}
impl GeometryFile {
    pub fn new(root: &Path) -> Self {
        Self {
            path: root.join("window-geometry-v1.json"),
        }
    }
    pub fn load(&self) -> Result<Option<StoredGeometry>, String> {
        let bytes = match fs::read(&self.path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("geometry_read_failed".into()),
        };
        if bytes.len() > 16_384 {
            return Err("geometry_invalid".into());
        }
        let geometry: StoredGeometry =
            serde_json::from_slice(&bytes).map_err(|_| "geometry_invalid")?;
        if !geometry.validate() {
            return Err("geometry_invalid".into());
        }
        Ok(Some(geometry))
    }
    pub fn save(&self, value: &StoredGeometry) -> Result<(), String> {
        if !value.validate() {
            return Err("geometry_invalid".into());
        }
        let parent = self.path.parent().ok_or("geometry_path")?;
        fs::create_dir_all(parent).map_err(|_| "geometry_write_failed")?;
        let temp = parent.join(format!(
            ".window-geometry-{}-{}.pending",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(|_| "geometry_write_failed")?;
            file.write_all(&serde_json::to_vec(value).map_err(|_| "geometry_encoding")?)
                .and_then(|_| file.sync_all())
                .map_err(|_| "geometry_write_failed")?;
            fs::rename(&temp, &self.path).map_err(|_| "geometry_write_failed")?;
            #[cfg(unix)]
            fs::File::open(parent)
                .and_then(|dir| dir.sync_all())
                .map_err(|_| "geometry_sync_failed")?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result.map_err(str::to_owned)
    }
    pub fn preserve_invalid(&self) -> Result<(), String> {
        if self.path.exists() {
            let target = self.path.with_extension(format!(
                "invalid-{}-{}.json",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            fs::copy(&self.path, target).map_err(|_| "geometry_backup_failed")?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn bounds(x: f64, y: f64, w: f64, h: f64) -> LogicalBounds {
        LogicalBounds {
            x,
            y,
            width: w,
            height: h,
        }
    }
    fn screen(id: &str, x: f64, scale: f64) -> DisplayArea {
        DisplayArea {
            id: id.into(),
            work_area: bounds(x, 24., 1440., 876.),
            scale_factor: scale,
        }
    }
    #[test]
    fn negative_monitor_and_removed_screen_remain_reachable() {
        let left = screen("left", -1440., 2.);
        let main = screen("main", 0., 1.);
        let restored = constrain(
            &bounds(-1300., 200., 560., 280.),
            Some("left"),
            &[left.clone(), main.clone()],
        )
        .unwrap();
        assert_eq!(restored.bounds.x, -1300.);
        let removed = constrain(&restored.bounds, Some("left"), &[main]).unwrap();
        assert_eq!(removed.bounds.x, 0.);
        assert_eq!(removed.monitor_id, "main");
    }
    #[test]
    fn scale_changes_and_oversize_are_logical_not_zero_origin() {
        let value = constrain(
            &bounds(-1900., -500., 3000., 1600.),
            Some("left"),
            &[screen("left", -1440., 1.5)],
        )
        .unwrap();
        assert_eq!(value.bounds, bounds(-1440., 24., 1440., 876.));
        assert_eq!(value.scale_factor, 1.5);
        assert!(
            constrain(
                &bounds(f64::NAN, 0., 560., 280.),
                None,
                &[screen("main", 0., 2.)]
            )
            .is_none()
        );
    }
    #[test]
    fn atomic_file_roundtrip_and_invalid_original_preserved() {
        let root = std::env::temp_dir().join(format!(
            "luna-geometry-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let file = GeometryFile::new(&root);
        let g = constrain(
            &bounds(200., 300., 560., 280.),
            None,
            &[screen("main", 0., 2.)],
        )
        .unwrap();
        file.save(&g).unwrap();
        assert_eq!(file.load().unwrap().unwrap().bounds, g.bounds);
        fs::write(&file.path, b"{\"schemaVersion\":99}").unwrap();
        assert!(file.load().is_err());
        file.preserve_invalid().unwrap();
        assert_eq!(fs::read(&file.path).unwrap(), b"{\"schemaVersion\":99}");
        assert_eq!(
            fs::read_dir(&root)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|e| e.file_name().to_string_lossy().contains("pending"))
                .count(),
            0
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn write_failure_does_not_destroy_previous_geometry() {
        let root = std::env::temp_dir().join(format!(
            "luna-geometry-failure-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let parent = root.join("not-a-directory");
        fs::write(&parent, b"sentinel").unwrap();
        let file = GeometryFile {
            path: parent.join("geometry.json"),
        };
        let g = constrain(&bounds(0., 0., 560., 280.), None, &[screen("main", 0., 2.)]).unwrap();
        assert!(file.save(&g).is_err());
        assert_eq!(fs::read(&parent).unwrap(), b"sentinel");
        fs::remove_dir_all(root).unwrap();
    }
}
