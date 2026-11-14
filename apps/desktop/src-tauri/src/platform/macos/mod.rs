//! Narrow native calls. Buffers are copied before native ownership is released.
use app_core::contracts::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::OnceLock;
mod window_capture;

static QUIT_APPLICATION: OnceLock<tauri::AppHandle> = OnceLock::new();
static GEOMETRY_APPLICATION: OnceLock<tauri::AppHandle> = OnceLock::new();

#[cfg(test)]
unsafe extern "C" {
    fn luna_preview_owner_maps_file(pid: i32, image_fd: i32) -> i32;
    fn luna_preview_bottle_identity_for_test(path: *const c_char) -> *mut c_char;
}

unsafe extern "C" {
    fn luna_geometry_initialize(callback: extern "C" fn(u64, i32)) -> i32;
    fn luna_geometry_defer(callback: extern "C" fn(*mut c_void), context: *mut c_void) -> i32;
    fn luna_owner_geometry(
        window: *mut c_void,
        result: *mut super::window_manager::PhysicalGeometry,
    ) -> i32;
    fn luna_hud_content_fit(
        window: *mut c_void,
        before: *const super::window_manager::PhysicalGeometry,
        height: u32,
        after: *mut super::window_manager::PhysicalGeometry,
    ) -> i32;
    fn luna_native_free(value: *mut c_void);
    fn luna_choose_export_path(default_name: *const c_char) -> *mut c_char;
    fn luna_write_clipboard(bytes: *const u8, length: usize) -> i32;
    fn luna_preview_supported() -> i32;
    fn luna_preview_permission(request: i32) -> i32;
    fn luna_reduce_transparency() -> i32;
    fn luna_hud_material(window: *mut c_void, reduced: i32) -> i32;
    fn luna_screens_json() -> *mut c_char;
    fn luna_resolve_window_nonce(nonce: *const c_char) -> *mut c_char;
    fn luna_capture_busy() -> i32;
    fn luna_capture_window(
        window: u32,
        host_pid: i32,
        host_start: *const c_char,
        bytes: *mut *mut u8,
        length: *mut usize,
        width: *mut u32,
        height: *mut u32,
        captured_at: *mut u64,
    ) -> i32;
    fn luna_hotkey_initialize(callback: extern "C" fn(u32, i32)) -> i32;
    fn luna_hotkey_register(id: u32, key: u32, modifiers: u32) -> i32;
    fn luna_hotkey_unregister(id: u32) -> i32;
    fn luna_quit_initialize(callback: extern "C" fn()) -> i32;
    #[cfg(test)]
    fn luna_quit_reply(allow: i32) -> i32;
    fn luna_quit_reply_async(
        allow: i32,
        completion: extern "C" fn(*mut c_void, i32),
        context: *mut c_void,
    ) -> i32;
}
extern "C" fn geometry_edge(epoch: u64, edge: i32) {
    use tauri::Manager;
    if let Some(app) = GEOMETRY_APPLICATION.get()
        && let Some(coordinator) =
            app.try_state::<std::sync::Arc<super::window_manager::WindowCoordinator>>()
    {
        coordinator.native_geometry_edge(epoch, edge);
    }
}
pub fn initialize_geometry(app: tauri::AppHandle) {
    GEOMETRY_APPLICATION.get_or_init(|| app);
    // SAFETY: Function pointer and AppHandle have process lifetime. This only
    // installs a callback, never replaces a framework/window delegate.
    let _ = unsafe { luna_geometry_initialize(geometry_edge) };
}
type DeferredGeometry = Box<dyn FnOnce() + Send>;
extern "C" fn complete_geometry(context: *mut c_void) {
    // SAFETY: The accepted RunLoop block returns this exact owned Box once.
    let callback = unsafe { Box::from_raw(context.cast::<DeferredGeometry>()) };
    callback();
}
pub fn defer_geometry(callback: impl FnOnce() + Send + 'static) -> AppResult<()> {
    let context = Box::into_raw(Box::new(Box::new(callback) as DeferredGeometry));
    // SAFETY: Native takes ownership only on status=0; no synchronous callback.
    if unsafe { luna_geometry_defer(complete_geometry, context.cast()) } != 0 {
        // SAFETY: Rejection leaves ownership with this caller; native code has
        // neither queued nor invoked the callback for this exact allocation.
        drop(unsafe { Box::from_raw(context) });
        return Err(error("geometry_dispatch_failed"));
    }
    Ok(())
}
pub fn owner_geometry(
    window: &tauri::WebviewWindow,
) -> AppResult<super::window_manager::PhysicalGeometry> {
    let raw = window
        .ns_window()
        .map_err(|_| error("geometry_read_failed"))?;
    let mut result = super::window_manager::PhysicalGeometry::default();
    // SAFETY: The bridge marshals a read of this live window to AppKit main.
    if unsafe { luna_owner_geometry(raw, &mut result) } != 0 {
        return Err(error("geometry_read_failed"));
    }
    Ok(result)
}
pub fn event_owner_geometry(
    window: &tauri::Window,
) -> AppResult<super::window_manager::PhysicalGeometry> {
    let raw = window
        .ns_window()
        .map_err(|_| error("geometry_read_failed"))?;
    let mut result = super::window_manager::PhysicalGeometry::default();
    // SAFETY: Read the exact event window, never resolve its reusable label to
    // a possibly newer NSWindow instance.
    if unsafe { luna_owner_geometry(raw, &mut result) } != 0 {
        return Err(error("geometry_read_failed"));
    }
    Ok(result)
}
pub fn apply_hud_content_fit(
    window: &tauri::WebviewWindow,
    before: &super::window_manager::PhysicalGeometry,
    height: u32,
) -> AppResult<super::window_manager::PhysicalGeometry> {
    let raw = window
        .ns_window()
        .map_err(|_| error("geometry_read_failed"))?;
    let mut after = super::window_manager::PhysicalGeometry::default();
    // SAFETY: Called only in an independent main RunLoop transaction; no Rust
    // lifecycle/card lock crosses native setFrame and its reentrant events.
    if unsafe { luna_hud_content_fit(raw, before, height, &mut after) } != 0 {
        return Err(error("hud_fit_not_applied"));
    }
    Ok(after)
}
struct QuitReplyContext {
    after_reply: Box<dyn FnOnce() + Send>,
    sender: tauri::async_runtime::Sender<i32>,
}
extern "C" fn complete_quit_reply(context: *mut c_void, status: i32) {
    // SAFETY: An accepted native dispatch calls this once on main and transfers
    // its exact Box allocation back. A rejected dispatch is completed directly
    // by the Rust owner instead and never enters the native queue.
    let context = unsafe { Box::from_raw(context.cast::<QuitReplyContext>()) };
    (context.after_reply)();
    let _ = context.sender.try_send(status);
}
extern "C" fn request_quit() {
    if let Some(app) = QUIT_APPLICATION.get() {
        // AppHandle only posts the normal RequestExit event. All durable work
        // remains in main's single asynchronous shutdown coordinator.
        app.exit(0);
    }
}
pub fn initialize_quit(app: tauri::AppHandle) -> AppResult<()> {
    // SAFETY: The bridge rejects non-main callers and existing delegate
    // selectors. The C callback and AppHandle both have process lifetime.
    let status = unsafe { luna_quit_initialize(request_quit) };
    if status != 0 {
        return Err(AppError::new(
            "native_quit_initialize",
            "desktop",
            format!("无法安全安装系统退出协调（{status}），未覆盖已有应用代理"),
        ));
    }
    QUIT_APPLICATION.get_or_init(|| app);
    Ok(())
}
pub async fn reply_pending_quit<F>(
    app: &tauri::AppHandle,
    allow: bool,
    after_reply: F,
) -> AppResult<bool>
where
    F: FnOnce() + Send + 'static,
{
    // A capacity-one, single-use channel is available through Tauri's existing
    // runtime. Neither this future nor the AppKit callback blocks its thread.
    let (sender, mut receiver) = tauri::async_runtime::channel(1);
    let context = Box::into_raw(Box::new(QuitReplyContext {
        after_reply: Box::new(after_reply),
        sender,
    }));
    // A raw pointer cannot cross a Send closure. This address names one owned
    // allocation; only the accepted callback or an explicit rejection frees it.
    let address = context as usize;
    if app
        .run_on_main_thread(move || {
            // SAFETY: This Task only schedules a separate CFRunLoop block. It
            // does not call NSApp replyYES while holding Tao's callback lock.
            let status = unsafe {
                luna_quit_reply_async(
                    i32::from(allow),
                    complete_quit_reply,
                    address as *mut c_void,
                )
            };
            if status != 0 {
                // The native queue did not take ownership on rejection.
                complete_quit_reply(address as *mut c_void, -1);
            }
        })
        .is_err()
    {
        // SAFETY: run_on_main_thread rejected its Task, so native code has
        // never seen this allocation and cannot complete it later.
        drop(unsafe { Box::from_raw(context) });
        return Err(error("native_quit_reply_unavailable"));
    }
    match receiver.recv().await {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => Err(error("native_quit_reply_unavailable")),
    }
}
fn error(code: &str) -> AppError {
    AppError::new(code, "desktop", "原生平台能力暂不可用，请重试或查看权限")
}
fn json<T: for<'de> Deserialize<'de>>(raw: *mut c_char) -> AppResult<T> {
    if raw.is_null() {
        return Err(error("native_response"));
    }
    // SAFETY: Only the bridge's allocated NUL-terminated JSON is passed here.
    let result = unsafe { serde_json::from_slice(CStr::from_ptr(raw).to_bytes()) };
    // SAFETY: The bridge owns malloc allocation; this exact pointer is freed once.
    unsafe { luna_native_free(raw.cast()) };
    result.map_err(|_| error("native_response"))
}
pub fn preview_supported() -> bool {
    // SAFETY: Availability-only bridge query, no pointers or capture side effects.
    unsafe { luna_preview_supported() != 0 }
}
pub fn preview_permission(request: bool) -> bool {
    // SAFETY: User initiation is enforced by the caller before request=true.
    unsafe { luna_preview_permission(i32::from(request)) != 0 }
}
pub fn reduce_transparency() -> bool {
    // SAFETY: Read-only NSWorkspace query is marshalled onto the main thread.
    unsafe { luna_reduce_transparency() != 0 }
}
pub fn set_hud_material(window: &tauri::WebviewWindow, reduced: bool) -> AppResult<bool> {
    let raw = window
        .ns_window()
        .map_err(|_| error("native_material_unavailable"))?;
    // SAFETY: The live Tauri window owns this NSWindow throughout the call. The
    // bridge synchronously marshals AppKit mutation and verification to main.
    let status = unsafe { luna_hud_material(raw, i32::from(reduced)) };
    if status < 0 {
        Err(error("native_material_unavailable"))
    } else {
        Ok(status == 1)
    }
}
pub fn screens() -> AppResult<Vec<super::window_geometry::DisplayArea>> {
    // SAFETY: The bridge returns one independently owned JSON allocation.
    json(unsafe { luna_screens_json() })
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeWindowIdentity {
    pub host_pid: u32,
    pub host_process_start: String,
    pub window_id: u32,
    pub on_screen: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CaptureFailureKind {
    CompatibilitySourceCap,
    GeometryUnavailable,
    OutputSafetyBudget,
    OtherExistingFailure,
}
/// Same-filter numerical facts only. These values never grant capture authority.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreviewGeometrySnapshot {
    pub cg_width_points: Option<f64>,
    pub cg_height_points: Option<f64>,
    pub cg_on_screen: Option<bool>,
    pub filter_width_points: Option<f64>,
    pub filter_height_points: Option<f64>,
    pub point_pixel_scale: Option<f64>,
    pub request_width: Option<u32>,
    pub request_height: Option<u32>,
    pub compatibility_source_cap: Option<bool>,
    pub reason: Option<String>,
    pub sc_window_frame_query: String,
    pub sc_window_width_points: Option<f64>,
    pub sc_window_height_points: Option<f64>,
}
impl PreviewGeometrySnapshot {
    fn checked(&self) -> bool {
        let finite = [
            self.cg_width_points,
            self.cg_height_points,
            self.filter_width_points,
            self.filter_height_points,
            self.point_pixel_scale,
            self.sc_window_width_points,
            self.sc_window_height_points,
        ]
        .into_iter()
        .flatten()
        .all(|value| value.is_finite() && value > 0.0);
        finite
            && matches!(
                self.sc_window_frame_query.as_str(),
                "queried" | "not_queried"
            )
            && (self.sc_window_frame_query != "not_queried"
                || (self.sc_window_width_points.is_none()
                    && self.sc_window_height_points.is_none()))
            && self.reason.as_deref().is_none_or(|reason| {
                matches!(
                    reason,
                    "cg_missing"
                        | "cg_dimensions"
                        | "filter_dimensions"
                        | "point_pixel_scale"
                        | "pixel_overflow"
                        | "request_unavailable"
                )
            })
            && self
                .request_width
                .is_none_or(|value| (1..=960).contains(&value))
            && self
                .request_height
                .is_none_or(|value| (1..=540).contains(&value))
    }
    pub(crate) fn signature(&self) -> Option<[u64; 8]> {
        if !self.checked() || self.reason.is_some() {
            return None;
        }
        let scale = self.point_pixel_scale?;
        let pixel = |point: Option<f64>| {
            let value = (point? * scale).ceil();
            // u64::MAX rounds up to 2^64 as f64. Reject that boundary before
            // casting, rather than accepting Rust's saturating float cast.
            (value.is_finite() && value >= 1.0 && value < 18_446_744_073_709_551_616.0)
                .then_some(value as u64)
        };
        Some([
            pixel(self.cg_width_points)?,
            pixel(self.cg_height_points)?,
            pixel(self.filter_width_points)?,
            pixel(self.filter_height_points)?,
            u64::from(self.request_width?),
            u64::from(self.request_height?),
            u64::from(self.cg_on_screen?),
            u64::from(self.compatibility_source_cap?),
        ])
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreviewRecoveryDiagnostic {
    pub state: String,
    pub credits: u8,
    pub normal_trigger_without_recovery_credit: bool,
    pub samples: u32,
    pub generation: u64,
    pub cache_entries: u8,
    pub metadata_only: bool,
    pub admission: bool,
}
/// Private, fixed-vocabulary metadata. It carries no image, capability, path,
/// title or free-form operating-system error text and never grants authority.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct PreviewDiagnostic {
    pub stage: String,
    pub operation: u64,
    pub revision: u64,
    pub request: u64,
    pub picker_operation: u64,
    pub picker_stage: Option<String>,
    pub received: u32,
    pub processed: u32,
    pub stream: bool,
    pub current: bool,
    pub timeout_source: Option<String>,
    pub reason: Option<String>,
    pub native_status: i32,
    pub sc_domain: Option<String>,
    pub sc_code: Option<i64>,
    pub api: Option<String>,
    pub configured_width: u32,
    pub configured_height: u32,
    pub actual_width: u64,
    pub actual_height: u64,
    pub encoded_bytes: u64,
    // Native API facts above remain intact when the compatibility adapter has
    // a different final result. These fields are private, fixed-vocabulary only.
    pub final_stage: Option<String>,
    pub final_code: Option<String>,
    pub geometry: Option<PreviewGeometrySnapshot>,
    pub native_geometry: Option<PreviewGeometrySnapshot>,
    pub failure_kind: Option<CaptureFailureKind>,
    pub post_proof: Option<String>,
    pub recovery: Option<PreviewRecoveryDiagnostic>,
}
const PREVIEW_FINAL_STAGES: &[&str] = &[
    "native_capture",
    "compatibility_bounds",
    "compatibility_capture",
    "worker_validation",
];
const PREVIEW_FINAL_CODES: &[&str] = &[
    "ok",
    "preview_unsupported",
    "preview_permission_denied",
    "preview_permission_revoked",
    "preview_target_closed",
    "preview_target_changed",
    "preview_window_not_on_screen",
    "preview_budget_exceeded",
    "preview_capture_busy",
    "preview_capture_timeout",
    "preview_capture_failed",
    "preview_selection_expired",
    "preview_selection_mismatch",
    "preview_selection_required",
    "preview_selection_cancelled",
    "preview_selection_timeout",
    "preview_selection_failed",
    "preview_encoding_failed",
    "preview_source_changed",
    "preview_cancelled",
    "preview_verification_unavailable",
    "preview_geometry_unavailable",
];
impl PreviewDiagnostic {
    pub(crate) fn worker(stage: &str) -> Self {
        Self {
            stage: stage.into(),
            ..Self::default()
        }
    }
    pub(crate) fn checked(self) -> Option<Self> {
        const STAGES: &[&str] = &[
            "picker_accepted",
            "picker_presented",
            "picker_update_received",
            "picker_cancel_received",
            "picker_start_failed_received",
            "picker_update_processed",
            "picker_cancel_processed",
            "picker_start_failed_processed",
            "picker_finished",
            "picker_native_timeout",
            "picker_rust_timeout",
            "picker_wait",
            "win_pre",
            "owner_pre",
            "capture",
            "capture_complete",
            "capture_timeout",
            "win_post",
            "owner_post",
            "frame_published",
            "frame_read",
            "frame_write",
            "frame_rejected",
            "geometry_observation",
        ];
        const REASONS: &[&str] = &[
            "sc_error",
            "image_missing",
            "image_dimensions",
            "jpeg_bytes",
            "jpeg_finalize",
            "jpeg_destination",
            "target_closed",
            "window_not_on_screen",
            "permission_revoked",
            "capture_busy",
            "capture_timeout",
            "capture_failed",
            "preview_encoding_failed",
            "preview_budget_exceeded",
            "sc_compatibility_failed",
            "selection_expired",
            "frame_guard",
            "frame_transport",
            "geometry_unavailable",
            "allocation_failed",
        ];
        if !STAGES.contains(&self.stage.as_str())
            || self.geometry.as_ref().is_some_and(|value| !value.checked())
            || self
                .native_geometry
                .as_ref()
                .is_some_and(|value| !value.checked())
            || match self.failure_kind {
                Some(CaptureFailureKind::CompatibilitySourceCap) => !self
                    .geometry
                    .as_ref()
                    .and_then(PreviewGeometrySnapshot::signature)
                    .is_some_and(|key| {
                        key[7] == 1
                            && (key[0].max(key[2]) > 4_194_304
                                || key[1].max(key[3]) > 4_194_304
                                || key[0].max(key[2]) > 4_194_304 / key[1].max(key[3]))
                    }),
                Some(CaptureFailureKind::GeometryUnavailable) => {
                    !self.geometry.as_ref().is_some_and(|geometry| {
                        geometry.reason.is_some() && geometry.signature().is_none()
                    })
                }
                _ => false,
            }
            || (self.final_stage.as_deref() == Some("compatibility_bounds")
                && self.final_code.as_deref() == Some("preview_budget_exceeded")
                && self.failure_kind != Some(CaptureFailureKind::CompatibilitySourceCap))
            || self.post_proof.as_deref().is_some_and(|value| {
                ![
                    "post_verified_retain",
                    "release_invalid",
                    "release_unproved",
                    "superseded",
                    "verified_frame",
                ]
                .contains(&value)
            })
            || self.recovery.as_ref().is_some_and(|value| {
                !["normal", "geometry_blocked", "recovering", "released"]
                    .contains(&value.state.as_str())
                    || value.credits > 3
                    || value.cache_entries > 4
            })
            || self
                .final_stage
                .as_deref()
                .is_some_and(|s| !PREVIEW_FINAL_STAGES.contains(&s))
            || self
                .final_code
                .as_deref()
                .is_some_and(|s| !PREVIEW_FINAL_CODES.contains(&s))
            || self
                .picker_stage
                .as_deref()
                .is_some_and(|s| !STAGES.contains(&s))
            || self
                .reason
                .as_deref()
                .is_some_and(|s| !REASONS.contains(&s) && !SAFE_SELECTION_REASONS.contains(&s))
            || self
                .timeout_source
                .as_deref()
                .is_some_and(|s| !["native", "rust"].contains(&s))
            || self
                .sc_domain
                .as_deref()
                .is_some_and(|s| !["sc_stream", "other"].contains(&s))
            || self
                .api
                .as_deref()
                .is_some_and(|s| !["shareable_content", "screenshot", "jpeg"].contains(&s))
            || !(0..=10).contains(&self.native_status)
            || self.received > 255
            || self.processed > 255
        {
            return None;
        }
        Some(self)
    }
    pub(crate) fn summary(&self) -> Option<String> {
        let value = self.clone().checked()?;
        let mut text = format!(
            "阶段 {} · 请求 {} · Native操作 {}",
            value.stage, value.request, value.operation
        );
        if let Some(stage) = &value.picker_stage {
            text.push_str(&format!(
                " · 选窗 {}#{}/收到{}/处理{}",
                stage, value.picker_operation, value.received, value.processed
            ));
        }
        if let Some(source) = &value.timeout_source {
            text.push_str(&format!(" · 截止来源 {source}"));
        }
        if let Some(reason) = &value.reason {
            text.push_str(&format!(" · 原因 {reason}"));
        }
        if value.native_status != 0 {
            text.push_str(&format!(" · Native状态 {}", value.native_status));
        }
        if let Some(domain) = &value.sc_domain {
            text.push_str(&format!(
                " · SC {domain}:{}",
                value.sc_code.unwrap_or_default()
            ));
        }
        if let Some(api) = &value.api {
            text.push_str(&format!(" · API {api}"));
        }
        if value.configured_width != 0 || value.actual_width != 0 {
            text.push_str(&format!(
                " · 配置 {}×{} / 实际 {}×{} / JPEG {}字节",
                value.configured_width,
                value.configured_height,
                value.actual_width,
                value.actual_height,
                value.encoded_bytes
            ));
        }
        if let Some(stage) = &value.final_stage {
            text.push_str(&format!(" · 最终阶段 {stage}"));
        }
        if let Some(code) = &value.final_code {
            text.push_str(&format!(" · 最终结果 {code}"));
        }
        if let Some(kind) = value.failure_kind {
            text.push_str(&format!(" · 失败类型 {kind:?}"));
        }
        if let Some(geometry) = &value.geometry {
            text.push_str(&format!(
                " · 几何 CG {:?}×{:?}pt/屏内{:?} filter {:?}×{:?}pt scale {:?} 请求 {:?}×{:?}px 源越限{:?} 原因{:?} SCframe {}",
                geometry.cg_width_points, geometry.cg_height_points, geometry.cg_on_screen,
                geometry.filter_width_points, geometry.filter_height_points, geometry.point_pixel_scale,
                geometry.request_width, geometry.request_height, geometry.compatibility_source_cap,
                geometry.reason, geometry.sc_window_frame_query));
        }
        if let Some(geometry) = &value.native_geometry {
            text.push_str(&format!(
                " · 本次SC几何 CG {:?}×{:?}pt filter {:?}×{:?}pt scale {:?} frame {:?}×{:?}pt/{}",
                geometry.cg_width_points,
                geometry.cg_height_points,
                geometry.filter_width_points,
                geometry.filter_height_points,
                geometry.point_pixel_scale,
                geometry.sc_window_width_points,
                geometry.sc_window_height_points,
                geometry.sc_window_frame_query
            ));
        }
        if let Some(post) = value.post_proof {
            text.push_str(&format!(" · 后证 {post}"));
        }
        if let Some(recovery) = value.recovery {
            text.push_str(&format!(" · 恢复 {} credit{} samples{} generation{} slots{} metadata{} admission{} normal_without_credit{}",
                recovery.state, recovery.credits, recovery.samples, recovery.generation, recovery.cache_entries,
                recovery.metadata_only, recovery.admission, recovery.normal_trigger_without_recovery_credit));
        }
        Some(text)
    }
    pub(crate) fn set_final_outcome(&mut self, stage: &str, code: &str) {
        if PREVIEW_FINAL_STAGES.contains(&stage) && PREVIEW_FINAL_CODES.contains(&code) {
            self.final_stage = Some(stage.into());
            self.final_code = Some(code.into());
        }
    }
    pub(crate) fn finish_failure(&mut self, code: &str) {
        if self.final_code.as_deref() != Some(code) {
            // A successful image is not a successful post-capture proof. Do not
            // retain a previous success (or expose an unknown error string).
            self.final_stage = None;
            self.final_code = None;
            self.set_final_outcome("worker_validation", code);
        }
    }
    pub(crate) fn inherit_picker(&mut self, previous: Option<&Self>) {
        if let Some(old) = previous {
            self.picker_operation = old.picker_operation;
            self.picker_stage = old.picker_stage.clone();
            self.received = old.received;
            self.processed = old.processed;
            self.stream = old.stream;
            self.current = old.current;
            self.timeout_source = old.timeout_source.clone();
        }
    }
    pub(crate) fn fixed_failure_reason(&mut self, failure: &AppError) {
        self.reason = SAFE_SELECTION_REASONS.iter().find_map(|reason| {
            (failure.code == "preview_selection_mismatch"
                && selection_failure(&failure.code, Some(reason)).message == failure.message)
                .then(|| (*reason).to_owned())
        });
    }
}
unsafe extern "C" {
    fn luna_preview_picker_diagnostic(token: *const c_char) -> *mut c_char;
    fn luna_preview_capture_diagnostic(window: u32, pid: i32, start: *const c_char) -> *mut c_char;
}
pub(crate) fn picker_diagnostic(token: &str) -> Option<PreviewDiagnostic> {
    let token = CString::new(token).ok()?;
    // SAFETY: Exact private token is borrowed; native returns an owned, bounded snapshot.
    let value: PreviewDiagnostic =
        json(unsafe { luna_preview_picker_diagnostic(token.as_ptr()) }).ok()?;
    value.checked()
}
pub(crate) fn capture_diagnostic(identity: &NativeWindowIdentity) -> Option<PreviewDiagnostic> {
    let start = CString::new(identity.host_process_start.as_str()).ok()?;
    // SAFETY: Immutable identity is borrowed; this reads metadata and never captures.
    let value: PreviewDiagnostic = json(unsafe {
        luna_preview_capture_diagnostic(
            identity.window_id,
            identity.host_pid as i32,
            start.as_ptr(),
        )
    })
    .ok()?;
    value.checked()
}
#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    #[test]
    fn diagnostic_snapshot_rejects_free_text_and_unknown_values_before_formatting() {
        for field in [
            "stage",
            "reason",
            "picker_stage",
            "sc_domain",
            "api",
            "timeout_source",
            "final_stage",
            "final_code",
        ] {
            let mut value = serde_json::json!({"stage":"capture_complete"});
            value[field] = serde_json::json!("/Users/private/sk-secret-value");
            let diagnostic: PreviewDiagnostic = serde_json::from_value(value).unwrap();
            assert!(diagnostic.summary().is_none(), "{field}");
        }
        assert!(
            serde_json::from_value::<PreviewDiagnostic>(
                serde_json::json!({"stage":"win_pre", "token":"secret"})
            )
            .is_err()
        );
    }
    #[test]
    fn wake_diagnostic_current_failure_overrides_success_but_preserves_exact_fallback_error() {
        for code in [
            "preview_permission_revoked",
            "preview_capture_busy",
            "preview_selection_mismatch",
            "preview_selection_expired",
            "preview_cancelled",
        ] {
            let mut value = PreviewDiagnostic::worker("capture_complete");
            value.set_final_outcome("native_capture", "ok");
            value.finish_failure(code);
            assert_eq!(value.final_stage.as_deref(), Some("worker_validation"));
            assert_eq!(value.final_code.as_deref(), Some(code));
            assert!(!value.summary().unwrap().contains("最终结果 ok"));
        }
        let mut value = PreviewDiagnostic::worker("capture_complete");
        value.set_final_outcome("compatibility_capture", "preview_encoding_failed");
        value.finish_failure("preview_encoding_failed");
        assert_eq!(value.final_stage.as_deref(), Some("compatibility_capture"));
        value.set_final_outcome("native_capture", "ok");
        assert_eq!(value.final_code.as_deref(), Some("ok"));
        assert_eq!(value.final_stage.as_deref(), Some("native_capture"));
        value.finish_failure("/Users/private secret stderr");
        assert!(value.final_stage.is_none() && value.final_code.is_none());
        assert!(!value.summary().unwrap().contains("secret"));
        assert!(!value.summary().unwrap().contains("最终结果 ok"));
    }
    #[test]
    fn wake_diagnostic_preserves_native_offscreen_and_actual_compatibility_result() {
        for status in [4, 9] {
            for code in [
                "ok",
                "preview_encoding_failed",
                "preview_source_changed",
                "preview_budget_exceeded",
            ] {
                let mut value: PreviewDiagnostic = serde_json::from_value(serde_json::json!({
                    "stage":"capture_complete", "operation":114, "native_status":status,
                    "reason":"window_not_on_screen", "api":"shareable_content"
                }))
                .unwrap();
                value.set_final_outcome("compatibility_capture", code);
                assert_eq!(value.native_status, status);
                assert_eq!(value.reason.as_deref(), Some("window_not_on_screen"));
                assert_eq!(value.final_code.as_deref(), Some(code));
                let text = value.summary().unwrap();
                assert!(text.contains(&format!("Native状态 {status}")));
                assert!(text.contains("原因 window_not_on_screen"));
                assert!(text.contains("最终阶段 compatibility_capture"));
                assert!(text.contains(&format!("最终结果 {code}")));
                assert!(text.len() < 1024);
            }
        }
        let mut value = PreviewDiagnostic::worker("owner_post");
        value.set_final_outcome("worker_validation", "preview_selection_mismatch");
        value.set_final_outcome("/Users/private/secret", "preview_encoding_failed");
        value.set_final_outcome("compatibility_capture", "secret stderr text");
        assert_eq!(value.final_stage.as_deref(), Some("worker_validation"));
        assert_eq!(
            value.final_code.as_deref(),
            Some("preview_selection_mismatch")
        );
        assert!(!value.summary().unwrap().contains("secret"));
    }
    #[test]
    fn diagnostic_snapshot_preserves_actual_sc_status_dimensions_and_timeout_origin() {
        let value: PreviewDiagnostic = serde_json::from_value(serde_json::json!({
            "stage":"capture_complete", "request":9, "operation":11, "native_status":5,
            "reason":"image_dimensions", "configured_width":960, "configured_height":540,
            "actual_width":1920, "actual_height":1080, "encoded_bytes":600000,
            "sc_domain":"sc_stream", "sc_code":-3811, "picker_operation":7,
            "picker_stage":"picker_native_timeout", "timeout_source":"native", "received":1,"processed":1
        })).unwrap();
        let summary = value.summary().unwrap();
        for expected in [
            "1920×1080",
            "960×540",
            "600000",
            "sc_stream:-3811",
            "image_dimensions",
            "native",
            "Native状态 5",
        ] {
            assert!(summary.contains(expected));
        }
        assert!(summary.len() < 1024);
    }
    #[test]
    fn diagnostic_reason_requires_the_exact_existing_fixed_formatter() {
        let mut value = PreviewDiagnostic::worker("owner_pre");
        value.fixed_failure_reason(&selection_failure(
            "preview_selection_mismatch",
            Some("owner_image_mapping"),
        ));
        assert_eq!(value.reason.as_deref(), Some("owner_image_mapping"));
        value.fixed_failure_reason(&AppError::new(
            "preview_selection_mismatch",
            "preview",
            "secret owner_image_mapping /Users/private",
        ));
        assert_eq!(value.reason, None);
    }
    #[test]
    fn unknown_native_diagnostic_queries_do_not_create_picker_or_capture_resources() {
        assert!(picker_diagnostic("unregistered-diagnostic-query").is_none());
        assert!(
            capture_diagnostic(&NativeWindowIdentity {
                host_pid: 0,
                host_process_start: "not-an-owner".into(),
                window_id: 0,
                on_screen: false
            })
            .is_none()
        );
    }
}
pub fn resolve_nonce(nonce: &str) -> AppResult<NativeWindowIdentity> {
    if !nonce.starts_with("LunaWindowProof-")
        || !(32..=112).contains(&nonce.len())
        || !nonce
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(error("invalid_window_challenge"));
    }
    let nonce = CString::new(nonce).map_err(|_| error("invalid_window_challenge"))?;
    // SAFETY: A checked, NUL-terminated challenge lives until the synchronous query returns.
    let value: serde_json::Value = json(unsafe { luna_resolve_window_nonce(nonce.as_ptr()) })?;
    if let Some(code) = value.get("error").and_then(|v| v.as_str()) {
        return Err(error(code));
    }
    serde_json::from_value(value).map_err(|_| error("no_capture_window"))
}
unsafe extern "C" {
    fn luna_preview_select_begin(
        transaction: *const c_char,
        completion: extern "C" fn(*mut c_void, *mut c_char),
        context: *mut c_void,
    ) -> i32;
    fn luna_preview_select_cancel(transaction: *const c_char);
    fn luna_preview_selection_release(transaction: *const c_char);
    fn luna_preview_selection_validate(
        transaction: *const c_char,
        image_fd: i32,
        image_path: *const c_char,
        bottle: *const c_char,
    ) -> *mut c_char;
    fn luna_capture_selected_window(
        transaction: *const c_char,
        window: u32,
        host_pid: i32,
        host_start: *const c_char,
        bytes: *mut *mut u8,
        length: *mut usize,
        width: *mut u32,
        height: *mut u32,
        captured_at: *mut u64,
    ) -> i32;
    fn luna_preview_selection_geometry(
        transaction: *const c_char,
        window: u32,
        host_pid: i32,
        host_start: *const c_char,
    ) -> *mut c_char;
    fn luna_preview_normalize(
        input: *const u8,
        input_length: usize,
        bytes: *mut *mut u8,
        length: *mut usize,
        width: *mut u32,
        height: *mut u32,
    ) -> i32;
}
fn selection_error(code: &str) -> AppError {
    AppError::new(
        code,
        "preview",
        match code {
            "preview_selection_cancelled" => "已取消选择预览窗口，游戏取文保持不变",
            "preview_selection_timeout" => "选择窗口已超时，请重新选择；游戏取文保持不变",
            "preview_selection_unsupported" => {
                "此系统不支持安全选择预览窗口，需要macOS 15.2或更新版本"
            }
            "preview_selection_busy" => "已有系统窗口选择正在进行，请先完成或取消",
            "preview_permission_denied" => "窗口预览需要全局屏幕录制权限，游戏取文不受影响",
            _ => "所选窗口身份或启动器归属无法核对，请重新选择对应的游戏窗口",
        },
    )
}
const SAFE_SELECTION_REASONS: &[&str] = &[
    "owner_identity",
    "target_file_identity",
    "owner_image_mapping",
    "owner_environment",
    "helper_running_bundle",
    "helper_plist",
    "helper_bundle_fields",
    "helper_modified_after_start",
    "bottle_configuration",
    "bottle_id_inventory",
    "target_file_changed",
    "filter_scope",
    "filter_identity",
    "bottle_identity",
    "bottle_identity_changed",
    "owner_evidence_changed",
];
fn selection_failure(code: &str, reason: Option<&str>) -> AppError {
    let mut failure = selection_error(code);
    if let Some(reason) = reason.filter(|reason| SAFE_SELECTION_REASONS.contains(reason)) {
        failure.message.push_str(&format!("（核验阶段：{reason}）"));
    }
    failure
}
/// Rebuild only exact fixed native diagnostics. Arbitrary AppError messages,
/// paths, OS errors and partially matching messages never cross this boundary.
pub(crate) fn safe_preview_failure_detail(failure: &AppError) -> Option<String> {
    if failure.code != "preview_selection_mismatch" {
        return None;
    }
    SAFE_SELECTION_REASONS.iter().find_map(|reason| {
        let fixed = selection_failure("preview_selection_mismatch", Some(reason)).message;
        (failure.message == fixed).then_some(fixed)
    })
}
struct PickerReply(std::sync::mpsc::SyncSender<AppResult<NativeWindowIdentity>>);
extern "C" fn picker_completed(context: *mut c_void, raw: *mut c_char) {
    // SAFETY: Native accepted this exact heap allocation and completes it once.
    // No stack references or UI/business locks cross this callback.
    let reply = unsafe { Box::from_raw(context.cast::<PickerReply>()) };
    let result = json::<serde_json::Value>(raw).and_then(|value| {
        if let Some(code) = value.get("error").and_then(|v| v.as_str()) {
            return Err(selection_error(code));
        }
        serde_json::from_value(value).map_err(|_| selection_error("preview_selection_mismatch"))
    });
    let _ = reply.0.try_send(result);
}
pub struct SelectedPreviewWindow {
    pub identity: NativeWindowIdentity,
    token: CString,
}
impl SelectedPreviewWindow {
    pub fn token(&self) -> &str {
        self.token.to_str().unwrap_or_default()
    }
    pub fn validate(
        &self,
        context: &app_infra::hook_backend::PreviewPairingContext,
    ) -> AppResult<()> {
        use std::os::fd::AsRawFd;
        use std::os::unix::ffi::OsStrExt;
        let image = CString::new(context.image_path().as_os_str().as_bytes())
            .map_err(|_| selection_error("preview_selection_mismatch"))?;
        let bottle = CString::new(context.bottle().as_os_str().as_bytes())
            .map_err(|_| selection_error("preview_selection_mismatch"))?;
        // SAFETY: The context retains its read-only file; native only borrows FD
        // and immutable strings, returns one separately owned JSON allocation.
        let value: serde_json::Value = json(unsafe {
            luna_preview_selection_validate(
                self.token.as_ptr(),
                context.image().as_raw_fd(),
                image.as_ptr(),
                bottle.as_ptr(),
            )
        })?;
        if let Some(code) = value.get("error").and_then(|v| v.as_str()) {
            return Err(selection_failure(
                code,
                value.get("reason").and_then(|v| v.as_str()),
            ));
        }
        match value.get("kind").and_then(|v| v.as_str()) {
            Some("owner_prefix_consistent" | "helper_metadata_consistent") => Ok(()),
            _ => Err(selection_error("preview_selection_mismatch")),
        }
    }
}
impl Drop for SelectedPreviewWindow {
    fn drop(&mut self) {
        // SAFETY: Native removes only the immutable resource with this exact
        // transaction capability; an in-flight capture retains its own filter.
        unsafe { luna_preview_selection_release(self.token.as_ptr()) };
    }
}
struct CancelPicker(Option<CString>);
impl Drop for CancelPicker {
    fn drop(&mut self) {
        if let Some(token) = &self.0 {
            // SAFETY: Cancellation copies the token before queuing AppKit cleanup;
            // it neither blocks nor signals another process.
            unsafe { luna_preview_select_cancel(token.as_ptr()) };
        }
    }
}
pub async fn select_preview_window(
    transaction: String,
    current: std::sync::Arc<dyn Fn() -> bool + Send + Sync>,
) -> AppResult<std::sync::Arc<SelectedPreviewWindow>> {
    if transaction.len() > 112
        || !transaction
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(selection_error("preview_selection_mismatch"));
    }
    let token =
        CString::new(transaction).map_err(|_| selection_error("preview_selection_mismatch"))?;
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let context = Box::into_raw(Box::new(PickerReply(sender)));
    // SAFETY: Accepted calls own context until their exactly-once completion;
    // rejection never schedules a callback. Presentation itself is async/main.
    let status =
        unsafe { luna_preview_select_begin(token.as_ptr(), picker_completed, context.cast()) };
    if status != 0 {
        // SAFETY: Explicit rejection leaves the allocation entirely with Rust.
        drop(unsafe { Box::from_raw(context) });
        return Err(selection_error(match status {
            2 => "preview_selection_unsupported",
            3 => "preview_permission_denied",
            4 => "preview_selection_busy",
            _ => "preview_selection_failed",
        }));
    }
    let mut cancel = CancelPicker(Some(token.clone()));
    let result = tauri::async_runtime::spawn_blocking(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
        loop {
            if !current() {
                return Err(selection_error("preview_selection_cancelled"));
            }
            if std::time::Instant::now() >= deadline {
                return Err(selection_error("preview_selection_timeout"));
            }
            match receiver.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(value) => return value,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return Err(selection_error("preview_selection_failed")),
            }
        }
    })
    .await
    .map_err(|_| selection_error("preview_selection_failed"))??;
    if result.window_id == 0 || result.host_pid == 0 {
        return Err(selection_error("preview_selection_mismatch"));
    }
    cancel.0 = None;
    Ok(std::sync::Arc::new(SelectedPreviewWindow {
        identity: result,
        token,
    }))
}

pub fn capture_busy() -> bool {
    // SAFETY: Atomic native single-flight flag; no pointers or side effects.
    unsafe { luna_capture_busy() != 0 }
}
pub struct NativeCoverCodec;
#[cfg(test)]
mod cover_codec_tests {
    use super::*;
    use app_infra::platform::game_assets::CoverImageCodec;
    fn crc(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb88320u32 & 0u32.wrapping_sub(crc & 1));
            }
        }
        !crc
    }
    fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(tag);
        out.extend_from_slice(data);
        let mut check = tag.to_vec();
        check.extend_from_slice(data);
        out.extend_from_slice(&crc(&check).to_be_bytes());
    }
    /// A real PNG encoder fixture, using bounded stored-deflate blocks. No user/design images.
    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut header = Vec::new();
        header.extend_from_slice(&width.to_be_bytes());
        header.extend_from_slice(&height.to_be_bytes());
        header.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &header);
        let mut pixels = Vec::new();
        for y in 0..height {
            pixels.push(0);
            for x in 0..width {
                pixels.extend_from_slice(&[(x % 251) as u8, (y % 241) as u8, 127]);
            }
        }
        let mut zlib = vec![0x78, 0x01];
        let count = pixels.len().div_ceil(65_535);
        for (i, block) in pixels.chunks(65_535).enumerate() {
            zlib.push(u8::from(i + 1 == count));
            let size = block.len() as u16;
            zlib.extend_from_slice(&size.to_le_bytes());
            zlib.extend_from_slice(&(!size).to_le_bytes());
            zlib.extend_from_slice(block);
        }
        let (mut a, mut b) = (1u32, 0u32);
        for byte in pixels {
            a = (a + u32::from(byte)) % 65521;
            b = (b + a) % 65521;
        }
        zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());
        chunk(&mut out, b"IDAT", &zlib);
        chunk(&mut out, b"IEND", &[]);
        out
    }
    #[test]
    fn real_imageio_normalizes_valid_png_to_bounded_jpeg_and_rejects_invalid_sources() {
        let codec = NativeCoverCodec;
        let source = png(600, 400);
        let image = codec.normalize(&source).unwrap();
        assert!(image.bytes.starts_with(&[0xff, 0xd8]));
        assert!(image.bytes.len() <= 524_288);
        assert_eq!((image.width, image.height), (600, 400));
        assert_eq!(codec.dimensions(&image.bytes).unwrap(), (600, 400));
        assert!(codec.normalize(b"<html>not a jpeg</html>").is_err());
        assert!(codec.normalize(&png(1, 1)).is_err());
        assert!(codec.normalize(&vec![1; 8_388_609]).is_err());
        let mut truncated = source;
        truncated.truncate(22);
        assert!(codec.normalize(&truncated).is_err());
    }
    #[test]
    fn real_imageio_metadata_limits_reject_dimension_bombs_before_large_pixel_decode() {
        let codec = NativeCoverCodec;
        // Reuse a small valid PNG's compressed bytes, changing only authenticated
        // IHDR metadata. This exercises the pre-decode metadata gate, not success
        // of a large image decoder or a synthetic full-size bitmap allocation.
        let metadata = |width: u32, height: u32| {
            let mut data = png(160, 90);
            data[16..20].copy_from_slice(&width.to_be_bytes());
            data[20..24].copy_from_slice(&height.to_be_bytes());
            let checksum = crc(&data[12..29]);
            data[29..33].copy_from_slice(&checksum.to_be_bytes());
            data
        };
        assert_eq!(
            codec.dimensions(&metadata(8000, 4000)).unwrap(),
            (8000, 4000)
        );
        assert_eq!(codec.dimensions(&metadata(8192, 90)).unwrap(), (8192, 90));
        assert!(codec.dimensions(&metadata(8193, 90)).is_err());
        assert!(codec.dimensions(&metadata(160, 8193)).is_err());
        assert!(codec.dimensions(&metadata(8000, 4001)).is_err());
        assert!(codec.normalize(&metadata(8193, 90)).is_err());
        assert!(codec.normalize(&metadata(8000, 4001)).is_err());
    }
}
unsafe extern "C" {
    fn luna_preview_identity_current(
        window_id: u32,
        host_pid: i32,
        host_start: *const c_char,
    ) -> i32;
    fn luna_cover_dimensions(
        bytes: *const u8,
        length: usize,
        width: *mut u32,
        height: *mut u32,
    ) -> i32;
    fn luna_cover_normalize(
        input: *const u8,
        input_length: usize,
        bytes: *mut *mut u8,
        length: *mut usize,
        width: *mut u32,
        height: *mut u32,
    ) -> i32;
}
pub fn preview_identity_current(identity: &NativeWindowIdentity) -> bool {
    let Ok(start) = CString::new(identity.host_process_start.as_str()) else {
        return false;
    };
    // SAFETY: Immutable, NUL-terminated process identity is retained throughout the read-only CoreGraphics query.
    unsafe {
        luna_preview_identity_current(identity.window_id, identity.host_pid as i32, start.as_ptr())
            != 0
    }
}
impl app_infra::platform::game_assets::CoverImageCodec for NativeCoverCodec {
    fn normalize(
        &self,
        source: &[u8],
    ) -> AppResult<app_infra::platform::game_assets::NormalizedCover> {
        if source.is_empty() || source.len() > app_infra::platform::game_assets::SOURCE_LIMIT {
            return Err(error("cover_image_bounds"));
        }
        let (mut bytes, mut length, mut width, mut height) = (std::ptr::null_mut(), 0, 0, 0);
        // SAFETY: ImageIO reads this bounded immutable slice synchronously and transfers one malloc allocation on success.
        let status = unsafe {
            luna_cover_normalize(
                source.as_ptr(),
                source.len(),
                &mut bytes,
                &mut length,
                &mut width,
                &mut height,
            )
        };
        if status != 0 {
            return Err(error(if status == 2 {
                "cover_image_bounds"
            } else {
                "cover_image_invalid"
            }));
        }
        if bytes.is_null() {
            return Err(error("cover_image_invalid"));
        }
        let valid = length > 0
            && length <= 524_288
            && (160..=960).contains(&width)
            && (90..=960).contains(&height);
        // SAFETY: The successful native result owns exactly length bytes; copied at most once before its exact pointer is freed.
        let copied = if valid {
            // SAFETY: Successful codec response owns an allocation of exactly
            // length readable bytes until luna_native_free below.
            unsafe { std::slice::from_raw_parts(bytes, length).to_vec() }
        } else {
            Vec::new()
        };
        // SAFETY: This is the codec's independently malloc-owned buffer; the
        // copied Rust bytes no longer borrow it and it is freed exactly once.
        unsafe { luna_native_free(bytes.cast()) };
        if !valid {
            return Err(error("cover_image_bounds"));
        }
        Ok(app_infra::platform::game_assets::NormalizedCover {
            bytes: copied,
            width,
            height,
        })
    }
    fn dimensions(&self, jpeg: &[u8]) -> AppResult<(u32, u32)> {
        if jpeg.is_empty() || jpeg.len() > 524_288 {
            return Err(error("cover_image_bounds"));
        }
        let (mut width, mut height) = (0, 0);
        // SAFETY: All buffers live throughout the synchronous metadata-only call.
        if unsafe { luna_cover_dimensions(jpeg.as_ptr(), jpeg.len(), &mut width, &mut height) } != 0
        {
            return Err(error("cover_image_invalid"));
        }
        Ok((width, height))
    }
}
pub struct CapturedFrame {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub captured_at_ms: u64,
}
pub(crate) struct PreviewCaptureAttempt {
    pub result: AppResult<CapturedFrame>,
    pub diagnostic: Option<PreviewDiagnostic>,
    pub failure_kind: Option<CaptureFailureKind>,
}
pub(crate) fn capture_selected_attempt(
    identity: &NativeWindowIdentity,
    selection: Option<&str>,
    current: Option<&dyn Fn() -> AppResult<()>>,
) -> PreviewCaptureAttempt {
    let previous_operation = capture_diagnostic(identity).map(|value| value.operation);
    let mut stage = "native_capture";
    let mut failure_kind = None;
    let mut diagnostic = None;
    let result = capture_selected_impl(
        identity,
        selection,
        current,
        &mut stage,
        &mut failure_kind,
        &mut diagnostic,
        previous_operation,
    );
    if result.is_err() && failure_kind.is_none() {
        failure_kind = Some(CaptureFailureKind::OtherExistingFailure);
    }
    if let Some(value) = &mut diagnostic {
        value.failure_kind = failure_kind;
        value.set_final_outcome(
            stage,
            result
                .as_ref()
                .map_or_else(|error| error.code.as_str(), |_| "ok"),
        );
    }
    PreviewCaptureAttempt {
        result,
        diagnostic,
        failure_kind,
    }
}
fn capture_selected_impl(
    identity: &NativeWindowIdentity,
    selection: Option<&str>,
    current: Option<&dyn Fn() -> AppResult<()>>,
    stage: &mut &'static str,
    failure_kind: &mut Option<CaptureFailureKind>,
    diagnostic: &mut Option<PreviewDiagnostic>,
    previous_operation: Option<u64>,
) -> AppResult<CapturedFrame> {
    let selection = selection
        .map(CString::new)
        .transpose()
        .map_err(|_| selection_error("preview_selection_mismatch"))?;
    let start = CString::new(identity.host_process_start.as_str())
        .map_err(|_| error("host_identity_unavailable"))?;
    let (mut bytes, mut length, mut width, mut height, mut captured_at) =
        (std::ptr::null_mut(), 0, 0, 0, 0);
    // SAFETY: All outputs are writable for this synchronous call; timed-out native
    // callbacks retain native heap objects and never access these Rust addresses.
    let status = unsafe {
        if let Some(token) = &selection {
            luna_capture_selected_window(
                token.as_ptr(),
                identity.window_id,
                identity.host_pid as i32,
                start.as_ptr(),
                &mut bytes,
                &mut length,
                &mut width,
                &mut height,
                &mut captured_at,
            )
        } else {
            luna_capture_window(
                identity.window_id,
                identity.host_pid as i32,
                start.as_ptr(),
                &mut bytes,
                &mut length,
                &mut width,
                &mut height,
                &mut captured_at,
            )
        }
    };
    *diagnostic =
        capture_diagnostic(identity).filter(|value| Some(value.operation) != previous_operation);
    if status != 0 {
        *failure_kind = Some(native_capture_failure_kind(status, diagnostic.as_ref()));
        if *failure_kind == Some(CaptureFailureKind::GeometryUnavailable) {
            return Err(error("preview_geometry_unavailable"));
        }
        // Compatibility fallback is permitted only for an original retained
        // single-window picker selection and these specific SC failures. Never
        // turn permission, identity, cancellation, busy or timeout into capture.
        if let Some(token) = &selection
            && compatibility_fallback_eligible(status)
        {
            if let Some(value) = diagnostic {
                value.native_geometry = value.geometry.clone();
            }
            *stage = "compatibility_bounds";
            let source = selected_capture_bounds(identity, token);
            let (source_width, source_height, geometry) = match source {
                Ok(value) => value,
                Err(failure) => {
                    *failure_kind = Some(failure.kind);
                    if let Some(value) = diagnostic {
                        value.geometry = failure.geometry;
                    }
                    return Err(failure.error);
                }
            };
            if let Some(value) = diagnostic {
                value.geometry = Some(geometry);
            }
            let late_failure = std::cell::RefCell::new(None);
            let validate = || {
                if let Some(current) = current {
                    current()?;
                }
                let dimensions = selected_capture_bounds(identity, token).map_err(|failure| {
                    let error = failure.error.clone();
                    *late_failure.borrow_mut() = Some(failure);
                    error
                })?;
                if (dimensions.0, dimensions.1) != (source_width, source_height) {
                    return Err(error("preview_source_changed"));
                }
                Ok(())
            };
            *stage = "compatibility_capture";
            let result = window_capture::capture_window(
                identity.window_id,
                source_width,
                source_height,
                &validate,
            );
            if let Some(failure) = late_failure.into_inner() {
                *failure_kind = Some(failure.kind);
                if let Some(value) = diagnostic {
                    value.geometry = failure.geometry;
                }
            } else {
                *failure_kind = result.as_ref().err().map(|failure| {
                    if failure.code == "preview_budget_exceeded" {
                        CaptureFailureKind::OutputSafetyBudget
                    } else {
                        CaptureFailureKind::OtherExistingFailure
                    }
                });
            }
            return result;
        }
        return Err(error(match status {
            1 => "preview_unsupported",
            2 => "preview_permission_revoked",
            3 => "preview_target_closed",
            4 => "preview_window_not_on_screen",
            5 => "preview_budget_exceeded",
            7 => "preview_capture_busy",
            8 => "preview_capture_timeout",
            10 => "preview_selection_expired",
            _ => "preview_capture_failed",
        }));
    }
    if bytes.is_null() {
        return Err(error("preview_encoding_failed"));
    }
    let valid = length > 0
        && length <= 524_288
        && width <= 960
        && height <= 540
        && width > 0
        && height > 0
        && captured_at > 0;
    let copied = if valid {
        // SAFETY: Success transfers a length-sized native allocation; copy before freeing.
        unsafe { std::slice::from_raw_parts(bytes, length).to_vec() }
    } else {
        Vec::new()
    };
    // SAFETY: Exactly the malloc pointer returned by the bridge, released once.
    unsafe { luna_native_free(bytes.cast()) };
    if !valid {
        *failure_kind = Some(CaptureFailureKind::OutputSafetyBudget);
        return Err(error("preview_budget_exceeded"));
    }
    Ok(CapturedFrame {
        bytes: copied,
        width,
        height,
        captured_at_ms: captured_at,
    })
}
fn compatibility_fallback_eligible(status: i32) -> bool {
    matches!(status, 4 | 9)
}
fn native_capture_failure_kind(
    status: i32,
    diagnostic: Option<&PreviewDiagnostic>,
) -> CaptureFailureKind {
    match diagnostic {
        Some(value)
            if status == 6
                && value.failure_kind == Some(CaptureFailureKind::GeometryUnavailable)
                && value
                    .geometry
                    .as_ref()
                    .is_some_and(|geometry| geometry.checked() && geometry.reason.is_some()) =>
        {
            CaptureFailureKind::GeometryUnavailable
        }
        Some(value)
            if status == 5
                && value.failure_kind == Some(CaptureFailureKind::OutputSafetyBudget) =>
        {
            CaptureFailureKind::OutputSafetyBudget
        }
        // A compatibility source cap can only originate in the conservative
        // source-pixel computation below, never an SC/native error or text.
        _ => CaptureFailureKind::OtherExistingFailure,
    }
}
struct GeometryBoundsFailure {
    error: AppError,
    kind: CaptureFailureKind,
    geometry: Option<PreviewGeometrySnapshot>,
}
pub(crate) fn selection_geometry(
    identity: &NativeWindowIdentity,
    token: &str,
) -> AppResult<PreviewGeometrySnapshot> {
    let token = CString::new(token).map_err(|_| selection_error("preview_selection_mismatch"))?;
    selection_geometry_impl(identity, &token)
}
fn selection_geometry_impl(
    identity: &NativeWindowIdentity,
    token: &CString,
) -> AppResult<PreviewGeometrySnapshot> {
    let start = CString::new(identity.host_process_start.as_str())
        .map_err(|_| error("host_identity_unavailable"))?;
    // SAFETY: The bounded wait returns an independently allocated JSON buffer.
    // The one native work item owns all asynchronous state after a timeout.
    let value: serde_json::Value = json(unsafe {
        luna_preview_selection_geometry(
            token.as_ptr(),
            identity.window_id,
            identity.host_pid as i32,
            start.as_ptr(),
        )
    })?;
    if let Some(code) = value.get("error").and_then(serde_json::Value::as_str) {
        return Err(error(match code {
            "preview_permission_revoked" => "preview_permission_revoked",
            "preview_target_closed" => "preview_target_closed",
            "preview_selection_expired" => "preview_selection_expired",
            "preview_capture_busy" => "preview_capture_busy",
            "preview_capture_timeout" => "preview_capture_timeout",
            _ => "preview_geometry_unavailable",
        }));
    }
    let geometry: PreviewGeometrySnapshot =
        serde_json::from_value(value).map_err(|_| error("preview_geometry_unavailable"))?;
    if !geometry.checked() {
        return Err(error("preview_geometry_unavailable"));
    }
    Ok(geometry)
}
fn checked_capture_bounds(
    mut geometry: PreviewGeometrySnapshot,
) -> Result<(u32, u32, PreviewGeometrySnapshot), GeometryBoundsFailure> {
    let Some(key) = geometry.signature() else {
        if geometry.reason.is_none() {
            let positive = |value: Option<f64>| value.is_some_and(|n| n.is_finite() && n > 0.0);
            let reason = if geometry.cg_width_points.is_none()
                || geometry.cg_height_points.is_none()
                || geometry.cg_on_screen.is_none()
            {
                "cg_missing"
            } else if !positive(geometry.cg_width_points) || !positive(geometry.cg_height_points) {
                "cg_dimensions"
            } else if !positive(geometry.filter_width_points)
                || !positive(geometry.filter_height_points)
            {
                "filter_dimensions"
            } else if !positive(geometry.point_pixel_scale) {
                "point_pixel_scale"
            } else if geometry.request_width.is_none() || geometry.request_height.is_none() {
                "request_unavailable"
            } else {
                "pixel_overflow"
            };
            for value in [
                &mut geometry.cg_width_points,
                &mut geometry.cg_height_points,
                &mut geometry.filter_width_points,
                &mut geometry.filter_height_points,
                &mut geometry.point_pixel_scale,
            ] {
                if !positive(*value) {
                    *value = None;
                }
            }
            geometry.reason = Some(reason.into());
            geometry.request_width = None;
            geometry.request_height = None;
            geometry.compatibility_source_cap = None;
        }
        return Err(GeometryBoundsFailure {
            error: error("preview_geometry_unavailable"),
            kind: CaptureFailureKind::GeometryUnavailable,
            geometry: Some(geometry),
        });
    };
    let width = key[0].max(key[2]);
    let height = key[1].max(key[3]);
    // This category is produced by finite, checked conservative source pixels,
    // never by the native status 5 or a matching AppError string.
    let source_cap = width > 4_194_304 || height > 4_194_304 || width > 4_194_304 / height;
    if source_cap != geometry.compatibility_source_cap.unwrap_or(false) {
        return Err(GeometryBoundsFailure {
            error: error("preview_geometry_unavailable"),
            kind: CaptureFailureKind::OtherExistingFailure,
            geometry: Some(geometry),
        });
    }
    if source_cap {
        return Err(GeometryBoundsFailure {
            error: error("preview_budget_exceeded"),
            kind: CaptureFailureKind::CompatibilitySourceCap,
            geometry: Some(geometry),
        });
    }
    Ok((width as u32, height as u32, geometry))
}
fn selected_capture_bounds(
    identity: &NativeWindowIdentity,
    token: &CString,
) -> Result<(u32, u32, PreviewGeometrySnapshot), GeometryBoundsFailure> {
    let geometry =
        selection_geometry_impl(identity, token).map_err(|error| GeometryBoundsFailure {
            error,
            kind: CaptureFailureKind::OtherExistingFailure,
            geometry: None,
        })?;
    checked_capture_bounds(geometry)
}
pub(super) fn normalize_preview_image(input: &[u8]) -> AppResult<CapturedFrame> {
    if input.is_empty() || input.len() > 2_097_152 {
        return Err(error("preview_budget_exceeded"));
    }
    let (mut bytes, mut length, mut width, mut height) = (std::ptr::null_mut(), 0, 0, 0);
    // SAFETY: Bounded immutable input lives for this synchronous ImageIO call;
    // success transfers one malloc allocation, copied and freed exactly once.
    let status = unsafe {
        luna_preview_normalize(
            input.as_ptr(),
            input.len(),
            &mut bytes,
            &mut length,
            &mut width,
            &mut height,
        )
    };
    if status != 0 {
        return Err(error(if status == 2 {
            "preview_budget_exceeded"
        } else {
            "preview_encoding_failed"
        }));
    }
    if bytes.is_null() {
        return Err(error("preview_encoding_failed"));
    }
    let valid =
        length > 0 && length <= 524_288 && width > 0 && width <= 960 && height > 0 && height <= 540;
    let copied = if valid {
        // SAFETY: Native success owns exactly length readable bytes.
        unsafe { std::slice::from_raw_parts(bytes, length).to_vec() }
    } else {
        Vec::new()
    };
    // SAFETY: Exactly the separately owned native allocation returned above.
    unsafe { luna_native_free(bytes.cast()) };
    if !valid {
        return Err(error("preview_budget_exceeded"));
    }
    Ok(CapturedFrame {
        bytes: copied,
        width,
        height,
        captured_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
    })
}
pub fn initialize_hotkeys(callback: extern "C" fn(u32, i32)) -> Result<(), String> {
    // SAFETY: Callback has process lifetime and the exact C ABI; it only sends an edge.
    let status = unsafe { luna_hotkey_initialize(callback) };
    if status == 0 {
        Ok(())
    } else {
        Err(format!("shortcut_initialize:{status}"))
    }
}
pub fn register_hotkey(id: u32, key: u32, modifiers: u32) -> Result<(), String> {
    // SAFETY: Plain numeric key/modifier values were validated by the coordinator.
    let status = unsafe { luna_hotkey_register(id, key, modifiers) };
    if status == 0 {
        Ok(())
    } else {
        Err(format!("shortcut_conflict:{status}"))
    }
}
pub fn unregister_hotkey(id: u32) -> Result<(), String> {
    // SAFETY: The bridge looks up only a registration owned by this process.
    let status = unsafe { luna_hotkey_unregister(id) };
    if status == 0 {
        Ok(())
    } else {
        Err(format!("shortcut_cleanup:{status}"))
    }
}

pub fn choose_export_path(default_name: &str) -> AppResult<Option<std::path::PathBuf>> {
    if default_name.is_empty()
        || default_name.len() > 160
        || default_name.contains(['/', '\\', '\0'])
    {
        return Err(error("export_name_invalid"));
    }
    let name = CString::new(default_name).map_err(|_| error("export_name_invalid"))?;
    // SAFETY: The validated filename lives through the synchronous AppKit save panel.
    let value: serde_json::Value = json(unsafe { luna_choose_export_path(name.as_ptr()) })?;
    match value.get("path") {
        Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(path)) if std::path::Path::new(path).is_absolute() => {
            Ok(Some(path.into()))
        }
        _ => Err(error("export_path_unavailable")),
    }
}
pub fn write_clipboard(text: &str) -> AppResult<()> {
    // SAFETY: A valid UTF-8 Rust string and its exact byte length remain live;
    // NSString copies the bytes, including any embedded NUL, before returning.
    let success = unsafe { luna_write_clipboard(text.as_ptr(), text.len()) };
    if success != 0 {
        Ok(())
    } else {
        Err(error("clipboard_unavailable"))
    }
}

#[cfg(test)]
mod preview_geometry_recovery_tests {
    use super::*;
    unsafe extern "C" {
        fn luna_preview_geometry_request_for_test(
            width: f64,
            height: f64,
            scale: f64,
            out_width: *mut u32,
            out_height: *mut u32,
        ) -> i32;
        fn luna_preview_capture_budget_for_test(
            finalized: i32,
            width: u64,
            height: u64,
            encoded: u64,
        ) -> i32;
        fn luna_preview_geometry_gate_for_test() -> i32;
        fn luna_preview_capture_post_status(
            retained_status: i32,
            process_current: i32,
            permission_granted: i32,
        ) -> i32;
    }
    fn request(width: f64, height: f64, scale: f64) -> Option<(u32, u32)> {
        let (mut w, mut h) = (0, 0);
        // SAFETY: This invokes the exact production pure request calculation;
        // the local outputs live for the synchronous call. No OS capture runs.
        (unsafe { luna_preview_geometry_request_for_test(width, height, scale, &mut w, &mut h) }
            == 0)
            .then_some((w, h))
    }
    fn snapshot(cg: (f64, f64), filter: (f64, f64), scale: f64) -> PreviewGeometrySnapshot {
        let output = request(filter.0, filter.1, scale);
        let w = (cg.0.max(filter.0) * scale).ceil();
        let h = (cg.1.max(filter.1) * scale).ceil();
        PreviewGeometrySnapshot {
            cg_width_points: Some(cg.0),
            cg_height_points: Some(cg.1),
            cg_on_screen: Some(false),
            filter_width_points: Some(filter.0),
            filter_height_points: Some(filter.1),
            point_pixel_scale: Some(scale),
            request_width: output.map(|v| v.0),
            request_height: output.map(|v| v.1),
            compatibility_source_cap: Some(w * h > 4_194_304.0),
            reason: None,
            sc_window_frame_query: "not_queried".into(),
            sc_window_width_points: None,
            sc_window_height_points: None,
        }
    }
    #[test]
    fn geometry_request_uses_filter_points_scale_and_bounds_independently_of_cli_source_cap() {
        assert_eq!(request(1462.0, 850.0, 2.0), Some((928, 540)));
        let geometry = snapshot((1462.0, 850.0), (1462.0, 850.0), 2.0);
        let key = geometry.signature().unwrap();
        assert_eq!(key, [2924, 1700, 2924, 1700, 928, 540, 0, 1]);
        let rejected = checked_capture_bounds(geometry).err().unwrap();
        assert_eq!(rejected.kind, CaptureFailureKind::CompatibilitySourceCap);
        assert_eq!(rejected.error.code, "preview_budget_exceeded");
        // The larger CG source still controls compatibility; using the smaller
        // filter request to relax this original cap is explicitly rejected.
        let geometry = snapshot((4000.0, 3000.0), (100.0, 100.0), 1.0);
        assert_eq!(geometry.request_width, Some(100));
        assert_eq!(
            checked_capture_bounds(geometry).err().unwrap().kind,
            CaptureFailureKind::CompatibilitySourceCap
        );
    }
    #[test]
    fn geometry_request_boundary_fractional_tiny_and_invalid_values_use_production_function() {
        assert_eq!(request(960.0, 540.0, 1.0), Some((960, 540)));
        assert_eq!(request(960.5, 540.5, 1.0), Some((959, 540)));
        assert_eq!(request(0.1, 0.2, 0.5), Some((1, 1)));
        assert_eq!(request(1e-300, 1e-300, 1.0), Some((1, 1)));
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -0.0, -1.0] {
            assert!(request(value, 540.0, 1.0).is_none());
            assert!(request(960.0, value, 1.0).is_none());
            assert!(request(960.0, 540.0, value).is_none());
        }
        assert!(request(f64::MAX, 1.0, 2.0).is_none());
        assert!(request(18_446_744_073_709_551_616.0, 1.0, 1.0).is_none());
        assert!(request(f64::MIN_POSITIVE, 1.0, f64::MIN_POSITIVE).is_none());
        let source = checked_capture_bounds(snapshot((2048.0, 2048.0), (2048.0, 2048.0), 1.0))
            .ok()
            .unwrap();
        assert_eq!((source.0, source.1), (2048, 2048));
        let source = checked_capture_bounds(snapshot((2048.1, 2048.0), (2048.0, 2048.0), 1.0))
            .err()
            .unwrap();
        assert_eq!(source.kind, CaptureFailureKind::CompatibilitySourceCap);
        let source = checked_capture_bounds(snapshot((100.1, 50.2), (100.2, 50.1), 1.25))
            .ok()
            .unwrap();
        assert_eq!((source.0, source.1), (126, 63));
    }
    #[test]
    fn geometry_signature_checks_finite_scalars_and_normalizes_only_pixel_facts() {
        let a = snapshot((10.1, 10.1), (10.1, 10.1), 1.0);
        let b = snapshot((10.2, 10.2), (10.2, 10.2), 1.0);
        assert_eq!(a.signature(), b.signature());
        assert_ne!(
            snapshot((10.0, 10.0), (10.0, 10.0), 1.1).signature(),
            snapshot((10.0, 10.0), (10.0, 10.0), 1.9).signature()
        );
        // Equal canonical pixels are the same hint, not an authority identity.
        assert_eq!(
            snapshot((10.0, 10.0), (10.0, 10.0), 2.0).signature(),
            snapshot((20.0, 20.0), (20.0, 20.0), 1.0).signature()
        );
        for value in [f64::NAN, f64::INFINITY, 0.0, -0.0, -1.0, f64::MAX] {
            let mut invalid = a.clone();
            invalid.point_pixel_scale = Some(value);
            assert!(invalid.signature().is_none());
            assert_eq!(
                checked_capture_bounds(invalid).err().unwrap().kind,
                CaptureFailureKind::GeometryUnavailable
            );
        }
        let mut invalid = a.clone();
        invalid.cg_width_points = None;
        assert!(invalid.signature().is_none());
        let mut invalid = a.clone();
        invalid.sc_window_width_points = Some(10.0);
        assert!(
            invalid.signature().is_none(),
            "metadata must not claim a queried SC frame"
        );
        let mut invalid = a.clone();
        invalid.reason = Some("pixel_overflow".into());
        assert!(invalid.signature().is_none());
        let mut invalid = a;
        invalid.compatibility_source_cap = Some(true);
        assert_eq!(
            checked_capture_bounds(invalid).err().unwrap().kind,
            CaptureFailureKind::OtherExistingFailure
        );
    }
    #[test]
    fn geometry_actual_dimensions_gate_precedes_encoding_and_preserves_original_jpeg_limit() {
        // SAFETY: This invokes the production pre-encode dimensions predicate
        // and post-encode byte predicate; there is no image or OS capture.
        let budget = |width, height, bytes| unsafe {
            luna_preview_capture_budget_for_test(1, width, height, bytes)
        };
        assert_eq!(budget(960, 540, 524_288), 0);
        assert_eq!(budget(960, 540, 524_289), 2);
        assert_eq!(budget(960, 540, 0), 2);
        for (w, h) in [
            (961, 540),
            (960, 541),
            (0, 1),
            (1, 0),
            (u64::MAX, 1),
            (1, u64::MAX),
        ] {
            assert_eq!(budget(w, h, 1), 1);
        }
        assert_eq!(
            unsafe { luna_preview_capture_budget_for_test(0, 960, 540, 1) },
            2
        );
    }
    #[test]
    fn geometry_failure_kind_requires_real_typed_computation_not_status_or_error_text() {
        for status in [0, 4, 5, 6, 9] {
            assert_eq!(
                native_capture_failure_kind(status, None),
                CaptureFailureKind::OtherExistingFailure
            );
            let mut fake = PreviewDiagnostic::worker("capture_complete");
            fake.native_status = status;
            fake.reason = Some("preview_budget_exceeded".into());
            fake.failure_kind = Some(CaptureFailureKind::CompatibilitySourceCap);
            assert_eq!(
                native_capture_failure_kind(status, Some(&fake)),
                CaptureFailureKind::OtherExistingFailure
            );
        }
        let mut actual = PreviewDiagnostic::worker("capture_complete");
        actual.failure_kind = Some(CaptureFailureKind::OutputSafetyBudget);
        assert_eq!(
            native_capture_failure_kind(5, Some(&actual)),
            CaptureFailureKind::OutputSafetyBudget
        );
        assert_eq!(
            native_capture_failure_kind(9, Some(&actual)),
            CaptureFailureKind::OtherExistingFailure
        );
        actual.failure_kind = Some(CaptureFailureKind::GeometryUnavailable);
        assert_eq!(
            native_capture_failure_kind(6, Some(&actual)),
            CaptureFailureKind::OtherExistingFailure
        );
        let mut geometry = snapshot((100.0, 100.0), (100.0, 100.0), 1.0);
        geometry.reason = Some("pixel_overflow".into());
        actual.geometry = Some(geometry);
        assert_eq!(
            native_capture_failure_kind(6, Some(&actual)),
            CaptureFailureKind::GeometryUnavailable
        );
        assert_eq!(
            native_capture_failure_kind(5, Some(&actual)),
            CaptureFailureKind::OtherExistingFailure
        );
        let app_error = error("preview_budget_exceeded");
        assert_eq!(app_error.code, "preview_budget_exceeded");
        assert_eq!(
            native_capture_failure_kind(5, None),
            CaptureFailureKind::OtherExistingFailure
        );
    }
    #[test]
    fn geometry_diagnostic_rejects_unknown_fields_nonfinite_and_unbounded_private_states() {
        let mut diagnostic = PreviewDiagnostic::worker("geometry_observation");
        diagnostic.geometry = Some(snapshot((100.0, 100.0), (100.0, 100.0), 2.0));
        diagnostic.recovery = Some(PreviewRecoveryDiagnostic {
            state: "geometry_blocked".into(),
            credits: 2,
            samples: 3,
            generation: 1,
            cache_entries: 4,
            metadata_only: true,
            admission: false,
            normal_trigger_without_recovery_credit: false,
        });
        diagnostic.post_proof = Some("post_verified_retain".into());
        assert!(diagnostic.clone().checked().is_some());
        let encoded = serde_json::to_value(&diagnostic).unwrap();
        let mut unknown = encoded.clone();
        unknown["geometry"]["title"] = serde_json::json!("private");
        assert!(serde_json::from_value::<PreviewDiagnostic>(unknown).is_err());
        let mut unknown = encoded;
        unknown["recovery"]["token"] = serde_json::json!("private");
        assert!(serde_json::from_value::<PreviewDiagnostic>(unknown).is_err());
        let mut invalid = diagnostic.clone();
        invalid.geometry.as_mut().unwrap().point_pixel_scale = Some(f64::NAN);
        assert!(invalid.checked().is_none());
        let mut invalid = diagnostic.clone();
        invalid.post_proof = Some("private".into());
        assert!(invalid.checked().is_none());
        let mut invalid = diagnostic.clone();
        invalid.recovery.as_mut().unwrap().credits = 4;
        assert!(invalid.checked().is_none());
        let mut invalid = diagnostic;
        invalid.recovery.as_mut().unwrap().cache_entries = 5;
        assert!(invalid.checked().is_none());
    }
    #[test]
    fn geometry_diagnostic_requires_matching_typed_facts_and_rejects_omitted_wire_fields() {
        let mut cap = PreviewDiagnostic::worker("capture_complete");
        cap.geometry = Some(snapshot((1462.0, 850.0), (1462.0, 850.0), 2.0));
        cap.failure_kind = Some(CaptureFailureKind::CompatibilitySourceCap);
        cap.set_final_outcome("compatibility_bounds", "preview_budget_exceeded");
        assert!(cap.clone().checked().is_some());
        for field in ["failure_kind", "geometry"] {
            let mut wire = serde_json::to_value(&cap).unwrap();
            wire.as_object_mut().unwrap().remove(field);
            let missing: PreviewDiagnostic = serde_json::from_value(wire).unwrap();
            assert!(missing.checked().is_none(), "missing {field}");
        }
        let mut invalid = cap.clone();
        invalid.geometry.as_mut().unwrap().compatibility_source_cap = Some(false);
        assert!(invalid.checked().is_none());
        let mut invalid = cap;
        invalid.geometry = Some(snapshot((100.0, 100.0), (100.0, 100.0), 1.0));
        assert!(invalid.checked().is_none());
        let mut unavailable = PreviewDiagnostic::worker("capture_complete");
        let mut geometry = snapshot((100.0, 100.0), (100.0, 100.0), 1.0);
        geometry.point_pixel_scale = None;
        geometry.request_width = None;
        geometry.request_height = None;
        geometry.compatibility_source_cap = None;
        geometry.reason = Some("point_pixel_scale".into());
        unavailable.geometry = Some(geometry);
        unavailable.failure_kind = Some(CaptureFailureKind::GeometryUnavailable);
        assert!(unavailable.clone().checked().is_some());
        let mut missing = unavailable.clone();
        missing.geometry = None;
        assert!(missing.checked().is_none());
        let mut missing = unavailable;
        missing.geometry.as_mut().unwrap().reason = None;
        assert!(missing.checked().is_none());
    }
    #[test]
    fn geometry_native_work_timeout_keeps_global_gate_until_real_completion_and_reuses_one_channel()
    {
        // SAFETY: This seam invokes the production metadata work helper with
        // controlled heap-owned read blocks. It performs no OS/window access.
        assert_eq!(unsafe { luna_preview_geometry_gate_for_test() }, 127);
        assert!(!capture_busy());
    }
    #[test]
    fn geometry_post_capture_policy_preserves_observed_revocation_after_regrant_and_owner_failures()
    {
        // SAFETY: This invokes the callback's actual production pure policy;
        // it returns fixed statuses and has no permission/token side effects.
        let status = |retained, process, granted| unsafe {
            luna_preview_capture_post_status(retained, process, granted)
        };
        for process in [0, 1] {
            for granted in [0, 1] {
                assert_eq!(
                    status(2, process, granted),
                    2,
                    "an observed revocation cannot be restored by a later grant"
                );
            }
        }
        for retained in [0, 1, 3, 10, -1] {
            assert_eq!(
                status(retained, 1, 0),
                2,
                "current TCC denial takes precedence"
            );
        }
        assert_eq!(status(0, 1, 1), 0);
        assert_eq!(status(0, 0, 1), 3);
        assert_eq!(status(3, 1, 1), 3, "owner/filter mismatch remains invalid");
        assert_eq!(
            status(10, 1, 1),
            10,
            "expired token cannot be restored by a grant"
        );
        assert_eq!(status(10, 0, 1), 10);
        assert_eq!(status(1, 1, 1), 3, "unknown retained status fails closed");
        assert_eq!(status(-1, 1, 1), 3);
    }
}

#[cfg(test)]
mod quit_tests {
    use super::*;

    #[test]
    fn native_quit_bridge_rejects_non_main_thread_without_appkit_mutation() {
        extern "C" fn callback() {}
        extern "C" fn completed(_: *mut c_void, _: i32) {}
        let (initialize, reply, queued_reply) = std::thread::spawn(|| {
            // SAFETY: These entry points must reject this background caller
            // before reading NSApp/delegate or changing pending reply state.
            unsafe {
                (
                    luna_quit_initialize(callback),
                    luna_quit_reply(0),
                    luna_quit_reply_async(0, completed, std::ptr::null_mut()),
                )
            }
        })
        .join()
        .unwrap();
        assert_eq!(initialize, 1);
        assert_eq!(reply, -1);
        assert_eq!(queued_reply, 1);
    }

    #[test]
    fn quit_reply_completion_releases_context_and_notifies_exactly_once() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        struct Dropped(Arc<AtomicUsize>);
        impl Drop for Dropped {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let drops = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let owned = Dropped(drops.clone());
        let (sender, mut receiver) = tauri::async_runtime::channel(1);
        let context = Box::into_raw(Box::new(QuitReplyContext {
            after_reply: Box::new(move || {
                observed.fetch_add(1, Ordering::SeqCst);
                drop(owned);
            }),
            sender,
        }));
        complete_quit_reply(context.cast(), 0);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(receiver.try_recv().unwrap(), 0);
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn cli_fallback_requires_the_specific_compatibility_or_offscreen_status() {
        for status in [0, 1, 2, 3, 5, 6, 7, 8, 10, 11, -1] {
            assert!(!compatibility_fallback_eligible(status));
        }
        assert!(compatibility_fallback_eligible(4));
        assert!(compatibility_fallback_eligible(9));
    }
}

#[cfg(test)]
mod preview_owner_mapping_tests {
    use super::*;
    use std::fs::{DirBuilder, OpenOptions};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    unsafe extern "C" {
        fn mmap(
            address: *mut c_void,
            length: usize,
            protection: i32,
            flags: i32,
            fd: i32,
            offset: i64,
        ) -> *mut c_void;
        fn munmap(address: *mut c_void, length: usize) -> i32;
    }
    struct OwnedDirectory(std::path::PathBuf);
    impl Drop for OwnedDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.0.join("mapped.bin"));
            let _ = std::fs::remove_file(self.0.join("unmapped.bin"));
            let _ = std::fs::remove_dir(&self.0);
        }
    }
    struct OwnedMapping(Option<*mut c_void>);
    impl Drop for OwnedMapping {
        fn drop(&mut self) {
            if let Some(address) = self.0.take() {
                // SAFETY: Only this guard owns the successful 4096-byte mapping;
                // it is never read, written, or unmapped elsewhere while owned.
                let _ = unsafe { munmap(address, 4096) };
            }
        }
    }
    #[test]
    fn actual_darwin_file_mapping_is_a_positive_witness_without_requiring_vm_eof() {
        let name = format!(
            "luna-preview-mapping-ci-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let directory_path = std::env::temp_dir().join(name);
        DirBuilder::new()
            .mode(0o700)
            .create(&directory_path)
            .unwrap();
        let directory = OwnedDirectory(directory_path);
        let image = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(directory.0.join("mapped.bin"))
            .unwrap();
        let other = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(directory.0.join("unmapped.bin"))
            .unwrap();
        image.set_len(4096).unwrap();
        other.set_len(4096).unwrap();
        let pid = i32::try_from(std::process::id()).unwrap();
        let witnessed = |fd| {
            // SAFETY: Both live descriptors belong to this test. Native borrows
            // them only for fstat/read-only bounded process mapping queries.
            unsafe { luna_preview_owner_maps_file(pid, fd) }
        };
        assert_eq!(witnessed(image.as_raw_fd()), 0);
        assert_eq!(witnessed(other.as_raw_fd()), 0);
        // SAFETY: The owned file has 4096 initialized zero bytes. Darwin's
        // PROT_READ=1 / MAP_PRIVATE=2 creates an independently owned mapping;
        // no test or production window/process configuration is changed.
        let address = unsafe { mmap(std::ptr::null_mut(), 4096, 1, 2, image.as_raw_fd(), 0) };
        assert_ne!(address as usize, usize::MAX, "mmap failed");
        let mut mapping = OwnedMapping(Some(address));
        assert_eq!(witnessed(image.as_raw_fd()), 1);
        assert_eq!(witnessed(other.as_raw_fd()), 0);
        let address = mapping.0.take().unwrap();
        // SAFETY: Taking the address transfers its sole ownership out of the
        // guard; this exact successful mapping is unmapped once here.
        let unmapped = unsafe { munmap(address, 4096) };
        assert_eq!(unmapped, 0);
        assert_eq!(witnessed(image.as_raw_fd()), 0);
        assert_eq!(witnessed(other.as_raw_fd()), 0);
        // SAFETY: Invalid arguments must be rejected before any process query.
        let invalid_pid = unsafe { luna_preview_owner_maps_file(0, image.as_raw_fd()) };
        assert_eq!(invalid_pid, 0);
        // SAFETY: fstat of this invalid descriptor must fail without side effects.
        let invalid_fd = unsafe { luna_preview_owner_maps_file(pid, -1) };
        assert_eq!(invalid_fd, 0);
    }
}

#[cfg(test)]
mod preview_evidence_tests {
    use super::*;
    use std::fs::DirBuilder;
    use std::os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, MetadataExt},
    };

    #[test]
    fn only_exact_fixed_native_reason_messages_are_displayed() {
        for reason in SAFE_SELECTION_REASONS {
            let failure = selection_failure("preview_selection_mismatch", Some(reason));
            assert_eq!(safe_preview_failure_detail(&failure), Some(failure.message));
        }
        for message in [
            "arbitrary failure",
            "所选窗口身份或启动器归属无法核对，请重新选择对应的游戏窗口（核验阶段：unknown）",
            "所选窗口身份或启动器归属无法核对，请重新选择对应的游戏窗口（核验阶段：owner_evidence_changed） /private/secret",
            "api_key=secret（核验阶段：owner_identity）",
        ] {
            assert!(
                safe_preview_failure_detail(&AppError::new(
                    "preview_selection_mismatch",
                    "preview",
                    message,
                ))
                .is_none()
            );
        }
        let mut failure = selection_failure("preview_selection_mismatch", Some("owner_identity"));
        failure.code = "preview_capture_busy".into();
        assert!(safe_preview_failure_detail(&failure).is_none());
        assert!(
            safe_preview_failure_detail(&selection_failure(
                "preview_selection_mismatch",
                Some("/private/secret"),
            ))
            .is_none()
        );
    }
    struct OwnedDirectory(std::path::PathBuf);
    impl Drop for OwnedDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn identity(path: &std::path::Path) -> serde_json::Value {
        let path = CString::new(path.as_os_str().as_bytes()).unwrap();
        // SAFETY: Native borrows a live immutable path and returns one owned
        // JSON allocation. This calls the same read-only BottleIdentity helper.
        json(unsafe { luna_preview_bottle_identity_for_test(path.as_ptr()) }).unwrap()
    }
    #[test]
    fn actual_bottle_directory_identity_survives_registry_replace_but_rejects_root_redirect() {
        let path = std::env::temp_dir().join(format!(
            "luna-preview-bottle-ci-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        let owned = OwnedDirectory(path);
        let bottle = owned.0.join("bottle");
        let other = owned.0.join("other");
        std::fs::create_dir(&bottle).unwrap();
        std::fs::create_dir(&other).unwrap();
        let original = identity(&bottle);
        assert!(original.get("error").is_none());
        assert_eq!(original["type"], "directory");
        assert_eq!(
            original["canonicalRoot"],
            bottle.canonicalize().unwrap().to_str().unwrap()
        );
        let before_children = std::fs::metadata(&bottle).unwrap();
        std::fs::write(bottle.join("system.reg"), b"old registry").unwrap();
        std::fs::write(
            bottle.join("system.reg.new"),
            b"atomic replacement registry",
        )
        .unwrap();
        std::fs::rename(bottle.join("system.reg.new"), bottle.join("system.reg")).unwrap();
        let after_children = std::fs::metadata(&bottle).unwrap();
        assert_eq!(
            (before_children.dev(), before_children.ino()),
            (after_children.dev(), after_children.ino())
        );
        assert_ne!(
            (
                before_children.size(),
                before_children.mtime(),
                before_children.mtime_nsec()
            ),
            (
                after_children.size(),
                after_children.mtime(),
                after_children.mtime_nsec()
            ),
            "fixture must actually change the parent directory facts used by the old FileStamp"
        );
        assert_eq!(
            identity(&bottle),
            original,
            "ordinary child replacement must not invalidate the pair"
        );
        assert!(original.get("size").is_none());
        assert!(original.get("mtimeSec").is_none());
        assert!(original.get("mtimeNano").is_none());
        std::fs::rename(&bottle, owned.0.join("old-bottle")).unwrap();
        std::fs::create_dir(&bottle).unwrap();
        let replacement = identity(&bottle);
        assert_eq!(replacement["canonicalRoot"], original["canonicalRoot"]);
        assert_ne!(replacement["ino"], original["ino"]);
        assert_ne!(replacement, original);
        let alias = owned.0.join("alias");
        std::os::unix::fs::symlink(&bottle, &alias).unwrap();
        let before_redirect = identity(&alias);
        std::fs::remove_file(&alias).unwrap();
        std::os::unix::fs::symlink(&other, &alias).unwrap();
        let redirected = identity(&alias);
        assert_ne!(
            redirected["canonicalRoot"],
            before_redirect["canonicalRoot"]
        );
        assert_ne!(redirected, before_redirect);
        let file = owned.0.join("not-directory");
        std::fs::write(&file, b"file").unwrap();
        assert!(identity(&file).get("error").is_some());
        assert!(identity(&owned.0.join("missing")).get("error").is_some());
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct CollectorProcessIdentity {
    pub pid: u32,
    pub start: String,
    pub dev: u64,
    pub ino: u64,
}
unsafe extern "C" {
    fn luna_collector_process_identity(pid: i32) -> *mut c_char;
    fn luna_collector_private_fd(fd: i32) -> i32;
    fn luna_collector_parent(parent: i32) -> i32;
    fn luna_collector_spawn(
        executable: *const c_char,
        control_fd: i32,
        frame_fd: i32,
        pid: *mut i32,
    ) -> i32;
    fn luna_collector_reap(pid: i32, exit_code: *mut i32) -> i32;
    fn luna_collector_runloop() -> i32;
    fn luna_collector_stop_runloop();
}
pub(crate) fn collector_process_identity(pid: u32) -> AppResult<CollectorProcessIdentity> {
    // SAFETY: Read-only bounded proc/stat query returns one native-owned JSON allocation.
    json(unsafe {
        luna_collector_process_identity(
            i32::try_from(pid).map_err(|_| error("collector_process_identity"))?,
        )
    })
}
pub(crate) fn collector_private_fd(fd: i32) -> bool {
    // SAFETY: Native only inspects this descriptor and makes it close-on-exec.
    unsafe { luna_collector_private_fd(fd) != 0 }
}
pub(crate) fn collector_parent(parent: u32) -> bool {
    // SAFETY: Read-only parent PID and exact executable vnode checks; no AppKit.
    unsafe { luna_collector_parent(parent as i32) != 0 }
}
pub(crate) fn collector_spawn(
    executable: &std::path::Path,
    control_fd: i32,
    frame_fd: i32,
) -> AppResult<u32> {
    use std::os::unix::ffi::OsStrExt;
    let executable =
        CString::new(executable.as_os_str().as_bytes()).map_err(|_| error("collector_spawn"))?;
    let mut pid = 0;
    // SAFETY: Native borrows live FDs/absolute path, duplicates only allowlisted
    // descriptors via posix_spawn actions, and writes one owned child PID.
    let status =
        unsafe { luna_collector_spawn(executable.as_ptr(), control_fd, frame_fd, &mut pid) };
    if status != 0 || pid <= 0 {
        Err(error("collector_spawn"))
    } else {
        Ok(pid as u32)
    }
}
pub(crate) fn collector_reap(pid: u32) -> AppResult<Option<i32>> {
    let mut code = 0;
    // SAFETY: Caller retains sole reaping authority for this exact child PID;
    // waitpid never signals or touches unrelated game processes.
    match unsafe { luna_collector_reap(pid as i32, &mut code) } {
        0 => Ok(None),
        1 => Ok(Some(code)),
        _ => Err(error("collector_reap")),
    }
}
pub(crate) fn collector_runloop() -> AppResult<()> {
    // SAFETY: Called only by the early worker branch on its own main thread.
    if unsafe { luna_collector_runloop() } == 0 {
        Ok(())
    } else {
        Err(error("collector_runloop"))
    }
}
pub(crate) fn collector_stop_runloop() {
    // SAFETY: Only schedules a stop on this worker's main RunLoop; no main-app state.
    unsafe { luna_collector_stop_runloop() };
}
