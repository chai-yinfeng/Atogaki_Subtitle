#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct UserRecord {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub password_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct JobRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub local_job_id: String,
    pub status: String,
    pub message: String,
    pub storage_dir: String,
    pub input_file_id: Option<Uuid>,
    pub render_output_file_id: Option<Uuid>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct FileRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub job_id: Option<Uuid>,
    pub kind: String,
    pub path: String,
    pub original_name: String,
    pub mime_type: Option<String>,
    pub size_bytes: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct SubtitleSegmentRecord {
    pub id: Uuid,
    pub job_id: Uuid,
    pub segment_index: i32,
    pub start_ms: i64,
    pub end_ms: i64,
    pub ja_text: String,
    pub zh_text: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GlossaryRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GlossaryTermRecord {
    pub id: Uuid,
    pub glossary_id: Uuid,
    pub source_text: String,
    pub target_text: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct JobEventRecord {
    pub id: Uuid,
    pub job_id: Uuid,
    pub status: String,
    pub message: String,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}
