use crate::contracts::*;
pub fn validate_preferences(p: &Preferences) -> AppResult<()> {
    if !["global"].contains(&p.scope_key.as_str()) && !p.scope_key.starts_with("game:") {
        return Err(AppError::new(
            "invalid_scope",
            "preferences",
            "无效偏好范围",
        ));
    }
    if !p.hud_background_opacity.is_finite() || !(0.0..=1.0).contains(&p.hud_background_opacity) {
        return Err(AppError::new(
            "invalid_preferences",
            "preferences",
            "背景透明度必须在0至1之间",
        ));
    }
    if !["system", "light", "dark"].contains(&p.theme.as_str())
        || !["system", "reduced", "material"].contains(&p.transparency_mode.as_str())
        || [p.source_font_pt, p.translation_font_pt, p.hud_width_pt]
            .iter()
            .any(|x| !x.is_finite() || *x <= 0.)
    {
        return Err(AppError::new(
            "invalid_preferences",
            "preferences",
            "字号、宽度或显示选项无效",
        ));
    }
    Ok(())
}
