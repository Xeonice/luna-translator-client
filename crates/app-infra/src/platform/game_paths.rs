//! Read-only reuse of the existing Bottle path resolver. Never spawns a game.
use app_core::contracts::*;
use std::path::{Path, PathBuf};
pub fn executable(game: &Game) -> AppResult<PathBuf> {
    let fail = || AppError::new("cover_game_path", "library", "无法核验游戏资源目录");
    let target = &game.target;
    let value = target
        .canonical_exe_path
        .as_deref()
        .or(target.launch_path.as_deref())
        .ok_or_else(fail)?;
    let path = if let Some(locator) = &target.bottle_locator {
        let bottle = Path::new(locator).canonicalize().map_err(|_| fail())?;
        if !bottle.join("cxbottle.conf").is_file()
            || target.environment_key != format!("crossover:{}", bottle.display())
        {
            return Err(fail());
        }
        super::game_launch::resolve(value, &super::game_launch::drive_mappings(&bottle)?)?
    } else if value.starts_with('/') {
        Path::new(value).canonicalize().map_err(|_| fail())?
    } else {
        return Err(fail());
    };
    if !path.is_file() {
        return Err(fail());
    }
    super::executables::pe_architecture(&path)?;
    Ok(path)
}
