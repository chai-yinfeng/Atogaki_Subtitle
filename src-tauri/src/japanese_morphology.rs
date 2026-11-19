use std::{
    fs::{self, File},
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, ensure};
use sudachi::{
    analysis::{Mode, Tokenize, stateless_tokenizer::StatelessTokenizer},
    config::Config,
    dic::dictionary::JapaneseDictionary,
    prelude::MorphemeView,
};

use crate::dictionary_download::SUDACHI_ARCHIVE;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Word {
    pub start: usize,
    pub end: usize,
    pub lemma: String,
    pub reading: String,
    pub pos: String,
    pub unknown: bool,
}

#[derive(Default)]
pub(crate) struct JapaneseMorphology(Mutex<Option<Arc<JapaneseDictionary>>>);

impl JapaneseMorphology {
    pub(crate) fn words(&self, directory: &Path, text: &str) -> Result<Vec<Word>> {
        let dictionary = {
            let mut cache = self
                .0
                .lock()
                .map_err(|_| anyhow::anyhow!("日语分词初始化失败"))?;
            if let Some(dictionary) = cache.as_ref() {
                dictionary.clone()
            } else {
                let path = unpack_dictionary(directory)?;
                let config = Config::new_embedded()?.with_system_dic(path);
                let dictionary = Arc::new(JapaneseDictionary::from_cfg(&config)?);
                *cache = Some(dictionary.clone());
                dictionary
            }
        };

        let tokenizer = StatelessTokenizer::new(dictionary);
        let mut words = Vec::new();
        for mode in [Mode::C, Mode::A] {
            let list = tokenizer.tokenize(text, mode, false)?;
            for token in list.iter() {
                let start_byte = token.begin();
                let end_byte = token.end();
                let pos = token.part_of_speech()[0].as_str();
                if start_byte >= end_byte || matches!(pos, "補助記号" | "空白" | "記号") {
                    continue;
                }
                let reading = token
                    .dictionary_form_morpheme()
                    .ok()
                    .map(|base| base.reading_form().to_owned())
                    .filter(|reading| !reading.is_empty())
                    .unwrap_or_else(|| token.reading_form().to_owned());
                let word = Word {
                    start: text[..start_byte].encode_utf16().count(),
                    end: text[..end_byte].encode_utf16().count(),
                    lemma: token.dictionary_form().into(),
                    reading: hiragana(&reading),
                    pos: pos.into(),
                    unknown: token.is_oov(),
                };
                if !words.contains(&word) {
                    words.push(word);
                }
            }
        }
        Ok(words)
    }
}

fn unpack_dictionary(directory: &Path) -> Result<std::path::PathBuf> {
    let archive = File::open(directory.join(SUDACHI_ARCHIVE))
        .context("请先在设置 → 学习词典下载 Sudachi 日语分词")?;
    let path = directory.join("sudachi-core-20261015.dic");
    const EXPECTED_BYTES: u64 = 202_236_003;
    if fs::metadata(&path).is_ok_and(|metadata| metadata.len() == EXPECTED_BYTES) {
        return Ok(path);
    }

    let mut archive = zip::ZipArchive::new(archive).context("无法打开 Sudachi 词库")?;
    let mut entry = archive.by_name("sudachi-dictionary-20261015/system_core.dic")?;
    ensure!(entry.size() == EXPECTED_BYTES, "Sudachi 词库大小异常");
    let partial = directory.join(format!(".sudachi-{}.part", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut output = File::options()
            .write(true)
            .create_new(true)
            .open(&partial)?;
        let bytes = std::io::copy(&mut entry, &mut output)?;
        ensure!(bytes == EXPECTED_BYTES, "Sudachi 词库解压不完整");
        output.sync_all()?;
        drop(output);
        fs::rename(&partial, &path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(partial);
    }
    result?;
    Ok(path)
}

pub(crate) fn hiragana(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            'ァ'..='ヶ' | 'ヽ'..='ヾ' => char::from_u32(ch as u32 - 0x60).unwrap_or(ch),
            _ => ch,
        })
        .collect()
}
