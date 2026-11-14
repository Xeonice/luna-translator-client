use crate::contracts::*;
pub fn canonical_u64(value: &str) -> AppResult<u64> {
    let n = value
        .parse::<u64>()
        .map_err(|_| AppError::new("invalid_u64", "validation", "无效的64位规范十进制值"))?;
    if n.to_string() != value {
        return Err(AppError::new(
            "invalid_u64",
            "validation",
            "64位值必须使用无前导零的十进制文本",
        ));
    }
    Ok(n)
}
pub fn utf16_span(text: &str, start: u32, end: u32) -> AppResult<(u32, u32)> {
    let (start, end) = (start as usize, end as usize);
    if start > end
        || end > text.len()
        || !text.is_char_boundary(start)
        || !text.is_char_boundary(end)
    {
        return Err(AppError::new(
            "invalid_span",
            "validation",
            "词语跨度不是有效UTF-8边界",
        ));
    }
    Ok((
        text[..start].encode_utf16().count() as u32,
        text[..end].encode_utf16().count() as u32,
    ))
}
