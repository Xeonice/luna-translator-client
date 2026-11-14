use crate::contracts::*;
use std::collections::BTreeSet;

/// A batch is one reviewed addition, never an implicit bulk edit or metadata overwrite.
pub fn normalize_game_batch(items: &mut [GameBatchItem]) -> AppResult<()> {
    if !(1..=100).contains(&items.len()) {
        return Err(AppError::new(
            "invalid_batch_size",
            "library",
            "每次批量添加需要 1 至 100 个项目",
        ));
    }
    let mut item_ids = BTreeSet::new();
    let mut targets = BTreeSet::new();
    for item in items {
        crate::domain::require_id(&item.item_id)?;
        if !item_ids.insert(item.item_id.clone()) {
            return Err(AppError::new(
                "duplicate_batch_item",
                "library",
                "批量项目身份重复",
            ));
        }
        if item.game.game_id.is_some() || item.game.row_revision.is_some() {
            return Err(AppError::new(
                "batch_add_only",
                "library",
                "批量添加不能编辑已有记录，请单独编辑游戏资料",
            ));
        }
        normalize_game_input(&mut item.game)?;
        validate_game(&item.game)?;
        let target = &item.game.target;
        if target.environment_kind == "crossover"
            && let Some(path) = &target.target_path_key
            && !targets.insert((target.environment_key.clone(), path.clone()))
        {
            return Err(AppError::new(
                "duplicate_batch_target",
                "library",
                "同一批次含有相同容器和路径的重复目标",
            ));
        }
    }
    Ok(())
}
pub fn validate_game(input: &GameInput) -> AppResult<()> {
    if input.display_name.trim().is_empty() {
        return Err(AppError::new("required_name", "library", "请填写游戏名称"));
    }
    for v in [
        &input.target.environment_key,
        input
            .target
            .target_path_key
            .as_ref()
            .unwrap_or(&String::new()),
    ] {
        if v.contains('\0') {
            return Err(AppError::new(
                "invalid_target",
                "library",
                "目标不能包含空字符",
            ));
        }
    }
    if !["crossover", "unresolved"].contains(&input.target.environment_kind.as_str()) {
        return Err(AppError::new(
            "unsupported_environment",
            "library",
            "首版仅支持CrossOver环境",
        ));
    }
    if !["unresolved", "verified", "stale", "invalid"]
        .contains(&input.target.verification_state.as_str())
    {
        return Err(AppError::new(
            "invalid_target",
            "library",
            "未知目标核验状态",
        ));
    }
    if input.target.verification_state == "verified"
        && (input.target.environment_key.is_empty()
            || input
                .target
                .canonical_exe_path
                .as_deref()
                .is_none_or(str::is_empty)
            || input
                .target
                .target_path_key
                .as_deref()
                .is_none_or(str::is_empty)
            || !matches!(input.target.architecture.as_deref(), Some("x86" | "x64")))
    {
        return Err(AppError::new(
            "unverified_target",
            "library",
            "已核验目标缺少完整身份",
        ));
    }
    if input.target.arguments.iter().any(|x| x.contains('\0')) {
        return Err(AppError::new(
            "invalid_arguments",
            "library",
            "启动参数不能包含空字符",
        ));
    }
    Ok(())
}
/// Windows lexical path normalization. Filesystem/process verification remains the platform port's job.
pub fn normalize_windows_path(value: &str) -> AppResult<(String, String)> {
    let fail = || {
        AppError::new(
            "invalid_windows_path",
            "library",
            "目标必须是Windows绝对路径，不能使用相对路径或设备流",
        )
    };
    if value.chars().any(char::is_control) {
        return Err(fail());
    }
    let value = value.replace('/', "\\");
    let value = value.strip_prefix("\\\\?\\").unwrap_or(&value);
    let (prefix, tail, protected) = if value.len() >= 3
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value.as_bytes()[1] == b':'
        && value.as_bytes()[2] == b'\\'
    {
        (
            format!("{}:\\", value[..1].to_ascii_uppercase()),
            &value[3..],
            0,
        )
    } else if let Some(tail) = value.strip_prefix("\\\\") {
        ("\\\\".into(), tail, 2)
    } else {
        return Err(fail());
    };
    let mut parts = Vec::new();
    for part in tail.split('\\') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            if parts.len() <= protected {
                return Err(fail());
            }
            parts.pop();
            continue;
        }
        if part.ends_with('.')
            || part.ends_with(' ')
            || part
                .chars()
                .any(|c| matches!(c, ':' | '*' | '?' | '<' | '>' | '|' | '"'))
        {
            return Err(fail());
        }
        parts.push(part);
    }
    if parts.len() < protected || parts.is_empty() {
        return Err(fail());
    }
    let path = format!("{prefix}{}", parts.join("\\"));
    let key = path.to_lowercase();
    Ok((path, key))
}
pub fn normalize_game_input(input: &mut GameInput) -> AppResult<()> {
    let t = &mut input.target;
    if let Some(path) = t.canonical_exe_path.clone() {
        match normalize_windows_path(&path) {
            Ok((canonical, key)) => {
                t.canonical_exe_path = Some(canonical);
                t.target_path_key = Some(key);
            }
            Err(e) => {
                if t.verification_state == "verified" {
                    return Err(e);
                }
                t.target_path_key = None;
            }
        }
    } else {
        t.target_path_key = None;
    }
    if t.environment_kind == "crossover" {
        let locator = t
            .bottle_locator
            .as_deref()
            .or_else(|| t.environment_key.strip_prefix("crossover:"))
            .ok_or_else(|| {
                AppError::new(
                    "invalid_environment",
                    "library",
                    "CrossOver环境缺少真实Bottle定位",
                )
            })?;
        if !locator.starts_with('/') || locator.contains('\0') {
            return Err(AppError::new(
                "invalid_environment",
                "library",
                "Bottle定位必须是本机绝对路径",
            ));
        }
        let mut parts = Vec::new();
        for part in locator.split('/') {
            if part.is_empty() || part == "." {
                continue;
            }
            if part == ".." {
                if parts.pop().is_none() {
                    return Err(AppError::new(
                        "invalid_environment",
                        "library",
                        "Bottle定位超出根目录",
                    ));
                }
            } else {
                parts.push(part);
            }
        }
        let locator = format!("/{}", parts.join("/"));
        t.environment_key = format!("crossover:{locator}");
        t.bottle_locator = Some(locator);
    } else {
        t.target_path_key = None;
    }
    Ok(())
}
