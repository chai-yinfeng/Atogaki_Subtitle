use std::{
    collections::HashMap,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use atogaki_subtitle::{
    application::LocalLearningService,
    infrastructure::local_db::{
        LocalLearningItemDetail, LocalLearningLookupSense, NewLocalLearningLookupResult,
    },
};
use flate2::read::GzDecoder;
use serde::Deserialize;
use serde_json::Value;
use sqlx::{
    ConnectOptions, QueryBuilder, Row, Sqlite,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use tokio::sync::Mutex;

use crate::desktop_settings::DesktopSettingsService;

const INDEX_FILE: &str = "atogaki-dictionary-index.sqlite";

#[derive(Clone)]
pub struct DictionaryLookupService {
    directory: PathBuf,
    learning: LocalLearningService,
    settings: DesktopSettingsService,
    index_lock: Arc<Mutex<()>>,
}

impl DictionaryLookupService {
    pub fn new(
        directory: PathBuf,
        learning: LocalLearningService,
        settings: DesktopSettingsService,
    ) -> Self {
        Self {
            directory,
            learning,
            settings,
            index_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn lookup(
        &self,
        item_id: &str,
        provider_id: &str,
    ) -> Result<LocalLearningItemDetail> {
        let detail = self
            .learning
            .get_item(item_id)
            .await?
            .ok_or_else(|| anyhow!("学习条目不存在：{item_id}"))?;
        if detail.item.item_type == "sentence" {
            bail!("整句收藏不支持标准词典查询");
        }
        let query = detail.item.source_text.trim();
        if query.is_empty() {
            bail!("学习条目没有可查询的文本");
        }
        let result = match provider_id {
            "jmdict" => {
                ensure_language(&detail, "ja", "JMdict")?;
                self.lookup_jmdict(item_id, query).await?
            }
            "tomoshi" => {
                ensure_language(&detail, "ja", "Tomoshi")?;
                self.lookup_tomoshi(item_id, query).await?
            }
            "ecdict" => {
                ensure_language(&detail, "en", "ECDICT")?;
                self.lookup_ecdict(item_id, query).await?
            }
            "merriam-webster" => {
                ensure_language(&detail, "en", "Merriam-Webster")?;
                self.lookup_merriam_webster(item_id, query).await?
            }
            "collins" => bail!("该来源目前仅保留配置入口，尚未接入查询协议"),
            _ => bail!("不支持的词典来源：{provider_id}"),
        };
        self.learning.upsert_lookup_result(result).await
    }

    async fn lookup_jmdict(
        &self,
        item_id: &str,
        query: &str,
    ) -> Result<NewLocalLearningLookupResult> {
        let source = self.directory.join("jmdict-eng.json.tgz");
        require_file(&source, "JMdict")?;
        let version = package_version(&source);
        self.ensure_index("jmdict", &version, &source).await?;
        let entry = self.query_index("jmdict", query).await?
            .ok_or_else(|| anyhow!("JMdict 中没有找到“{query}”的精确词形"))?;
        Ok(entry.into_lookup(
            item_id,
            "jmdict",
            "JMdict",
            "JMdict / Electronic Dictionary Research and Development Group",
            Some("https://www.edrdg.org/jmdict/j_jmdict.html"),
            Some("CC BY-SA 4.0"),
            Some(version),
        ))
    }

    async fn lookup_ecdict(
        &self,
        item_id: &str,
        query: &str,
    ) -> Result<NewLocalLearningLookupResult> {
        let source = self.directory.join("ecdict.csv");
        require_file(&source, "ECDICT 基础英中")?;
        let version = package_version(&source);
        self.ensure_index("ecdict", &version, &source).await?;
        let candidates = english_lemma_candidates(query);
        let entry = self
            .query_index_candidates("ecdict", &candidates)
            .await?
            .ok_or_else(|| anyhow!("ECDICT 中没有找到“{query}”或其英语词形对应的词头"))?;
        Ok(entry.into_lookup(
            item_id,
            "ecdict",
            "ECDICT 基础英中",
            "ECDICT contributors / Linwei (skywind3000)",
            Some("https://github.com/skywind3000/ECDICT"),
            Some("MIT（上游声明；混合历史数据来源见项目 README）"),
            Some(version),
        ))
    }

    async fn ensure_index(&self, provider: &str, version: &str, source: &Path) -> Result<()> {
        let _guard = self.index_lock.lock().await;
        let pool = open_index(&self.directory.join(INDEX_FILE)).await?;
        create_index_schema(&pool).await?;
        let indexed_version = sqlx::query_scalar::<_, String>(
            "SELECT source_version FROM dictionary_index_meta WHERE provider_id = ?",
        )
        .bind(provider)
        .fetch_optional(&pool)
        .await?;
        if indexed_version.as_deref() == Some(version) {
            pool.close().await;
            return Ok(());
        }
        if provider == "ecdict" {
            replace_ecdict_index(&pool, provider, version, source).await?;
            pool.close().await;
            return Ok(());
        }
        let source = source.to_path_buf();
        let provider_owned = provider.to_string();
        let entries = tokio::task::spawn_blocking(move || match provider_owned.as_str() {
            "jmdict" => parse_jmdict(&source),
            _ => bail!("unsupported index provider"),
        })
        .await
        .context("词典索引任务异常结束")??;
        replace_index(&pool, provider, version, entries).await?;
        pool.close().await;
        Ok(())
    }

    async fn query_index(&self, provider: &str, query: &str) -> Result<Option<IndexedEntry>> {
        self.query_index_candidates(provider, &[query.to_string()])
            .await
    }

    async fn query_index_candidates(
        &self,
        provider: &str,
        queries: &[String],
    ) -> Result<Option<IndexedEntry>> {
        let pool = open_index(&self.directory.join(INDEX_FILE)).await?;
        create_index_schema(&pool).await?;
        let mut row = None;
        for query in queries {
            row = sqlx::query(
                "SELECT e.entry_id, e.headword, e.reading, e.senses_json
                 FROM dictionary_forms f
                 JOIN dictionary_entries e
                   ON e.provider_id = f.provider_id AND e.entry_id = f.entry_id
                 WHERE f.provider_id = ? AND f.form = ? COLLATE NOCASE
                 ORDER BY f.rank ASC LIMIT 1",
            )
            .bind(provider)
            .bind(query)
            .fetch_optional(&pool)
            .await?;
            if row.is_some() {
                break;
            }
        }
        pool.close().await;
        row.map(|row| {
            Ok(IndexedEntry {
                id: row.try_get("entry_id")?,
                headword: row.try_get("headword")?,
                reading: row.try_get("reading")?,
                senses: serde_json::from_str(&row.try_get::<String, _>("senses_json")?)?,
                forms: Vec::new(),
            })
        })
        .transpose()
    }

    async fn lookup_tomoshi(
        &self,
        item_id: &str,
        query: &str,
    ) -> Result<NewLocalLearningLookupResult> {
        let compressed = self.directory.join("tomoshi-dict-open.db.zst");
        require_file(&compressed, "Tomoshi")?;
        let version = package_version(&compressed);
        let database = self.ensure_tomoshi_database(&compressed, &version).await?;
        let options = SqliteConnectOptions::new()
            .filename(&database)
            .read_only(true)
            .create_if_missing(false)
            .disable_statement_logging();
        let pool = SqlitePoolOptions::new().max_connections(1).connect_with(options).await
            .with_context(|| format!("无法打开 Tomoshi 数据库 {}", database.display()))?;
        let rows = sqlx::query(
            "SELECT e.id, e.data, z.data AS zh_data
             FROM forms f JOIN entries e ON e.id = f.entry_id
             LEFT JOIN zh_defs z ON z.entry_id = e.id AND z.locale = 'zh-CN'
             WHERE f.text = ? ORDER BY f.is_common DESC, e.is_common DESC LIMIT 8",
        )
        .bind(query)
        .fetch_all(&pool)
        .await?;
        pool.close().await;
        let row = rows.first().ok_or_else(|| anyhow!("Tomoshi 中没有找到“{query}”的精确词形"))?;
        let data: TomoshiEntry = serde_json::from_str(row.try_get("data")?)?;
        let zh_data = row.try_get::<Option<String>, _>("zh_data")?
            .and_then(|value| serde_json::from_str::<Value>(&value).ok());
        let senses = tomoshi_senses(&data, zh_data.as_ref());
        if senses.is_empty() {
            bail!("Tomoshi 返回的词条没有可显示的中文释义");
        }
        Ok(NewLocalLearningLookupResult {
            learning_item_id: item_id.to_string(),
            provider_id: "tomoshi".to_string(),
            provider_name: "Tomoshi".to_string(),
            headword: data.kanji.first().map(|item| item.text.clone())
                .or_else(|| data.kana.first().map(|item| item.text.clone()))
                .unwrap_or_else(|| query.to_string()),
            reading: data.kana.first().map(|item| item.text.clone()),
            pronunciation: None,
            audio_url: None,
            senses,
            attribution_text: "Tomoshi (Y1Z)；JMdict / EDRDG 及开放数据贡献者".to_string(),
            source_url: Some("https://github.com/tomoshi-app/tomoshi-dict-data".to_string()),
            license_label: Some("查询所用 entries/forms/zh_defs：CC BY-SA 4.0".to_string()),
            data_version: Some(version),
            fetched_at_unix: now_unix(),
            cache_expires_at_unix: None,
        })
    }

    async fn ensure_tomoshi_database(&self, compressed: &Path, version: &str) -> Result<PathBuf> {
        let database = self.directory.join("tomoshi-dict-open.db");
        let version_file = database.with_extension("db.version");
        let current = std::fs::read_to_string(&version_file).ok();
        if database.is_file() && current.as_deref().map(str::trim) == Some(version) {
            return Ok(database);
        }
        if database.is_file() && current.is_none() {
            std::fs::write(&version_file, version)?;
            return Ok(database);
        }
        let source = compressed.to_path_buf();
        let destination = database.clone();
        let saved_version = version.to_string();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let part = destination.with_extension("db.part");
            let backup = destination.with_extension("db.backup");
            let mut decoder = zstd::stream::read::Decoder::new(File::open(&source)?)?;
            let mut output = File::create(&part)?;
            std::io::copy(&mut decoder, &mut output)?;
            output.flush()?;
            if backup.exists() {
                std::fs::remove_file(&backup)?;
            }
            if destination.exists() {
                std::fs::rename(&destination, &backup)?;
            }
            if let Err(error) = std::fs::rename(&part, &destination) {
                if backup.exists() {
                    let _ = std::fs::rename(&backup, &destination);
                }
                return Err(error).context("无法原子替换 Tomoshi 数据库");
            }
            if backup.exists() {
                std::fs::remove_file(&backup)?;
            }
            std::fs::write(destination.with_extension("db.version"), saved_version)?;
            Ok(())
        })
        .await
        .context("Tomoshi 解压任务异常结束")??;
        Ok(database)
    }

    async fn lookup_merriam_webster(
        &self,
        item_id: &str,
        query: &str,
    ) -> Result<NewLocalLearningLookupResult> {
        let key = self.settings.dictionary_api_key("merriam-webster")?;
        let network = self.settings.download_network_settings().await?;
        let client = network.client.apply(
            reqwest::Client::builder()
                .user_agent("Atogaki/0.1 dictionary lookup")
                .timeout(Duration::from_secs(45))
                .connect_timeout(Duration::from_secs(20))
                .read_timeout(Duration::from_secs(40)),
        )?.build()?;
        let preferred = self.settings.merriam_webster_reference().await?;
        let fallback = if preferred == "learners" { "collegiate" } else { "learners" };
        let mut selected_reference = None;
        let mut body = None;
        let mut subscription_error = None;
        for reference in [preferred.as_str(), fallback] {
            let response = client
                .get(format!(
                    "https://www.dictionaryapi.com/api/v3/references/{reference}/json/{}",
                    urlencoding::encode(query)
                ))
                .query(&[("key", key.as_str())])
                .send()
                .await
                .context("无法连接 Merriam-Webster")?;
            let status = response.status();
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("")
                .to_string();
            let response_body = response.text().await?;
            if response_body.contains("Not subscribed for this reference") {
                subscription_error = Some(response_body);
                continue;
            }
            if !status.is_success() {
                bail!("Merriam-Webster 请求失败（HTTP {status}）");
            }
            if !content_type.contains("json") {
                bail!("Merriam-Webster 返回了非 JSON 响应：{}", response_body.trim());
            }
            body = Some(
                serde_json::from_str::<Value>(&response_body)
                    .context("Merriam-Webster 返回了无效 JSON")?,
            );
            selected_reference = Some(reference.to_string());
            break;
        }
        let selected_reference = selected_reference.ok_or_else(|| {
            anyhow!(
                "当前 Key 不属于 Learner's 或 Collegiate Dictionary 产品：{}",
                subscription_error.unwrap_or_else(|| "未返回订阅信息".to_string())
            )
        })?;
        if selected_reference != preferred {
            self.settings
                .save_merriam_webster_reference(&selected_reference)
                .await?;
        }
        let body = body.expect("selected Merriam-Webster response has a body");
        let values = body.as_array().ok_or_else(|| anyhow!("Merriam-Webster 返回格式不正确"))?;
        if values.first().is_some_and(Value::is_string) {
            let suggestions = values.iter().filter_map(Value::as_str).take(6).collect::<Vec<_>>();
            bail!("Merriam-Webster 没有精确词条；建议：{}", suggestions.join("、"));
        }
        let entry = values.first().ok_or_else(|| anyhow!("Merriam-Webster 没有返回词条"))?;
        let headword = entry.pointer("/hwi/hw").and_then(Value::as_str)
            .or_else(|| entry.pointer("/meta/id").and_then(Value::as_str))
            .unwrap_or(query).replace('*', "");
        let pronunciation = entry.pointer("/hwi/prs/0/mw").and_then(Value::as_str).map(str::to_string);
        let audio = entry.pointer("/hwi/prs/0/sound/audio").and_then(Value::as_str).map(mw_audio_url);
        let part_of_speech = entry.get("fl").and_then(Value::as_str).map(str::to_string);
        let definitions = entry.get("shortdef").and_then(Value::as_array)
            .into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect::<Vec<_>>();
        if definitions.is_empty() {
            bail!("Merriam-Webster 返回的词条没有简明释义");
        }
        let examples = collect_mw_examples(entry);
        Ok(NewLocalLearningLookupResult {
            learning_item_id: item_id.to_string(),
            provider_id: "merriam-webster".to_string(),
            provider_name: if selected_reference == "learners" {
                "Merriam-Webster's Advanced Learner's English Dictionary".to_string()
            } else {
                "Merriam-Webster's Collegiate Dictionary".to_string()
            },
            headword,
            reading: None,
            pronunciation,
            audio_url: audio,
            senses: vec![LocalLearningLookupSense { part_of_speech, definitions, examples }],
            attribution_text: if selected_reference == "learners" {
                "Merriam-Webster's Advanced Learner's English Dictionary API".to_string()
            } else {
                "Merriam-Webster's Collegiate Dictionary API".to_string()
            },
            source_url: Some("https://www.merriam-webster.com/".to_string()),
            license_label: Some("内容版权归 Merriam-Webster 所有；按 API 条款使用".to_string()),
            data_version: Some(selected_reference),
            fetched_at_unix: now_unix(),
            cache_expires_at_unix: Some(now_unix() + 24 * 60 * 60),
        })
    }
}

fn ensure_language(detail: &LocalLearningItemDetail, expected: &str, provider: &str) -> Result<()> {
    if detail.item.source_language.to_string() != expected {
        bail!("{provider} 不支持当前条目的源语言");
    }
    Ok(())
}

fn require_file(path: &Path, name: &str) -> Result<()> {
    if !path.is_file() {
        bail!("{name} 词典包尚未安装；请先到设置 → 学习词典下载");
    }
    Ok(())
}

fn package_version(path: &Path) -> String {
    std::fs::read_to_string(format!("{}.version", path.display()))
        .ok().map(|value| value.trim().to_string()).filter(|value| !value.is_empty())
        .unwrap_or_else(|| "本地包".to_string())
}

fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
}

#[derive(Debug, Clone)]
struct IndexedEntry {
    id: String,
    headword: String,
    reading: Option<String>,
    senses: Vec<LocalLearningLookupSense>,
    forms: Vec<String>,
}

impl IndexedEntry {
    fn into_lookup(
        self,
        item_id: &str,
        provider_id: &str,
        provider_name: &str,
        attribution: &str,
        source_url: Option<&str>,
        license: Option<&str>,
        version: Option<String>,
    ) -> NewLocalLearningLookupResult {
        NewLocalLearningLookupResult {
            learning_item_id: item_id.to_string(), provider_id: provider_id.to_string(),
            provider_name: provider_name.to_string(), headword: self.headword,
            reading: self.reading, pronunciation: None, audio_url: None, senses: self.senses,
            attribution_text: attribution.to_string(), source_url: source_url.map(str::to_string),
            license_label: license.map(str::to_string), data_version: version,
            fetched_at_unix: now_unix(), cache_expires_at_unix: None,
        }
    }
}

async fn open_index(path: &Path) -> Result<sqlx::SqlitePool> {
    let options = SqliteConnectOptions::new().filename(path).create_if_missing(true);
    Ok(SqlitePoolOptions::new().max_connections(1).connect_with(options).await?)
}

async fn create_index_schema(pool: &sqlx::SqlitePool) -> Result<()> {
    sqlx::query("CREATE TABLE IF NOT EXISTS dictionary_entries (provider_id TEXT NOT NULL, entry_id TEXT NOT NULL, headword TEXT NOT NULL, reading TEXT, senses_json TEXT NOT NULL, PRIMARY KEY(provider_id, entry_id))").execute(pool).await?;
    sqlx::query("CREATE TABLE IF NOT EXISTS dictionary_forms (provider_id TEXT NOT NULL, form TEXT NOT NULL COLLATE NOCASE, entry_id TEXT NOT NULL, rank INTEGER NOT NULL, PRIMARY KEY(provider_id, form, entry_id))").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS dictionary_forms_lookup ON dictionary_forms(provider_id, form COLLATE NOCASE, rank)").execute(pool).await?;
    sqlx::query("CREATE TABLE IF NOT EXISTS dictionary_index_meta (provider_id TEXT PRIMARY KEY, source_version TEXT NOT NULL, completed_at_unix INTEGER NOT NULL)").execute(pool).await?;
    Ok(())
}

async fn replace_index(pool: &sqlx::SqlitePool, provider: &str, version: &str, entries: Vec<IndexedEntry>) -> Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM dictionary_forms WHERE provider_id = ?").bind(provider).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM dictionary_entries WHERE provider_id = ?").bind(provider).execute(&mut *tx).await?;
    for chunk in entries.chunks(200) {
        let mut builder = QueryBuilder::<Sqlite>::new("INSERT INTO dictionary_entries (provider_id, entry_id, headword, reading, senses_json) ");
        builder.push_values(chunk, |mut row, entry| {
            row.push_bind(provider).push_bind(&entry.id).push_bind(&entry.headword)
                .push_bind(&entry.reading).push_bind(serde_json::to_string(&entry.senses).expect("serializable senses"));
        });
        builder.build().execute(&mut *tx).await?;
        let forms = chunk.iter().flat_map(|entry| entry.forms.iter().enumerate().map(move |(rank, form)| (entry, rank, form))).collect::<Vec<_>>();
        for form_chunk in forms.chunks(300) {
            let mut form_builder = QueryBuilder::<Sqlite>::new("INSERT OR IGNORE INTO dictionary_forms (provider_id, form, entry_id, rank) ");
            form_builder.push_values(form_chunk, |mut row, (entry, rank, form)| {
                row.push_bind(provider).push_bind(*form).push_bind(&entry.id).push_bind(*rank as i64);
            });
            form_builder.build().execute(&mut *tx).await?;
        }
    }
    sqlx::query("INSERT INTO dictionary_index_meta (provider_id, source_version, completed_at_unix) VALUES (?, ?, ?) ON CONFLICT(provider_id) DO UPDATE SET source_version=excluded.source_version, completed_at_unix=excluded.completed_at_unix")
        .bind(provider).bind(version).bind(now_unix()).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

async fn replace_ecdict_index(
    pool: &sqlx::SqlitePool,
    provider: &str,
    version: &str,
    source: &Path,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM dictionary_forms WHERE provider_id = ?")
        .bind(provider)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM dictionary_entries WHERE provider_id = ?")
        .bind(provider)
        .execute(&mut *tx)
        .await?;
    let mut reader = csv::ReaderBuilder::new().flexible(true).from_path(source)?;
    let mut entries = Vec::with_capacity(200);
    let mut indexed_entries = 0_usize;
    for (index, row) in reader.deserialize::<EcdictRow>().enumerate() {
        let row = row.with_context(|| format!("ECDICT CSV 第 {} 行无法解析", index + 2))?;
        if let Some(entry) = ecdict_entry(index, row) {
            entries.push(entry);
        }
        if entries.len() == 200 {
            insert_index_entries(&mut tx, provider, &entries).await?;
            indexed_entries += entries.len();
            entries.clear();
        }
    }
    if !entries.is_empty() {
        insert_index_entries(&mut tx, provider, &entries).await?;
        indexed_entries += entries.len();
    }
    if indexed_entries == 0 {
        bail!("ECDICT CSV 没有可用词条");
    }
    sqlx::query("INSERT INTO dictionary_index_meta (provider_id, source_version, completed_at_unix) VALUES (?, ?, ?) ON CONFLICT(provider_id) DO UPDATE SET source_version=excluded.source_version, completed_at_unix=excluded.completed_at_unix")
        .bind(provider).bind(version).bind(now_unix()).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

async fn insert_index_entries(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    provider: &str,
    entries: &[IndexedEntry],
) -> Result<()> {
    let mut builder = QueryBuilder::<Sqlite>::new("INSERT INTO dictionary_entries (provider_id, entry_id, headword, reading, senses_json) ");
    builder.push_values(entries, |mut row, entry| {
        row.push_bind(provider).push_bind(&entry.id).push_bind(&entry.headword)
            .push_bind(&entry.reading).push_bind(serde_json::to_string(&entry.senses).expect("serializable senses"));
    });
    builder.build().execute(&mut **tx).await?;
    let forms = entries.iter().flat_map(|entry| entry.forms.iter().enumerate().map(move |(rank, form)| (entry, rank, form))).collect::<Vec<_>>();
    for chunk in forms.chunks(300) {
        let mut builder = QueryBuilder::<Sqlite>::new("INSERT OR IGNORE INTO dictionary_forms (provider_id, form, entry_id, rank) ");
        builder.push_values(chunk, |mut row, (entry, rank, form)| {
            row.push_bind(provider).push_bind(*form).push_bind(&entry.id).push_bind(*rank as i64);
        });
        builder.build().execute(&mut **tx).await?;
    }
    Ok(())
}

#[derive(Deserialize)]
struct JmRoot { tags: HashMap<String, String>, words: Vec<JmWord> }
#[derive(Deserialize)]
struct JmWord { id: String, kanji: Vec<JmForm>, kana: Vec<JmForm>, sense: Vec<JmSense> }
#[derive(Deserialize)]
struct JmForm { text: String, #[serde(default)] common: bool }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JmSense { #[serde(default)] part_of_speech: Vec<String>, gloss: Vec<JmGloss> }
#[derive(Deserialize)]
struct JmGloss { lang: String, text: String }

fn parse_jmdict(path: &Path) -> Result<Vec<IndexedEntry>> {
    let decoder = GzDecoder::new(File::open(path)?);
    let mut archive = tar::Archive::new(decoder);
    let mut root = None;
    for file in archive.entries()? {
        let mut file = file?;
        if file.path()?.extension().is_some_and(|extension| extension == "json") {
            root = Some(serde_json::from_reader::<_, JmRoot>(&mut file)?);
            break;
        }
    }
    let root = root.ok_or_else(|| anyhow!("JMdict 压缩包中没有 JSON 数据"))?;
    Ok(root.words.into_iter().filter_map(|word| {
        let headword = word.kanji.iter().find(|form| form.common).or(word.kanji.first())
            .or_else(|| word.kana.iter().find(|form| form.common)).or(word.kana.first())?.text.clone();
        let reading = word.kana.iter().find(|form| form.common).or(word.kana.first()).map(|form| form.text.clone());
        let senses = word.sense.into_iter().filter_map(|sense| {
            let definitions = sense.gloss.into_iter().filter(|gloss| gloss.lang == "eng").map(|gloss| gloss.text).collect::<Vec<_>>();
            if definitions.is_empty() { return None; }
            let pos = sense.part_of_speech.iter().filter_map(|tag| root.tags.get(tag)).cloned().collect::<Vec<_>>();
            Some(LocalLearningLookupSense { part_of_speech: (!pos.is_empty()).then(|| pos.join(" / ")), definitions, examples: Vec::new() })
        }).collect::<Vec<_>>();
        if senses.is_empty() { return None; }
        let mut forms = word.kanji.into_iter().chain(word.kana).map(|form| form.text).collect::<Vec<_>>();
        forms.sort(); forms.dedup();
        Some(IndexedEntry { id: word.id, headword, reading, senses, forms })
    }).collect())
}

#[derive(Deserialize)]
struct EcdictRow {
    word: String,
    #[serde(default)]
    phonetic: String,
    #[serde(default)]
    definition: String,
    #[serde(default)]
    translation: String,
    #[serde(default)]
    pos: String,
    #[serde(default)]
    exchange: String,
}

#[cfg(test)]
fn parse_ecdict(path: &Path) -> Result<Vec<IndexedEntry>> {
    let mut reader = csv::ReaderBuilder::new().flexible(true).from_path(path)?;
    let mut entries = Vec::new();
    for (index, row) in reader.deserialize::<EcdictRow>().enumerate() {
        let row = row.with_context(|| format!("ECDICT CSV 第 {} 行无法解析", index + 2))?;
        if let Some(entry) = ecdict_entry(index, row) {
            entries.push(entry);
        }
    }
    if entries.is_empty() {
        bail!("ECDICT CSV 没有可用词条");
    }
    Ok(entries)
}

fn ecdict_entry(index: usize, row: EcdictRow) -> Option<IndexedEntry> {
    let headword = row.word.trim();
    if headword.is_empty() {
        return None;
    }
    let mut definitions = split_dictionary_lines(&row.translation);
    if definitions.is_empty() {
        definitions = split_dictionary_lines(&row.definition);
    }
    if definitions.is_empty() {
        return None;
    }
    let mut forms = vec![headword.to_string()];
    for exchange in row.exchange.split('/') {
        if let Some((_, value)) = exchange.split_once(':') {
            for form in value.split(',').map(str::trim).filter(|value| !value.is_empty()) {
                if !forms.iter().any(|candidate| candidate.eq_ignore_ascii_case(form)) {
                    forms.push(form.to_string());
                }
            }
        }
    }
    Some(IndexedEntry {
        id: index.to_string(),
        headword: headword.to_string(),
        reading: (!row.phonetic.trim().is_empty()).then(|| row.phonetic.trim().to_string()),
        senses: vec![LocalLearningLookupSense {
            part_of_speech: (!row.pos.trim().is_empty()).then(|| row.pos.trim().to_string()),
            definitions,
            examples: Vec::new(),
        }],
        forms,
    })
}

fn split_dictionary_lines(value: &str) -> Vec<String> {
    value
        .lines()
        .flat_map(|line| line.split('；'))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

#[derive(Deserialize)]
struct TomoshiEntry { #[serde(default)] kanji: Vec<TomoshiForm>, #[serde(default)] kana: Vec<TomoshiForm>, senses: Vec<TomoshiSense> }
#[derive(Deserialize)] struct TomoshiForm { text: String }
#[derive(Deserialize)] struct TomoshiSense { #[serde(default)] pos: Vec<String>, #[serde(default)] glosses: Vec<TomoshiGloss> }
#[derive(Deserialize)] struct TomoshiGloss { text: String, lang: String }

fn tomoshi_senses(entry: &TomoshiEntry, zh_data: Option<&Value>) -> Vec<LocalLearningLookupSense> {
    entry.senses.iter().enumerate().filter_map(|(index, sense)| {
        let supplemental = zh_data.and_then(|value| value.pointer(&format!("/senses/{index}")));
        let mut definitions = supplemental.and_then(|value| value.get("glosses")).and_then(Value::as_array)
            .into_iter().flatten().filter_map(|value| value.get("text")).filter_map(Value::as_str).map(str::to_string).collect::<Vec<_>>();
        if definitions.is_empty() { definitions = sense.glosses.iter().filter(|gloss| gloss.lang == "zho").map(|gloss| gloss.text.clone()).collect(); }
        if definitions.is_empty() { return None; }
        let examples = supplemental.and_then(|value| value.get("examples")).and_then(Value::as_object)
            .into_iter().flat_map(|values| values.values()).filter_map(Value::as_str).map(str::to_string).collect();
        Some(LocalLearningLookupSense { part_of_speech: (!sense.pos.is_empty()).then(|| sense.pos.join(" / ")), definitions, examples })
    }).collect()
}

fn mw_audio_url(audio: &str) -> String {
    let directory = if audio.starts_with("bix") { "bix" } else if audio.starts_with("gg") { "gg" }
        else if audio.chars().next().is_some_and(|value| value.is_ascii_digit() || !value.is_ascii_alphabetic()) { "number" }
        else { &audio[..1] };
    format!("https://media.merriam-webster.com/audio/prons/en/us/mp3/{directory}/{audio}.mp3")
}

fn collect_mw_examples(value: &Value) -> Vec<String> {
    fn visit(value: &Value, examples: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                if let Some(items) = map.get("vis").and_then(Value::as_array) {
                    for item in items { if let Some(text) = item.get("t").and_then(Value::as_str) { examples.push(text.replace("{it}", "").replace("{/it}", "")); } }
                }
                for child in map.values() { visit(child, examples); }
            }
            Value::Array(items) => for item in items { visit(item, examples); },
            _ => {}
        }
    }
    let mut examples = Vec::new(); visit(value, &mut examples); examples.sort(); examples.dedup(); examples.truncate(6); examples
}

fn english_lemma_candidates(query: &str) -> Vec<String> {
    fn push(candidates: &mut Vec<String>, value: impl Into<String>) {
        let value = value.into();
        if value.len() >= 2 && !candidates.iter().any(|candidate| candidate == &value) {
            candidates.push(value);
        }
    }

    let original = query.trim();
    let mut candidates = Vec::new();
    if original.is_empty() {
        return candidates;
    }
    push(&mut candidates, original.to_string());
    let word = original.to_ascii_lowercase();
    push(&mut candidates, word.clone());
    if !word.chars().all(|character| character.is_ascii_alphabetic() || character == '\'') {
        return candidates;
    }
    if let Some(stem) = word.strip_suffix("'s") {
        push(&mut candidates, stem.to_string());
    }
    if let Some(stem) = word.strip_suffix("ies") {
        push(&mut candidates, format!("{stem}y"));
    }
    if let Some(stem) = word.strip_suffix("ied") {
        push(&mut candidates, format!("{stem}y"));
    }
    if let Some(stem) = word.strip_suffix("ves") {
        push(&mut candidates, format!("{stem}f"));
        push(&mut candidates, format!("{stem}fe"));
    }
    if let Some(stem) = word.strip_suffix('s') {
        push(&mut candidates, stem.to_string());
    }
    if let Some(stem) = word.strip_suffix("es") {
        push(&mut candidates, stem.to_string());
    }
    for suffix in ["ing", "ed", "er", "est"] {
        if let Some(stem) = word.strip_suffix(suffix) {
            push(&mut candidates, stem.to_string());
            let mut characters = stem.chars().rev();
            if let (Some(last), Some(previous)) = (characters.next(), characters.next()) {
                if last == previous && !matches!(last, 'a' | 'e' | 'i' | 'o' | 'u') {
                    push(&mut candidates, stem[..stem.len() - last.len_utf8()].to_string());
                }
            }
            push(&mut candidates, format!("{stem}e"));
        }
    }
    candidates
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use sqlx::Row;

    use crate::credential_store::{CredentialStore, SystemCredentialStore};

    use super::{
        INDEX_FILE, TomoshiEntry, create_index_schema, english_lemma_candidates, mw_audio_url,
        open_index, package_version, parse_ecdict, parse_jmdict, replace_index, tomoshi_senses,
    };

    #[test]
    fn builds_mw_audio_paths() {
        assert!(mw_audio_url("bixbite").contains("/bix/"));
        assert!(mw_audio_url("ggreen").contains("/gg/"));
        assert!(mw_audio_url("apple01").contains("/a/"));
    }

    #[test]
    fn derives_conservative_english_dictionary_headwords() {
        assert!(english_lemma_candidates("studies").contains(&"study".to_string()));
        assert!(english_lemma_candidates("running").contains(&"run".to_string()));
        assert!(english_lemma_candidates("making").contains(&"make".to_string()));
        assert!(english_lemma_candidates("stopped").contains(&"stop".to_string()));
        assert!(english_lemma_candidates("boxes").contains(&"box".to_string()));
        assert_eq!(english_lemma_candidates("look after"), vec!["look after"]);
    }

    #[test]
    fn parses_ecdict_translations_and_exchange_forms() {
        let root = std::env::temp_dir().join(format!("atogaki-ecdict-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("ecdict.csv");
        std::fs::write(
            &source,
            "word,phonetic,definition,translation,pos,exchange\n\
             gadget,ˈɡædʒɪt,a small device,小器具；小装置,n:100,s:gadgets\n\
             perceived,pəˈsiːvd,,感知到的,v:100,0:perceive\n",
        )
        .unwrap();
        let entries = parse_ecdict(&source).unwrap();
        let gadget = entries.iter().find(|entry| entry.headword == "gadget").unwrap();
        assert!(gadget.forms.contains(&"gadgets".to_string()));
        assert_eq!(gadget.senses[0].definitions, vec!["小器具", "小装置"]);
        let perceived = entries.iter().find(|entry| entry.headword == "perceived").unwrap();
        assert!(perceived.forms.contains(&"perceive".to_string()));
        std::fs::remove_dir_all(root).unwrap();
    }

    /// Manual regression hook for real downloaded packages; CI does not own these large files.
    #[test]
    #[ignore = "requires ATOGAKI_DICTIONARY_SMOKE_DIR with installed packages"]
    fn parses_installed_dictionary_packages() {
        let root = PathBuf::from(
            std::env::var("ATOGAKI_DICTIONARY_SMOKE_DIR")
                .expect("ATOGAKI_DICTIONARY_SMOKE_DIR is required"),
        );
        let jmdict = parse_jmdict(&root.join("jmdict-eng.json.tgz")).unwrap();
        assert!(
            jmdict
                .iter()
                .any(|entry| entry.forms.iter().any(|form| form == "勉強"))
        );
    }

    /// Builds the persistent App index and checks representative exact-form lookups.
    #[test]
    #[ignore = "requires ATOGAKI_DICTIONARY_SMOKE_DIR with installed packages"]
    fn indexes_installed_dictionary_packages() {
        let root = PathBuf::from(
            std::env::var("ATOGAKI_DICTIONARY_SMOKE_DIR")
                .expect("ATOGAKI_DICTIONARY_SMOKE_DIR is required"),
        );
        tauri::async_runtime::block_on(async {
            let pool = open_index(&root.join(INDEX_FILE)).await.unwrap();
            create_index_schema(&pool).await.unwrap();
            let source = root.join("jmdict-eng.json.tgz");
            replace_index(
                &pool,
                "jmdict",
                &package_version(&source),
                parse_jmdict(&source).unwrap(),
            )
            .await
            .unwrap();
            let japanese = sqlx::query_scalar::<_, String>(
                "SELECT e.headword FROM dictionary_forms f JOIN dictionary_entries e ON e.provider_id=f.provider_id AND e.entry_id=f.entry_id WHERE f.provider_id='jmdict' AND f.form='勉強' LIMIT 1",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            assert!(!japanese.is_empty());
            pool.close().await;
        });
    }

    #[test]
    #[ignore = "requires ATOGAKI_DICTIONARY_SMOKE_DIR with decompressed Tomoshi database"]
    fn queries_installed_tomoshi_database() {
        let root = PathBuf::from(
            std::env::var("ATOGAKI_DICTIONARY_SMOKE_DIR")
                .expect("ATOGAKI_DICTIONARY_SMOKE_DIR is required"),
        );
        tauri::async_runtime::block_on(async {
            let options = sqlx::sqlite::SqliteConnectOptions::new()
                .filename(root.join("tomoshi-dict-open.db"))
                .read_only(true);
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .unwrap();
            let row = sqlx::query(
                "SELECT e.data, z.data AS zh_data FROM forms f JOIN entries e ON e.id=f.entry_id LEFT JOIN zh_defs z ON z.entry_id=e.id AND z.locale='zh-CN' WHERE f.text='勉強' LIMIT 1",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            let entry: TomoshiEntry = serde_json::from_str(row.get("data")).unwrap();
            let zh_data: serde_json::Value =
                serde_json::from_str(row.get("zh_data")).unwrap();
            let senses = tomoshi_senses(&entry, Some(&zh_data));
            assert!(senses.iter().any(|sense| !sense.definitions.is_empty()));
            pool.close().await;
        });
    }

    #[test]
    #[ignore = "uses the saved Merriam-Webster key and consumes one API request"]
    fn queries_merriam_webster_with_saved_key() {
        let key = SystemCredentialStore
            .get("dictionary:merriam-webster")
            .unwrap()
            .expect("Merriam-Webster key is not configured");
        tauri::async_runtime::block_on(async {
            let reference = std::env::var("ATOGAKI_MW_REFERENCE")
                .unwrap_or_else(|_| "collegiate".to_string());
            let response = reqwest::Client::builder()
                .user_agent("Atogaki/0.1 dictionary smoke test")
                .build()
                .unwrap()
                .get(format!("https://www.dictionaryapi.com/api/v3/references/{reference}/json/apple"))
                .query(&[("key", key.as_str())])
                .send()
                .await
                .unwrap();
            assert!(response.status().is_success());
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("missing")
                .to_string();
            let body = response.text().await.unwrap();
            assert!(
                content_type.contains("json"),
                "unexpected content type {content_type}: {body}"
            );
            let value: serde_json::Value = serde_json::from_str(&body).unwrap();
            let entry = value.as_array().and_then(|items| items.first()).unwrap();
            assert!(entry.get("shortdef").and_then(|value| value.as_array()).is_some_and(|items| !items.is_empty()));
        });
    }
}
