use app_core::contracts::{AppError, AppResult, ShortcutIntent, ShortcutProjection};
use tauri::{
    AppHandle,
    menu::{Menu, MenuItem, SubmenuBuilder},
};

fn reading_menu_items() -> [(ShortcutIntent, &'static str, &'static str); 3] {
    [
        (
            ShortcutIntent::ToggleHud,
            "reading_toggle_hud",
            "显示 / 隐藏字幕",
        ),
        (
            ShortcutIntent::ToggleCapturePause,
            "reading_toggle_pause",
            "暂停 / 继续取文",
        ),
        (
            ShortcutIntent::CycleReadingDisplayMode,
            "reading_cycle_display",
            "原文 / 译文显示",
        ),
    ]
}
fn shortcut_label(canonical: &str) -> String {
    canonical
        .split('+')
        .map(|part| match part {
            "Ctrl" | "Control" => "⌃",
            "Alt" | "Option" => "⌥",
            "Shift" => "⇧",
            "Super" | "Cmd" | "Command" | "CmdOrCtrl" | "Meta" => "⌘",
            key => key,
        })
        .collect::<Vec<_>>()
        .join("")
}
pub fn capture_pause_enabled(source_kind: Option<&str>, state: Option<&str>) -> bool {
    matches!((source_kind, state), (Some("hook"), Some("active")))
}
pub fn sync_capture_pause(app: &AppHandle, enabled: bool) -> AppResult<()> {
    let item = app
        .menu()
        .and_then(|menu| menu.get("reading_toggle_pause"))
        .and_then(|item| item.as_menuitem().cloned())
        .ok_or_else(|| AppError::new("menu_unavailable", "desktop", "阅读菜单暂不可用"))?;
    item.set_enabled(enabled).map_err(|_| {
        AppError::new(
            "menu_capture_state",
            "desktop",
            "暂停菜单状态暂未更新，请重试",
        )
    })
}
/// Display effective bindings without registering a second NSMenu accelerator.
/// Carbon remains the single native global-shortcut owner.
pub fn sync_shortcuts(app: &AppHandle, projection: &ShortcutProjection) -> AppResult<()> {
    for (intent, id, label) in reading_menu_items() {
        let effective = projection
            .bindings
            .iter()
            .find(|binding| binding.intent == intent)
            .and_then(|binding| binding.effective.as_deref());
        if let Some(item) = app
            .menu()
            .and_then(|menu| menu.get(id))
            .and_then(|item| item.as_menuitem().cloned())
        {
            let text = effective
                .map(|key| format!("{label}   {}", shortcut_label(key)))
                .unwrap_or_else(|| label.into());
            item.set_text(text).map_err(|_| {
                AppError::new(
                    "menu_shortcut_projection",
                    "desktop",
                    "菜单快捷键显示暂未更新，请重试",
                )
            })?;
        }
    }
    Ok(())
}
pub fn build(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let quit = MenuItem::with_id(app, "quit", "退出 Luna", true, Some("CmdOrCtrl+Q"))?;
    let application = SubmenuBuilder::new(app, "Luna")
        .about(None)
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .item(&quit)
        .build()?;
    let edit = SubmenuBuilder::new(app, "编辑")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    let main = MenuItem::with_id(app, "main", "显示主窗口", true, Some("CmdOrCtrl+1"))?;
    let hud = MenuItem::with_id(app, "hud", "显示字幕", true, Some("CmdOrCtrl+2"))?;
    let settings = MenuItem::with_id(app, "settings", "设置…", true, Some("CmdOrCtrl+,"))?;
    let reset = MenuItem::with_id(app, "hud_reset", "恢复字幕位置", true, None::<&str>)?;
    let toggle_hud = MenuItem::with_id(
        app,
        "reading_toggle_hud",
        "显示 / 隐藏字幕",
        true,
        None::<&str>,
    )?;
    let toggle_pause = MenuItem::with_id(
        app,
        "reading_toggle_pause",
        "暂停 / 继续取文",
        false,
        None::<&str>,
    )?;
    let cycle_display = MenuItem::with_id(
        app,
        "reading_cycle_display",
        "原文 / 译文显示",
        true,
        None::<&str>,
    )?;
    let show_game = MenuItem::with_id(app, "show_game", "显示游戏", true, None::<&str>)?;
    let reading = SubmenuBuilder::new(app, "阅读")
        .item(&toggle_hud)
        .item(&toggle_pause)
        .item(&cycle_display)
        .separator()
        .item(&show_game)
        .item(&reset)
        .build()?;
    let windows = SubmenuBuilder::new(app, "窗口")
        .item(&main)
        .item(&hud)
        .item(&settings)
        .separator()
        .minimize()
        .maximize()
        .close_window()
        .build()?;
    Menu::with_items(app, &[&application, &edit, &reading, &windows])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn effective_canonical_shortcuts_have_apple_menu_labels() {
        assert_eq!(shortcut_label("Alt+Super+H"), "⌥⌘H");
        assert_eq!(shortcut_label("Control+Shift+F12"), "⌃⇧F12");
        assert_eq!(reading_menu_items().len(), 3);
    }
    #[test]
    fn pause_menu_requires_an_active_hook_session() {
        assert!(!capture_pause_enabled(None, None));
        assert!(!capture_pause_enabled(Some("manual"), Some("active")));
        for state in ["stopped", "opening", "failed", "crashed"] {
            assert!(!capture_pause_enabled(Some("hook"), Some(state)));
        }
        assert!(capture_pause_enabled(Some("hook"), Some("active")));
    }
}
