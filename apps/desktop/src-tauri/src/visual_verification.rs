//! Explicit, validation-only composition root. It never constructs product services.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use tauri::{Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

const FLAG: &str = "--general-visual-verification";
const MODE_CONTRACT: &str = "luna-general-native-isolated-renderer/v1:20261108";
const LABEL: &str = "general-visual-verification";
const STATES: &[&str] = &[
    "all-page-settings-general-normal",
    "all-page-settings-general-empty",
    "all-page-settings-general-loading",
    "all-page-settings-general-error",
    "all-page-settings-general-disabled",
    "all-page-settings-general-long-text",
    "all-page-settings-general-light",
    "all-page-settings-general-narrow",
    "all-page-settings-general-zoom-200",
    "all-page-settings-general-keyboard-focus-escape",
    "all-page-settings-general-scope-global",
    "all-page-settings-general-scope-game",
    "all-page-settings-general-restore-inheritance",
    "all-page-settings-general-save-failed",
];
const FACTS: &[&str] = &[
    "title",
    "headings",
    "text",
    "switches",
    "buttons",
    "scope",
    "status",
    "dirty",
    "menuOpen",
    "focused",
    "geometry",
    "scrolls",
    "theme",
    "zoom",
    "fontStatus",
    "documentVisibility",
    "hasFocus",
    "userAgent",
    "replay",
    "errors",
    "renderSource",
    "provenance",
    "actions",
    "controlledCommands",
    "readQueries",
    "saveCalls",
    "startupWrites",
    "forbiddenEffects",
    "caseAssertions",
    "loadedResources",
    "nativeAcl",
];

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Context {
    schema_version: u8,
    kind: String,
    transport: String,
    side: String,
    state_id: String,
    subcase: String,
    canvas_id: String,
    nonce: String,
    width: u32,
    height: u32,
    theme: String,
    zoom: u8,
}
fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl Context {
    fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != 1
            || self.kind != "general-native-visual-context/v1"
            || self.transport != "packaged_native"
            || !["reference", "candidate"].contains(&self.side.as_str())
            || !STATES.contains(&self.state_id.as_str())
            || !hex(&self.nonce, 32)
        {
            return Err("invalid finite visual identity");
        }
        if ![(1000, 720), (760, 560)].contains(&(self.width, self.height))
            || !["dark", "light"].contains(&self.theme.as_str())
            || ![1, 2].contains(&self.zoom)
            || self.canvas_id
                != format!(
                    "{}x{}-{}-z{}",
                    self.width, self.height, self.theme, self.zoom
                )
        {
            return Err("invalid finite visual canvas");
        }
        if self.subcase != "primary"
            && !(self.subcase == "initial" && self.state_id == "all-page-settings-general-loading")
            && !(self.subcase == "menu-open"
                && self.state_id == "all-page-settings-general-keyboard-focus-escape")
            && !(self.subcase == "content-end"
                && self.state_id == "all-page-settings-general-normal"
                && self.zoom == 2)
        {
            return Err("invalid finite visual subcase");
        }
        Ok(())
    }
    fn source_state(&self) -> &str {
        match self.state_id.as_str() {
            "all-page-settings-general-empty" => "empty",
            "all-page-settings-general-loading" if self.subcase == "initial" => "loading-initial",
            "all-page-settings-general-loading" => "loading",
            "all-page-settings-general-error" => "read-error",
            "all-page-settings-general-save-failed" => "save-failed",
            _ => "normal",
        }
    }
    fn route(&self) -> String {
        if self.side == "candidate" {
            return "general-visual.html".into();
        }
        format!(
            "general-reference/settings.html?context=native-proposal&state={}&theme={}&zoom={}",
            self.source_state(),
            self.theme,
            self.zoom
        )
    }
}
fn parse(args: &[String]) -> Result<Option<Context>, &'static str> {
    if !args.iter().any(|arg| arg.starts_with("--general-visual")) {
        return Ok(None);
    }
    if args.len() != 6 || args[0] != FLAG {
        return Err("expected finite side/state/canvas/subcase/nonce arguments");
    }
    let pieces: Vec<_> = args[3].split('-').collect();
    if pieces.len() != 3 {
        return Err("invalid canvas identifier");
    }
    let size: Vec<_> = pieces[0].split('x').collect();
    if size.len() != 2 {
        return Err("invalid canvas dimensions");
    }
    let context = Context {
        schema_version: 1,
        kind: "general-native-visual-context/v1".into(),
        transport: "packaged_native".into(),
        side: args[1].clone(),
        state_id: args[2].clone(),
        canvas_id: args[3].clone(),
        subcase: args[4].clone(),
        nonce: args[5].clone(),
        width: size[0].parse().map_err(|_| "invalid width")?,
        height: size[1].parse().map_err(|_| "invalid height")?,
        theme: pieces[1].into(),
        zoom: pieces[2]
            .strip_prefix('z')
            .ok_or("invalid zoom")?
            .parse()
            .map_err(|_| "invalid zoom")?,
    };
    context.validate()?;
    Ok(Some(context))
}
fn bounded(value: &Value, depth: usize) -> bool {
    if depth > 8 {
        return false;
    }
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => true,
        Value::String(s) => s.len() <= 32_768,
        Value::Array(a) => a.len() <= 128 && a.iter().all(|v| bounded(v, depth + 1)),
        Value::Object(o) => {
            o.len() <= 128
                && o.iter().all(|(k, v)| {
                    k.len() <= 128
                        && !["__proto__", "constructor", "prototype"].contains(&k.as_str())
                        && bounded(v, depth + 1)
                })
        }
    }
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    context: Context,
    facts: Value,
    error: Option<String>,
}
fn validate_observation(context: &Context, observation: &Observation) -> Result<(), &'static str> {
    observation.context.validate()?;
    if &observation.context != context {
        return Err("visual observation identity mismatch");
    }
    let object = observation
        .facts
        .as_object()
        .ok_or("visual facts must be a finite object")?;
    if object.keys().any(|key| !FACTS.contains(&key.as_str()))
        || !bounded(&observation.facts, 0)
        || observation
            .error
            .as_ref()
            .is_some_and(|error| error.len() > 8192)
        || serde_json::to_vec(observation)
            .map_err(|_| "invalid observation encoding")?
            .len()
            > 65_536
    {
        return Err("visual observation exceeds finite bounds");
    }
    Ok(())
}
fn write_once(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| "visual record encoding failed")?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|_| "fresh private visual record required")?;
    file.write_all(&bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .map_err(|_| "visual record write failed".into())
}
fn prepare_directory(nonce: &str) -> Result<PathBuf, String> {
    if !hex(nonce, 32) {
        return Err("invalid private visual nonce".into());
    }
    let base = std::env::temp_dir().join("luna-general-visual");
    match fs::create_dir(&base) {
        Ok(()) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&base, fs::Permissions::from_mode(0o700))
                    .map_err(|_| "cannot protect visual base")?;
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err("cannot create private visual base".into()),
    }
    let meta = fs::symlink_metadata(&base).map_err(|_| "cannot inspect private visual base")?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("visual base must be a real private directory".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err("visual base is not private".into());
        }
    }
    let directory = base.join(nonce);
    fs::create_dir(&directory).map_err(|_| "visual nonce must be fresh")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .map_err(|_| "cannot protect visual directory")?;
    }
    Ok(directory)
}
struct VisualRuntime {
    context: Context,
    directory: PathBuf,
    source_identity: Value,
    written: AtomicBool,
}
#[tauri::command]
fn general_visual_observed(
    window: WebviewWindow,
    runtime: tauri::State<'_, VisualRuntime>,
    observation: Observation,
) -> Result<(), String> {
    if window.label() != LABEL {
        return Err("isolated visual window required".into());
    }
    validate_observation(&runtime.context, &observation).map_err(str::to_string)?;
    if runtime
        .written
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("visual observation was already sealed".into());
    }
    let inner = window
        .inner_size()
        .map_err(|_| "visual inner bounds unavailable")?;
    let outer = window
        .outer_size()
        .map_err(|_| "visual outer bounds unavailable")?;
    let scale = window
        .scale_factor()
        .map_err(|_| "visual backing scale unavailable")?;
    if !scale.is_finite() || scale <= 0. {
        return Err("visual backing scale invalid".into());
    }
    let record = serde_json::json!({"schema_version":1,"kind":"general-native-raw-observation/v1","controlled_transport":true,"native_visual_rendering_only":true,"os_runtime_verified":false,"storage_runtime_verified":false,"game_tests_executed":false,
        "pid":std::process::id(),"executable":std::env::current_exe().map_err(|_|"visual executable unavailable")?,"window_label":LABEL,"source_identity":runtime.source_identity,
        "window":{"inner_pixels":{"width":inner.width,"height":inner.height},"outer_pixels":{"width":outer.width,"height":outer.height},"backing_scale":scale},"observation":observation});
    write_once(&runtime.directory.join("observation.json"), &record)
}
fn source_identity(app: &tauri::AppHandle) -> Result<Value, String> {
    let path = app
        .path()
        .resource_dir()
        .map_err(|_| "visual resource directory unavailable")?
        .join("luna-source-identity.json");
    let mut file = fs::File::open(path).map_err(|_| "sealed package source identity required")?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(131_073)
        .read_to_end(&mut bytes)
        .map_err(|_| "sealed source read failed")?;
    if bytes.len() > 131_072 {
        return Err("sealed source identity exceeds limit".into());
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| "sealed source identity malformed")?;
    if value.get("kind").and_then(Value::as_str) != Some("luna_sealed_source_identity")
        || !value
            .get("source_tree_sha256")
            .and_then(Value::as_str)
            .is_some_and(|s| hex(s, 64))
    {
        return Err("sealed source identity invalid".into());
    }
    Ok(value)
}
#[cfg(target_os = "macos")]
fn run(context: Context, mut tauri_context: tauri::Context<tauri::Wry>) -> Result<(), String> {
    *tauri_context.config_mut() = serde_json::from_str(include_str!("../tauri.visual.conf.json"))
        .map_err(|_| "isolated visual config malformed")?;
    let directory = prepare_directory(&context.nonce)?;
    write_once(
        &directory.join("started.json"),
        &serde_json::json!({"schema_version":1,"kind":"general-native-isolated-start/v1","mode_contract":MODE_CONTRACT,"context":context,"pid":std::process::id(),"controlled_transport":true,"business_runtime_constructed":false}),
    )?;
    let setup_context = context.clone();
    let setup_directory = directory.clone();
    let app=tauri::Builder::default().enable_macos_default_menu(false).invoke_handler(tauri::generate_handler![general_visual_observed]).setup(move |app| {
        app.set_activation_policy(tauri::ActivationPolicy::Accessory);
        let identity=source_identity(app.handle()).map_err(std::io::Error::other)?;
        app.manage(VisualRuntime{context:setup_context.clone(),directory:setup_directory.clone(),source_identity:identity,written:AtomicBool::new(false)});
        let encoded=serde_json::to_string(&setup_context)?;
        let mut initialization=format!("Object.defineProperty(window,'__LUNA_GENERAL_VISUAL__',{{value:Object.freeze({encoded}),writable:false,configurable:false}});");
        if setup_context.side=="reference" {
            initialization.push_str(r#"
window.addEventListener('DOMContentLoaded', () => {
  import('tauri://localhost/general-reference/replay.js').catch(error => {
    const message = ('General source replay failed: ' + String(error)).slice(0, 2048);
    const observation = {
      context: window.__LUNA_GENERAL_VISUAL__,
      facts: {errors: [message], renderSource: 'independent-user-approved-general-html'},
      error: message,
    };
    window.__GENERAL_VISUAL_RESULT__ = Object.freeze(observation);
    const bridge = window.__TAURI_INTERNALS__;
    if (!bridge || typeof bridge.invoke !== 'function') {
      console.error('General source error observation transport is absent');
      return;
    }
    bridge.invoke('general_visual_observed', {observation}).catch(reportError => {
      console.error('General source error observation failed', String(reportError).slice(0, 2048));
    });
  });
}, {once: true});
"#);
        }
        let route=setup_context.route();
        let expected_path=format!("/{}",route.split('?').next().unwrap_or(""));
        let expected_query=route.split_once('?').map(|(_,query)|query.to_owned());
        let window=WebviewWindowBuilder::new(app,LABEL,WebviewUrl::App(route.into()))
            .title("设置").inner_size(setup_context.width as f64,setup_context.height as f64).min_inner_size(760.,560.)
            .transparent(true).theme(Some(if setup_context.theme=="light" {tauri::Theme::Light}else{tauri::Theme::Dark}))
            .incognito(true).center().focused(false)
            .on_navigation(move |url| url.scheme()=="tauri" && url.host_str()==Some("localhost") && url.path()==expected_path && url.query()==expected_query.as_deref())
            .on_new_window(|_,_|tauri::webview::NewWindowResponse::Deny)
            .title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true)
            .effects(tauri::window::EffectsBuilder::new().effect(tauri::window::Effect::HudWindow).state(tauri::window::EffectState::Active).radius(16.).build())
            .initialization_script(&initialization).build()?;
        // Reassert the declared logical canvas after overlay creation; raw Native/CG bounds remain authoritative.
        window.set_size(tauri::LogicalSize::new(setup_context.width as f64,setup_context.height as f64))?;
        Ok(())
    }).build(tauri_context).map_err(|_|"isolated visual application setup failed")?;
    let handle = app.handle().clone();
    let finish_dir = directory.clone();
    let finish_nonce = context.nonce;
    std::thread::spawn(move || {
        let start = Instant::now();
        loop {
            if let Ok(meta) = fs::symlink_metadata(finish_dir.join("finish")) {
                if meta.is_file() && !meta.file_type().is_symlink() && meta.len() <= 64 {
                    if let Ok(value) = fs::read_to_string(finish_dir.join("finish")) {
                        if value == finish_nonce {
                            let _ = write_once(
                                &finish_dir.join("finished.json"),
                                &serde_json::json!({"status":"finished_by_owned_controller","pid":std::process::id()}),
                            );
                            handle.exit(0);
                            break;
                        }
                    }
                }
            }
            if start.elapsed() > Duration::from_secs(45) {
                let _ = write_once(
                    &finish_dir.join("finished.json"),
                    &serde_json::json!({"status":"watchdog_timeout","pid":std::process::id()}),
                );
                handle.exit(70);
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
    app.run(|_, _| {});
    Ok(())
}
/// Pure early parsing, before either product services or an AppKit event loop.
pub(crate) fn request() -> Result<Option<Context>, &'static str> {
    parse(&std::env::args().skip(1).collect::<Vec<_>>())
}
/// Uses the same sealed asset context; only this private branch replaces its config.
pub(crate) fn run_request(request: Context, context: tauri::Context<tauri::Wry>) -> i32 {
    #[cfg(target_os = "macos")]
    {
        match run(request, context) {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("General visual verification failed: {error}");
                65
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (request, context);
        64
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn args() -> Vec<String> {
        [
            FLAG,
            "candidate",
            STATES[0],
            "1000x720-dark-z1",
            "primary",
            "0123456789abcdef0123456789abcdef",
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    }
    fn observation(context: Context) -> Observation {
        Observation {
            context,
            facts: serde_json::json!({"title":"设置","dirty":false}),
            error: None,
        }
    }
    #[test]
    fn absent_flag_retains_normal_path() {
        assert_eq!(parse(&["--capture-worker".into()]).unwrap(), None);
    }
    #[test]
    fn all_finite_canvases_and_states_parse() {
        for side in ["reference", "candidate"] {
            for state in STATES {
                for canvas in [
                    "1000x720-dark-z1",
                    "1000x720-dark-z2",
                    "1000x720-light-z1",
                    "1000x720-light-z2",
                    "760x560-dark-z1",
                    "760x560-dark-z2",
                    "760x560-light-z1",
                    "760x560-light-z2",
                ] {
                    let mut a = args();
                    a[1] = side.into();
                    a[2] = (*state).into();
                    a[3] = canvas.into();
                    assert!(parse(&a).unwrap().is_some());
                }
            }
        }
    }
    #[test]
    fn reserved_visual_namespace_never_falls_back_to_product_setup() {
        for flag in [
            "--general-visual-verification=reference",
            "--general-visual-verification-typo",
            "--general-visual-verificatoin",
            "--general-visual",
        ] {
            assert!(parse(&[flag.into()]).is_err());
        }
        assert_eq!(parse(&["--capture-worker".into()]).unwrap(), None);
        assert_eq!(
            parse(&["--unrelated-existing-argument".into()]).unwrap(),
            None
        );
    }
    #[test]
    fn unknown_extra_and_partial_arguments_fail() {
        for index in 1..6 {
            let mut a = args();
            a[index] = "unknown".into();
            assert!(parse(&a).is_err());
        }
        let mut a = args();
        a.push("--url=https://example.invalid".into());
        assert!(parse(&a).is_err());
        assert!(parse(&[FLAG.into()]).is_err());
    }
    #[test]
    fn nonce_cannot_escape_or_be_reused_as_path() {
        for nonce in [
            "../escape",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "0123456789abcdef0123456789abcdefff/extra",
        ] {
            let mut a = args();
            a[5] = nonce.into();
            assert!(parse(&a).is_err());
        }
    }
    #[test]
    fn subcases_are_state_and_zoom_bound() {
        let mut a = args();
        a[4] = "initial".into();
        assert!(parse(&a).is_err());
        a[2] = STATES[2].into();
        assert!(parse(&a).is_ok());
        a[4] = "menu-open".into();
        assert!(parse(&a).is_err());
        a[2] = STATES[9].into();
        assert!(parse(&a).is_ok());
        a[4] = "content-end".into();
        assert!(parse(&a).is_err());
        a[2] = STATES[0].into();
        a[3] = "1000x720-dark-z2".into();
        assert!(parse(&a).is_ok());
    }
    #[test]
    fn observations_are_bound_and_cannot_install_controls() {
        let c = parse(&args()).unwrap().unwrap();
        let mut o = observation(c.clone());
        assert!(validate_observation(&c, &o).is_ok());
        o.context.nonce = "ffffffffffffffffffffffffffffffff".into();
        assert!(validate_observation(&c, &o).is_err());
        o = observation(c.clone());
        o.facts = serde_json::json!({"url":"https://example.invalid"});
        assert!(validate_observation(&c, &o).is_err());
        o.facts = serde_json::json!({"text":"x".repeat(65536)});
        assert!(validate_observation(&c, &o).is_err());
    }
    #[test]
    fn unknown_packet_fields_and_wrong_types_fail() {
        let c = parse(&args()).unwrap().unwrap();
        let mut v = serde_json::to_value(observation(c)).unwrap();
        v["path"] = Value::String("/tmp/other".into());
        assert!(serde_json::from_value::<Observation>(v).is_err());
        let mut a = args();
        a[3] = "1000x720-dark-ztrue".into();
        assert!(parse(&a).is_err());
    }
    #[test]
    fn json_depth_and_arrays_are_bounded() {
        let mut v = Value::Null;
        for _ in 0..10 {
            v = Value::Array(vec![v]);
        }
        assert!(!bounded(&v, 0));
        assert!(!bounded(&serde_json::to_value(vec!["x"; 129]).unwrap(), 0));
    }
}
