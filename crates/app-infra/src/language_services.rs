use crate::{dictionary::Dictionary, language::LanguageClient};
use app_core::{contracts::*, ports::runtime::*};
use std::path::PathBuf;

pub struct OfflineLanguageServices {
    client: LanguageClient,
    dictionary: Result<Dictionary, String>,
}
impl OfflineLanguageServices {
    pub fn new(resources: PathBuf) -> Self {
        let dictionary = Dictionary::open(&resources);
        Self {
            client: LanguageClient::new(resources),
            dictionary,
        }
    }
}
impl LanguageServices for OfflineLanguageServices {
    fn analysis_revision(&self) -> &'static str {
        "mecab-unidic-v2-lossless-whitespace"
    }
    fn analyze<'a>(&'a self, text: &'a str) -> PortFuture<'a, Vec<LexicalToken>> {
        Box::pin(async move {
            self.client
                .analyze(text)
                .await
                .map_err(|message| AppError::new("language_worker_failed", "language", message))
        })
    }
    fn lookup(&self, token: &LexicalToken) -> AppResult<Vec<DictionaryEntry>> {
        self.dictionary
            .as_ref()
            .map_err(|_| {
                AppError::new(
                    "dictionary_unavailable",
                    "dictionary",
                    "随包词典核验失败，请重新安装完整客户端",
                )
            })?
            .lookup(&token.lemma, &token.surface, &token.reading)
            .map_err(|_| AppError::new("dictionary_query_failed", "dictionary", "本地词典查询失败"))
    }
    fn resources(&self) -> Vec<ResourceInfo> {
        vec![
            self.client.resource_info(),
            match &self.dictionary {
                Ok(dictionary) => dictionary.resource_info(),
                Err(_) => ResourceInfo {
                    kind: "dictionary".into(),
                    version: String::new(),
                    manifest_sha256: String::new(),
                    available: false,
                    detail: "词典资源缺失或完整性校验失败，请重新安装".into(),
                },
            },
        ]
    }
}
