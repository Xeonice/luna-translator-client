//! Capture identities supplied by the verified HookBackend, never by UI text.
use crate::contracts::LexicalToken;

#[derive(Debug, Clone)]
pub struct ResourceSnapshotInput {
    pub snapshot_id: String,
    pub kind: String,
    pub version: String,
    pub manifest_sha256: String,
    pub contract_version: String,
    pub metadata_json: String,
}
#[derive(Debug, Clone)]
pub struct HookSessionInput {
    pub session_id: String,
    pub game_id: Option<String>,
    pub environment_key: String,
    pub target_exe_path: String,
    pub windows_pid: u32,
    pub process_created_u64: String,
    pub architecture: String,
    pub broker_instance_id: String,
    pub resource: ResourceSnapshotInput,
    pub target_identity_sha256: String,
}
#[derive(Debug, Clone)]
pub struct HookSourceInput {
    pub source_id: String,
    pub host_instance_id: String,
    pub hook_address_u64: String,
    pub context_u64: String,
    pub subcontext_u64: String,
    pub hook_code: String,
    pub hook_name: String,
    pub encoding: String,
    pub stream_identity_sha256: String,
    pub identity_extensions_json: String,
}
#[derive(Debug, Clone)]
pub struct HookTextInput {
    pub session_id: String,
    pub event_id: String,
    pub sequence_u64: String,
    pub source: HookSourceInput,
    pub text: String,
    pub source_language: String,
}
#[derive(Debug, Clone)]
pub struct TokenSnapshotInput {
    pub unit_id: String,
    pub text_revision: u32,
    pub analysis_revision: String,
    pub tokens: Vec<LexicalToken>,
    pub resource: ResourceSnapshotInput,
}

/// Internal capture policy; never inferred from a game name or source text.
pub const CAPTURE_IDLE_MS: i64 = 1_500;
pub const CAPTURE_MAX_AGE_MS: i64 = 6_000;
pub const CAPTURE_MAX_BYTES: usize = 65_536;
pub const CAPTURE_MAX_REVISIONS: u32 = 128;
pub const CAPTURE_MAX_REFS: usize = 128;
pub const CAPTURE_PROCESSING_REVISION_V1: &str = "hook-bounded-aggregate-v1";
// Historical V2 identity remains unchanged for stored facts and callers.
pub const CAPTURE_PROCESSING_REVISION: &str = "hook-bounded-continuation-v2";
pub const CAPTURE_PROCESSING_REVISION_V3: &str = "hook-bounded-quoted-growth-v3";

/// Versioned bounded heuristics, not a claim that Hook callbacks expose pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapturePolicy {
    V1,
    V2,
    V3,
}
impl CapturePolicy {
    pub const fn processing_revision(self) -> &'static str {
        match self {
            Self::V1 => CAPTURE_PROCESSING_REVISION_V1,
            Self::V2 => CAPTURE_PROCESSING_REVISION,
            Self::V3 => CAPTURE_PROCESSING_REVISION_V3,
        }
    }
    pub fn from_processing_revision(revision: &str) -> Option<Self> {
        match revision {
            CAPTURE_PROCESSING_REVISION_V1 => Some(Self::V1),
            CAPTURE_PROCESSING_REVISION => Some(Self::V2),
            CAPTURE_PROCESSING_REVISION_V3 => Some(Self::V3),
            _ => None,
        }
    }
    pub fn due(self, open: &OpenCapture<'_>, now: i64) -> bool {
        now.saturating_sub(open.created_at_ms) >= CAPTURE_MAX_AGE_MS
            || (now.saturating_sub(open.changed_at_ms) >= CAPTURE_IDLE_MS
                && (self == Self::V1 || self.ends_sentence(open.text)))
    }
    pub fn ends_sentence(self, text: &str) -> bool {
        match self {
            Self::V1 => text
                .trim_end()
                .trim_end_matches(['」', '』', '”', '’', '"', '\'', ')', '）', ']', '】'])
                .ends_with(['。', '！', '？', '!', '?', '.', '…']),
            Self::V2 | Self::V3 => balanced_terminal(text),
        }
    }
    pub fn fold(self, open: &OpenCapture<'_>, incoming: &str, now: i64) -> CaptureFold {
        // A distinct event with identical text may be a legitimate repeated line.
        // Only the repository may discard replay of the same event identity.
        if self.due(open, now)
            || open.text_revision >= CAPTURE_MAX_REVISIONS
            || incoming == open.text
            || open.text.starts_with(incoming)
        {
            return CaptureFold::NewUnit;
        }
        if incoming.starts_with(open.text) {
            return if incoming.len() <= CAPTURE_MAX_BYTES {
                CaptureFold::GrowingSnapshot
            } else {
                CaptureFold::NewUnit
            };
        }
        if self == Self::V3
            && let Some(prefix_bytes) = quoted_growth_prefix(open.text, incoming)
        {
            return if open.ref_count < CAPTURE_MAX_REFS
                && prefix_bytes
                    .checked_add(incoming.len())
                    .is_some_and(|len| len <= CAPTURE_MAX_BYTES)
            {
                CaptureFold::GrowingQuotedSnapshot { prefix_bytes }
            } else {
                CaptureFold::NewUnit
            };
        }
        if open.ref_count >= CAPTURE_MAX_REFS
            || open.text.len().saturating_add(incoming.len()) > CAPTURE_MAX_BYTES
        {
            CaptureFold::NewUnit
        } else {
            CaptureFold::Append
        }
    }
}

fn closing_for(c: char) -> Option<char> {
    match c {
        '「' => Some('」'),
        '『' => Some('』'),
        '“' => Some('”'),
        '‘' => Some('’'),
        '(' => Some(')'),
        '（' => Some('）'),
        '[' => Some(']'),
        '［' => Some('］'),
        '【' => Some('】'),
        '〈' => Some('〉'),
        '《' => Some('》'),
        '{' => Some('}'),
        '｛' => Some('｝'),
        '〔' => Some('〕'),
        _ => None,
    }
}
fn is_closing(c: char) -> bool {
    matches!(
        c,
        '」' | '』' | '”' | '’' | ')' | '）' | ']' | '］' | '】' | '〉' | '》' | '}' | '｝' | '〕'
    )
}
/// All supported delimiters must balance. ASCII double quotes toggle unless
/// backslash-escaped; ASCII apostrophes are ordinary text (e.g. `don't`).
/// A closing quote without an existing terminal mark does not complete a unit.
fn balanced_terminal(text: &str) -> bool {
    if !delimiter_state(text).is_some_and(|state| state.is_empty()) {
        return false;
    }
    text.trim_end_matches(|c: char| c.is_whitespace() || is_closing(c) || c == '"')
        .ends_with(['。', '！', '？', '!', '?', '.', '…'])
}

struct OpenDelimiter {
    close: char,
    byte_index: usize,
    is_quote: bool,
}
// Shared scanner: V2 completion semantics stay unchanged; V3 adds positions.
fn delimiter_state(text: &str) -> Option<Vec<OpenDelimiter>> {
    let mut expected: Vec<OpenDelimiter> = Vec::new();
    let mut escaped = false;
    for (byte_index, c) in text.char_indices() {
        if c == '"' && !escaped {
            if expected.last().is_some_and(|d| d.close == '"') {
                expected.pop();
            } else {
                expected.push(OpenDelimiter {
                    close: '"',
                    byte_index,
                    is_quote: true,
                });
            }
        } else if let Some(close) = closing_for(c) {
            expected.push(OpenDelimiter {
                close,
                byte_index,
                is_quote: matches!(c, '「' | '『' | '“' | '‘'),
            });
        } else if is_closing(c) && expected.pop().is_none_or(|d| d.close != c) {
            return None;
        }
        escaped = c == '\\' && !escaped;
    }
    Some(expected)
}

/// Narrow redraw heuristic: only one unmatched quote may anchor an entire
/// suffix which is a nontrivial strict prefix of the new snapshot. No general
/// overlap, normalization, bracket anchor, or page identity is inferred.
fn quoted_growth_prefix(old: &str, incoming: &str) -> Option<usize> {
    let state = delimiter_state(old)?;
    let [anchor] = state.as_slice() else {
        return None;
    };
    if !anchor.is_quote || !old.is_char_boundary(anchor.byte_index) {
        return None;
    }
    let suffix = old.get(anchor.byte_index..)?;
    if !suffix.chars().skip(1).any(|c| !c.is_whitespace())
        || incoming.len() <= suffix.len()
        || !incoming.starts_with(suffix)
    {
        return None;
    }
    // The prefix is already fully balanced: the scanner left exactly this one
    // quote. Validate the replacement candidate too, including closing order.
    delimiter_state(incoming)?;
    Some(anchor.byte_index)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureVersion {
    pub unit_id: String,
    pub text_revision: u32,
}
#[derive(Debug, Clone, Default)]
pub struct CaptureOutcome {
    pub visible_version: Option<CaptureVersion>,
    pub finalized_versions: Vec<CaptureVersion>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureFlushReason {
    Due,
    Shutdown,
}
#[derive(Debug, Clone)]
pub struct CaptureFlush {
    pub session_id: Option<String>,
    pub reason: CaptureFlushReason,
}
#[derive(Debug)]
pub struct OpenCapture<'a> {
    pub text: &'a str,
    pub created_at_ms: i64,
    pub changed_at_ms: i64,
    pub text_revision: u32,
    pub ref_count: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureFold {
    Append,
    GrowingSnapshot,
    GrowingQuotedSnapshot { prefix_bytes: usize },
    NewUnit,
}
#[cfg(test)]
mod aggregation_tests {
    use super::*;
    fn open(text: &str) -> OpenCapture<'_> {
        OpenCapture {
            text,
            created_at_ms: 0,
            changed_at_ms: 100,
            text_revision: 1,
            ref_count: 1,
        }
    }
    #[test]
    fn bounded_capture_distinguishes_fragments_growth_and_legitimate_repetition() {
        assert_eq!(
            CapturePolicy::V1.fold(&open("長い"), "文。", 1400),
            CaptureFold::Append
        );
        assert_eq!(
            CapturePolicy::V1.fold(&open("長い"), "長い文。", 1400),
            CaptureFold::GrowingSnapshot
        );
        assert_eq!(
            CapturePolicy::V1.fold(&open("長い"), "長い", 101),
            CaptureFold::NewUnit
        );
        assert_eq!(
            CapturePolicy::V1.fold(&open("長い文"), "長い", 101),
            CaptureFold::NewUnit
        );
        assert_eq!(
            CapturePolicy::V1.fold(&open("長い"), "文。", 1600),
            CaptureFold::NewUnit
        );
        assert!(CapturePolicy::V1.ends_sentence("「本当？」"));
        assert!(CapturePolicy::V1.ends_sentence("文。\n"));
        assert!(!CapturePolicy::V1.ends_sentence("続いている"));
    }
    #[test]
    fn capture_limits_include_age_revision_refs_and_utf8_bytes() {
        let mut state = open("😀");
        state.changed_at_ms = 5900;
        assert!(CapturePolicy::V1.due(&state, CAPTURE_MAX_AGE_MS));
        state.text_revision = CAPTURE_MAX_REVISIONS;
        assert_eq!(
            CapturePolicy::V1.fold(&state, "語", 5901),
            CaptureFold::NewUnit
        );
        state.text_revision = 1;
        state.ref_count = CAPTURE_MAX_REFS;
        assert_eq!(
            CapturePolicy::V1.fold(&state, "語", 5901),
            CaptureFold::NewUnit
        );
        state.ref_count = 1;
        let text = "語".repeat(CAPTURE_MAX_BYTES / 3);
        state.text = &text;
        assert_eq!(
            CapturePolicy::V1.fold(&state, "😀", 5901),
            CaptureFold::NewUnit
        );
    }
    #[test]
    fn v2_incomplete_continuation_ignores_idle_but_preserves_all_hard_limits() {
        let policy = CapturePolicy::V2;
        for now in [1499, 1500, 1599, 1600, 1601, 2000, 2500, 2999, 5999] {
            assert!(!policy.due(&open("長い😀"), now), "{now}");
            assert_eq!(
                policy.fold(&open("長い😀"), "文。", now),
                CaptureFold::Append
            );
            assert_eq!(
                policy.fold(&open("長い😀"), "長い😀文。", now),
                CaptureFold::GrowingSnapshot
            );
        }
        let mut state = open("😀");
        state.changed_at_ms = 5999;
        for now in [6000, 6001] {
            assert!(policy.due(&state, now));
            assert_eq!(policy.fold(&state, "語", now), CaptureFold::NewUnit);
        }
        state.text_revision = CAPTURE_MAX_REVISIONS;
        assert_eq!(policy.fold(&state, "語", 5999), CaptureFold::NewUnit);
        state.text_revision = 1;
        state.ref_count = CAPTURE_MAX_REFS;
        assert_eq!(policy.fold(&state, "語", 5999), CaptureFold::NewUnit);
        state.ref_count = 1;
        let text = "語".repeat(CAPTURE_MAX_BYTES / 3);
        state.text = &text;
        assert_eq!(policy.fold(&state, "😀", 5999), CaptureFold::NewUnit);
        assert_eq!(
            policy.fold(&open("同文"), "同文", 2500),
            CaptureFold::NewUnit
        );
        assert_eq!(
            policy.fold(&open("長い文"), "長い", 2500),
            CaptureFold::NewUnit
        );
        assert_eq!(
            policy.fold(&open("句中"), "句中引用", -1),
            CaptureFold::GrowingSnapshot
        );
        assert_eq!(
            CapturePolicy::from_processing_revision(CAPTURE_PROCESSING_REVISION_V1),
            Some(CapturePolicy::V1)
        );
        assert_eq!(
            CapturePolicy::from_processing_revision(CAPTURE_PROCESSING_REVISION),
            Some(policy)
        );
        assert_eq!(
            CapturePolicy::from_processing_revision("unknown-policy"),
            None
        );
    }
    #[test]
    fn v2_terminal_marks_require_balanced_supported_delimiters() {
        for text in [
            "文。\n",
            "「本当？」",
            "『「本当？」』",
            "“本当？”",
            "‘本当？’",
            "（文。）",
            "(文.)",
            "[文!]",
            "［文！］",
            "【文。】",
            "〈文。〉",
            "《文。》",
            "{文.}",
            "｛文。｝",
            "〔文。〕",
            "「文。  」\n",
            "don't stop.",
            "\"Say it.\"",
            "\"Say \\\"it\\\".\"",
            "😀…",
            "文。続き。",
        ] {
            assert!(
                CapturePolicy::V2.ends_sentence(text),
                "must complete: {text:?}"
            );
        }
        for text in [
            "未完",
            "「まだ続く。",
            "『引用…",
            "「『順序。』",
            "「文。』",
            "文。」",
            ")文。",
            "文。続く",
            "引用「語」",
            "「語」",
            "（文。",
            "\"Say it.",
            "\"word\"",
            "「文。』」",
            "文、",
            "文：",
        ] {
            assert!(
                !CapturePolicy::V2.ends_sentence(text),
                "must remain incomplete: {text:?}"
            );
        }
        // V1's historical permissive marker behavior is deliberately retained.
        assert!(CapturePolicy::V1.ends_sentence("「まだ続く。"));
        assert!(CapturePolicy::V1.ends_sentence("文。」"));
    }
    #[test]
    fn v3_only_unique_open_quote_strict_suffix_growth_preserves_the_prefix() {
        for (quote, close) in [
            ('「', '」'),
            ('『', '』'),
            ('“', '”'),
            ('‘', '’'),
            ('"', '"'),
        ] {
            let prefix = "【葵😀が】";
            let old = format!("{prefix}{quote}明日は");
            let incoming = format!("{quote}明日は晴れる{close}");
            let state = open(&old);
            assert_eq!(
                CapturePolicy::V2.fold(&state, &incoming, 2500),
                CaptureFold::Append
            );
            assert_eq!(
                CapturePolicy::V3.fold(&state, &incoming, 2500),
                CaptureFold::GrowingQuotedSnapshot {
                    prefix_bytes: prefix.len()
                }
            );
        }
        assert_eq!(
            CapturePolicy::V3.fold(&open("「語"), "「語の続き", 2500),
            CaptureFold::GrowingSnapshot
        );
        assert_eq!(
            CapturePolicy::from_processing_revision(CAPTURE_PROCESSING_REVISION_V3),
            Some(CapturePolicy::V3)
        );
    }
    #[test]
    fn v3_does_not_interpret_generic_overlap_mismatches_or_non_growth_as_snapshots() {
        for (old, incoming) in [
            ("名前abc", "abcdef"),
            ("名「", "「語"),
            ("名「  ", "「  語"),
            ("名「語", "「語"),
            ("名「語句", "「語"),
            ("名「語", "『語句"),
            ("名「『語", "「『語句"),
            ("名】「語", "「語句"),
            ("名（語", "（語句"),
            ("名「語", "「語句』"),
            ("名「語", "「語句）"),
            ("名「語」", "「語」句"),
            ("don't 語", "語句"),
        ] {
            assert_eq!(
                CapturePolicy::V3.fold(&open(old), incoming, 2500),
                CapturePolicy::V2.fold(&open(old), incoming, 2500),
                "{old:?} / {incoming:?}"
            );
        }
        let old = "名「『引用』の語";
        assert_eq!(
            CapturePolicy::V3.fold(&open(old), "「『引用』の語句", 2500),
            CaptureFold::GrowingQuotedSnapshot {
                prefix_bytes: "名".len()
            }
        );
        assert_eq!(
            CapturePolicy::V3.fold(&open("名\"a \\\"語\\\""), "\"a \\\"語\\\"句", 2500),
            CaptureFold::GrowingQuotedSnapshot {
                prefix_bytes: "名".len()
            }
        );
        assert_eq!(
            CapturePolicy::V3.fold(&open("名「語"), "名「語", 2500),
            CaptureFold::NewUnit
        );
        assert_eq!(
            CapturePolicy::V3.fold(&open("名「語句"), "名「語", 2500),
            CaptureFold::NewUnit
        );
    }
    #[test]
    fn v3_quoted_snapshot_uses_result_bytes_and_preserves_ref_revision_and_age_caps() {
        let prefix = "x".repeat(60_000);
        let suffix = format!("「{}", "語".repeat(1000));
        let old = format!("{prefix}{suffix}");
        let incoming = format!("{suffix}{}", "a".repeat(1000));
        assert!(old.len() + incoming.len() > CAPTURE_MAX_BYTES);
        assert!(prefix.len() + incoming.len() < CAPTURE_MAX_BYTES);
        let mut state = open(&old);
        state.ref_count = CAPTURE_MAX_REFS - 1;
        state.text_revision = CAPTURE_MAX_REVISIONS - 1;
        state.changed_at_ms = 5900;
        assert_eq!(
            CapturePolicy::V3.fold(&state, &incoming, 5999),
            CaptureFold::GrowingQuotedSnapshot {
                prefix_bytes: prefix.len()
            }
        );
        assert_eq!(
            CapturePolicy::V3.fold(&state, &incoming, 6000),
            CaptureFold::NewUnit
        );
        state.ref_count = CAPTURE_MAX_REFS;
        assert_eq!(
            CapturePolicy::V3.fold(&state, &incoming, 5999),
            CaptureFold::NewUnit
        );
        state.ref_count = 1;
        state.text_revision = CAPTURE_MAX_REVISIONS;
        assert_eq!(
            CapturePolicy::V3.fold(&state, &incoming, 5999),
            CaptureFold::NewUnit
        );
        state.text_revision = 1;
        let excessive = format!("{suffix}{}", "a".repeat(3000));
        assert_eq!(
            CapturePolicy::V3.fold(&state, &excessive, 5999),
            CaptureFold::NewUnit
        );
    }
}
