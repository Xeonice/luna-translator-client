use app_core::contracts::*;
use std::{
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};
fn error(message: &str) -> AppError {
    AppError::new("executable_invalid", "platform", message)
}
pub fn inspect(paths: Vec<String>) -> AppResult<Vec<ExecutableSelection>> {
    if paths.len() > 128 {
        return Err(error("一次最多选择128个程序"));
    }
    paths
        .into_iter()
        .map(|path| {
            let path = PathBuf::from(path)
                .canonicalize()
                .map_err(|_| error("无法读取所选程序"))?;
            if !path.is_file() {
                return Err(error("请选择EXE文件"));
            }
            let architecture = pe_architecture(&path)?;
            let mut environment_key = None;
            let mut bottle_locator = None;
            for ancestor in path.ancestors() {
                if ancestor.join("cxbottle.conf").is_file() {
                    environment_key = Some(format!("crossover:{}", ancestor.display()));
                    bottle_locator = Some(ancestor.to_string_lossy().into_owned());
                    break;
                }
            }
            Ok(ExecutableSelection {
                display_name: path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "游戏".into()),
                path: path.to_string_lossy().into_owned(),
                architecture: Some(architecture),
                environment_key,
                bottle_locator,
            })
        })
        .collect()
}
pub fn pe_architecture(path: &Path) -> AppResult<String> {
    let mut file = std::fs::File::open(path).map_err(|_| error("无法读取程序头"))?;
    let len = file
        .metadata()
        .map_err(|_| error("无法读取程序属性"))?
        .len();
    let mut dos = [0u8; 64];
    file.read_exact(&mut dos)
        .map_err(|_| error("程序不是有效PE文件"))?;
    if &dos[..2] != b"MZ" {
        return Err(error("只支持Windows PE可执行文件"));
    }
    let offset =
        u32::from_le_bytes(dos[60..64].try_into().map_err(|_| error("PE偏移无效"))?) as u64;
    if offset > len.saturating_sub(24) {
        return Err(error("PE头超出文件范围"));
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|_| error("无法定位PE头"))?;
    let mut header = [0u8; 24];
    file.read_exact(&mut header)
        .map_err(|_| error("PE头不完整"))?;
    if &header[..4] != b"PE\0\0" || u16::from_le_bytes([header[22], header[23]]) & 0x2000 != 0 {
        return Err(error("请选择EXE程序，不能使用DLL"));
    }
    match u16::from_le_bytes([header[4], header[5]]) {
        0x14c => Ok("x86".into()),
        0x8664 => Ok("x64".into()),
        _ => Err(error("首版仅支持x86或x64的Windows程序")),
    }
}
#[cfg(target_os = "macos")]
pub fn pick() -> AppResult<Vec<ExecutableSelection>> {
    let script = "var app=Application.currentApplication(); app.includeStandardAdditions=true; JSON.stringify(app.chooseFile({withPrompt:'选择游戏 EXE，可多选',ofType:['exe'],multipleSelectionsAllowed:true}).map(function(p){return p.toString();}));";
    let result = std::process::Command::new("/usr/bin/osascript")
        .args(["-l", "JavaScript", "-e", script])
        .output()
        .map_err(|_| error("无法打开文件选择器"))?;
    if !result.status.success() {
        return Ok(vec![]);
    }
    let paths: Vec<String> =
        serde_json::from_slice(&result.stdout).map_err(|_| error("文件选择结果无效"))?;
    inspect(paths)
}
#[cfg(not(target_os = "macos"))]
pub fn pick() -> AppResult<Vec<ExecutableSelection>> {
    Err(error("当前正式包尚未提供该平台的文件选择实现"))
}
