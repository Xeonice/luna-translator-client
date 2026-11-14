//! Single-owner MeCab 0.996 ABI. Native pointers remain inside this process.
use app_core::contracts::LexicalToken;
use libloading::Library;
use sha2::{Digest, Sha256};
use std::{
    ffi::{CStr, CString, c_char, c_int, c_long, c_void},
    fs::File,
    io::Read,
    path::{Component, Path},
};

#[repr(C)]
struct Node {
    prev: *const Node,
    next: *const Node,
    enext: *const Node,
    bnext: *const Node,
    rpath: *const c_void,
    lpath: *const c_void,
    surface: *const c_char,
    feature: *const c_char,
    id: u32,
    length: u16,
    rlength: u16,
    rc_attr: u16,
    lc_attr: u16,
    posid: u16,
    char_type: u8,
    stat: u8,
    isbest: u8,
    alpha: f32,
    beta: f32,
    prob: f32,
    wcost: i16,
    cost: c_long,
}
type New = unsafe extern "C" fn(c_int, *mut *mut c_char) -> *mut c_void;
type Destroy = unsafe extern "C" fn(*mut c_void);
type Parse = unsafe extern "C" fn(*mut c_void, *const c_char) -> *const Node;
type Error = unsafe extern "C" fn(*mut c_void) -> *const c_char;
type Version = unsafe extern "C" fn() -> *const c_char;

pub const MAX_TEXT_BYTES: usize = 65_536;
pub const MAX_FRAME_BYTES: usize = 1_048_576;
pub struct Mecab {
    tagger: *mut c_void,
    destroy: Destroy,
    parse: Parse,
    error: Error,
    _library: Library,
    pub resource_version: String,
}
impl Drop for Mecab {
    fn drop(&mut self) {
        // SAFETY: Unique tagger; its library is still alive during Drop.
        unsafe { (self.destroy)(self.tagger) };
    }
}

pub fn verify_resources(root: &Path) -> Result<(String, String), String> {
    let manifest_bytes =
        std::fs::read(root.join("language/manifest.json")).map_err(|e| e.to_string())?;
    if manifest_bytes
        != include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/language/manifest.json"
        ))
    {
        return Err("language manifest differs from compiled resource lock".into());
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).map_err(|e| e.to_string())?;
    if manifest["schema_version"] != 1
        || manifest["mecab_version"] != "0.996"
        || manifest["unidic_version"] != "2025.12"
    {
        return Err("unapproved language resource identity".into());
    }
    for entry in manifest["files"]
        .as_array()
        .ok_or("resource files missing")?
    {
        let relative = entry["path"].as_str().ok_or("invalid resource path")?;
        // The lexical worker validates only resources it actually owns. The
        // read-only dictionary adapter independently verifies its database.
        if ![
            "language/mecab/",
            "language/unidic/",
            "licenses/mecab/",
            "licenses/unidic/",
        ]
        .iter()
        .any(|prefix| relative.starts_with(prefix))
        {
            continue;
        }
        let path = Path::new(relative);
        if path.is_absolute()
            || path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err("unsafe resource path".into());
        }
        let full = root.join(path);
        let metadata = full.symlink_metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() != entry["size"].as_u64().ok_or("invalid resource size")?
        {
            return Err(format!("resource size/type mismatch: {}", path.display()));
        }
        let mut stream = File::open(&full).map_err(|e| e.to_string())?;
        let mut hasher = Sha256::new();
        let mut block = [0u8; 65_536];
        loop {
            let count = stream.read(&mut block).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            hasher.update(&block[..count]);
        }
        if format!("{:x}", hasher.finalize())
            != entry["sha256"].as_str().ok_or("invalid resource digest")?
        {
            return Err(format!("resource hash mismatch: {}", path.display()));
        }
    }
    Ok((
        manifest["resource_version"]
            .as_str()
            .ok_or("missing resource version")?
            .into(),
        format!("{:x}", Sha256::digest(&manifest_bytes)),
    ))
}

impl Mecab {
    pub fn open(root: &Path) -> Result<Self, String> {
        let (resource_version, _) = verify_resources(root)?;
        // SAFETY: The DLL/dylib bytes were verified against the reviewed offline manifest.
        let library = unsafe { Library::new(root.join("language/mecab/libmecab.2.dylib")) }
            .map_err(|e| e.to_string())?;
        // SAFETY: These are the documented C ABI symbols of pinned MeCab 0.996.
        let (new, destroy, parse, error, version) = unsafe {
            (
                *library
                    .get::<New>(b"mecab_new\0")
                    .map_err(|e| e.to_string())?,
                *library
                    .get::<Destroy>(b"mecab_destroy\0")
                    .map_err(|e| e.to_string())?,
                *library
                    .get::<Parse>(b"mecab_sparse_tonode\0")
                    .map_err(|e| e.to_string())?,
                *library
                    .get::<Error>(b"mecab_strerror\0")
                    .map_err(|e| e.to_string())?,
                *library
                    .get::<Version>(b"mecab_version\0")
                    .map_err(|e| e.to_string())?,
            )
        };
        // SAFETY: Version points to a static NUL-terminated library string.
        if unsafe { CStr::from_ptr(version()) }.to_bytes() != b"0.996" {
            return Err("MeCab runtime version mismatch".into());
        }
        let dictionary = root.join("language/unidic");
        let arguments = [
            CString::new("language-worker").unwrap(),
            CString::new("-r").unwrap(),
            CString::new("/dev/null").unwrap(),
            CString::new("-d").unwrap(),
            CString::new(dictionary.to_str().ok_or("non UTF-8 dictionary path")?)
                .map_err(|e| e.to_string())?,
        ];
        let mut argv: Vec<_> = arguments
            .iter()
            .map(|arg| arg.as_ptr().cast_mut())
            .collect();
        // SAFETY: Arguments are live and NUL-terminated throughout tagger creation.
        let tagger = unsafe { new(argv.len() as c_int, argv.as_mut_ptr()) };
        if tagger.is_null() {
            return Err("MeCab could not open the bundled UniDic".into());
        }
        Ok(Self {
            tagger,
            destroy,
            parse,
            error,
            _library: library,
            resource_version,
        })
    }
    pub fn analyze(&mut self, text: &str) -> Result<Vec<LexicalToken>, String> {
        if text.len() > MAX_TEXT_BYTES {
            return Err("language input exceeds 65536 UTF-8 bytes".into());
        }
        let input = CString::new(text).map_err(|_| "language input contains NUL")?;
        // SAFETY: Tagger and input are valid; nodes are borrowed only until next parse/Drop.
        let mut pointer = unsafe { (self.parse)(self.tagger, input.as_ptr()) };
        if pointer.is_null() {
            // SAFETY: Error string is owned by the live tagger.
            return Err(unsafe { CStr::from_ptr((self.error)(self.tagger)) }
                .to_string_lossy()
                .into_owned());
        }
        let mut cursor = 0usize;
        let mut utf16_cursor = 0u32;
        let mut tokens = Vec::new();
        let mut visited = 0usize;
        while !pointer.is_null() {
            visited += 1;
            if visited > text.len() + 3 {
                return Err("invalid cyclic MeCab node chain".into());
            }
            // SAFETY: MeCab owns a valid node chain until the next parse.
            let node = unsafe { &*pointer };
            pointer = node.next;
            if matches!(node.stat, 2 | 3) {
                continue;
            }
            if node.length == 0 || node.rlength < node.length {
                return Err("invalid MeCab surface length".into());
            }
            let start = cursor
                .checked_add(usize::from(node.rlength - node.length))
                .ok_or("span overflow")?;
            let end = start
                .checked_add(usize::from(node.length))
                .ok_or("span overflow")?;
            if start > cursor {
                let whitespace = whitespace_span(text, cursor, start, utf16_cursor)?;
                utf16_cursor = whitespace.utf16_end;
                tokens.push(whitespace);
            }
            let surface = text
                .get(start..end)
                .ok_or("MeCab returned a non UTF-8-aligned span")?;
            // SAFETY: node.surface is readable for node.length bytes by MeCab's ABI.
            let actual = unsafe {
                std::slice::from_raw_parts(node.surface.cast::<u8>(), usize::from(node.length))
            };
            if actual != surface.as_bytes() {
                return Err("MeCab surface differs from original source span".into());
            }
            if surface.chars().all(char::is_whitespace) {
                let whitespace = whitespace_span(text, start, end, utf16_cursor)?;
                utf16_cursor = whitespace.utf16_end;
                cursor = end;
                tokens.push(whitespace);
                continue;
            }
            let feature = bounded_string(node.feature, 16_384)?;
            let fields = parse_csv(&feature)?;
            let lemma = fields
                .get(7)
                .filter(|f| !f.is_empty() && f.as_str() != "*")
                .map(String::as_str)
                .unwrap_or(surface);
            let reading = fields
                .get(20)
                .filter(|f| !f.is_empty() && f.as_str() != "*")
                .or_else(|| fields.get(9).filter(|f| !f.is_empty() && f.as_str() != "*"))
                .map(|f| hiragana(f))
                .unwrap_or_default();
            let part_of_speech = fields
                .iter()
                .take(4)
                .filter(|f| !f.is_empty() && f.as_str() != "*")
                .cloned()
                .collect::<Vec<_>>()
                .join("-");
            tokens.push(LexicalToken {
                surface: surface.into(),
                lemma: lemma.into(),
                reading,
                part_of_speech,
                byte_start: start as u32,
                byte_end: end as u32,
                utf16_start: utf16_cursor,
                utf16_end: utf16_cursor + surface.encode_utf16().count() as u32,
                known: node.stat == 0,
            });
            utf16_cursor = tokens.last().expect("token was appended").utf16_end;
            cursor = end;
        }
        if cursor < text.len() {
            tokens.push(whitespace_span(text, cursor, text.len(), utf16_cursor)?);
        }
        validate_spans(text, &tokens)?;
        Ok(tokens)
    }
}

/// MeCab omits whitespace from its lexical node chain. Preserve those bytes as
/// source-layout tokens; never claim that a missing non-whitespace span is a word.
fn whitespace_span(
    text: &str,
    start: usize,
    end: usize,
    utf16_start: u32,
) -> Result<LexicalToken, String> {
    let surface = text
        .get(start..end)
        .ok_or("invalid UTF-8 whitespace span")?;
    if surface.is_empty() || !surface.chars().all(char::is_whitespace) {
        return Err("MeCab skipped a non-whitespace source span".into());
    }
    Ok(LexicalToken {
        surface: surface.into(),
        lemma: surface.into(),
        reading: String::new(),
        part_of_speech: "whitespace".into(),
        byte_start: start as u32,
        byte_end: end as u32,
        utf16_start,
        utf16_end: utf16_start + surface.encode_utf16().count() as u32,
        known: false,
    })
}

fn bounded_string(pointer: *const c_char, maximum: usize) -> Result<String, String> {
    if pointer.is_null() {
        return Err("null MeCab feature pointer".into());
    }
    for length in 0..maximum {
        // SAFETY: MeCab's ABI provides a readable NUL-terminated feature string.
        if unsafe { *pointer.add(length) } == 0 {
            // SAFETY: The scan established these initialized bytes.
            return String::from_utf8(
                unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), length) }.to_vec(),
            )
            .map_err(|e| e.to_string());
        }
    }
    Err("MeCab feature exceeds bounded copy limit".into())
}
pub fn hiragana(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if ('\u{30a1}'..='\u{30f6}').contains(&c) {
                char::from_u32(c as u32 - 0x60).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}
fn parse_csv(value: &str) -> Result<Vec<String>, String> {
    let mut output = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                current.push('"');
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                output.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    if quoted {
        return Err("unterminated UniDic CSV field".into());
    }
    output.push(current);
    Ok(output)
}
pub fn validate_spans(text: &str, tokens: &[LexicalToken]) -> Result<(), String> {
    let mut last = 0;
    let mut utf16_end = 0;
    for token in tokens {
        let start = token.byte_start as usize;
        let end = token.byte_end as usize;
        if start != last || start >= end || text.get(start..end) != Some(token.surface.as_str()) {
            return Err("invalid language token source spans".into());
        }
        let expected_start = utf16_end + text[last..start].encode_utf16().count() as u32;
        let expected_end = expected_start + token.surface.encode_utf16().count() as u32;
        if expected_start != token.utf16_start || expected_end != token.utf16_end {
            return Err("invalid UTF-16 token source spans".into());
        }
        last = end;
        utf16_end = expected_end;
    }
    if last != text.len() {
        return Err("language tokens do not cover the complete source text".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_os = "macos", target_pointer_width = "64"))]
    #[test]
    fn pinned_mecab_node_c_layout_is_explicit() {
        assert_eq!(std::mem::size_of::<Node>(), 112);
        assert_eq!(std::mem::offset_of!(Node, surface), 48);
        assert_eq!(std::mem::offset_of!(Node, feature), 56);
        assert_eq!(std::mem::offset_of!(Node, length), 68);
        assert_eq!(std::mem::offset_of!(Node, stat), 79);
        assert_eq!(std::mem::offset_of!(Node, cost), 104);
    }

    #[test]
    fn parses_real_unidic_quoted_csv() {
        assert_eq!(
            parse_csv("名詞,普通名詞,*,*,\"a,b\",\"a\"\"b\"").unwrap(),
            vec!["名詞", "普通名詞", "*", "*", "a,b", "a\"b"]
        );
        assert!(parse_csv("\"unfinished").is_err());
    }
    #[test]
    fn ruby_normalization_preserves_long_marks() {
        assert_eq!(hiragana("トショカン・スーパー"), "としょかん・すーぱー");
    }
    #[test]
    fn actual_bundled_mecab_has_unicode_spans_and_real_lexical_fields() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
        let mut engine = Mecab::open(&root).expect("reviewed offline MeCab+UniDic resources");
        let text = "😀 放課後、図書館で彼女と待ち合わせをした。";
        let tokens = engine.analyze(text).unwrap();
        validate_spans(text, &tokens).unwrap();
        assert!(
            tokens
                .iter()
                .any(|t| t.surface == "彼女" && t.reading == "かのじょ" && t.lemma == "彼女")
        );
        let start = text.find("図書館").unwrap() as u32;
        let compound = tokens
            .iter()
            .filter(|t| t.byte_start >= start && t.byte_end <= start + "図書館".len() as u32)
            .collect::<Vec<_>>();
        assert_eq!(
            compound
                .iter()
                .map(|t| t.surface.as_str())
                .collect::<String>(),
            "図書館"
        );
        assert_eq!(
            compound
                .iter()
                .map(|t| t.reading.as_str())
                .collect::<String>(),
            "としょかん"
        );
        assert!(
            tokens
                .iter()
                .any(|t| t.surface == "彼女" && t.utf16_start < t.byte_start)
        );
        let unknown = engine.analyze("zzqvunregisteredtoken").unwrap();
        assert!(
            unknown.iter().any(|token| !token.known
                && token.lemma == token.surface
                && token.reading.is_empty())
        );
        assert!(engine.analyze("bad\0input").is_err());
        assert!(engine.analyze(&"あ".repeat(MAX_TEXT_BYTES)).is_err());
    }
    #[test]
    fn real_mecab_preserves_multiline_and_unicode_whitespace_losslessly() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
        let mut engine = Mecab::open(&root).expect("real bundled MeCab and UniDic");
        for source in [
            " 彼女は図書館へ行った。 ",
            "彼女\tは本を読んだ。",
            "彼女は図書館へ。\n本を読んだ。",
            "彼女は図書館へ。\r\n本を読んだ。",
            "　彼女　は図書館へ。　",
            " \t\n\r\n　彼女😀と図書館へ行った。\t  \r\n　",
            "\n\r\n \t　",
            "彼女は\\nという二文字を読んだ。",
            "",
        ] {
            let tokens = engine.analyze(source).expect("real whitespace analysis");
            validate_spans(source, &tokens).expect("complete reversible UTF-8 and UTF-16 spans");
            assert_eq!(
                tokens
                    .iter()
                    .map(|token| token.surface.as_str())
                    .collect::<String>(),
                source
            );
            for token in tokens
                .iter()
                .filter(|token| token.surface.chars().all(char::is_whitespace))
            {
                assert!(token.reading.is_empty());
                assert!(!token.known);
                assert_eq!(token.part_of_speech, "whitespace");
            }
            if source.contains("彼女") {
                assert!(
                    tokens
                        .iter()
                        .any(|token| token.surface == "彼女" && token.reading == "かのじょ")
                );
            }
        }
    }

    #[test]
    fn whitespace_gaps_cannot_hide_text_or_invalid_boundaries() {
        assert!(whitespace_span("a b", 0, 1, 0).is_err());
        assert!(whitespace_span("　", 1, 3, 0).is_err());
        assert!(whitespace_span(" ", 0, 0, 0).is_err());
        let first = whitespace_span(" \n", 0, 1, 0).unwrap();
        assert!(
            validate_spans(" \n", &[first]).is_err(),
            "a missing trailing byte must fail"
        );
        let second = whitespace_span(" \n", 1, 2, 1).unwrap();
        assert!(
            validate_spans(" \n", &[second]).is_err(),
            "a missing leading byte must fail"
        );
    }
}
