use anyhow::{Context, Result};
use axum::{Json, Router, response::Html, routing::get};
use serde::Serialize;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::interface::cli::ServeArgs;

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    database_configured: bool,
    jobs_dir: String,
    uploads_dir: String,
    whisper_model_configured: bool,
    vad_model_configured: bool,
    workers: usize,
}

pub async fn serve(args: ServeArgs) -> Result<()> {
    let bind = args.bind;
    let health_args = args.clone();
    let app = Router::new()
        .route("/", get(index))
        .route("/api/health", get(move || health(health_args.clone())))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(bind)
        .await
        .with_context(|| format!("failed to bind {bind}"))?;

    println!("Atogaki web server listening on http://{bind}");
    axum::serve(listener, app)
        .await
        .context("web server failed")
}

async fn index() -> Html<&'static str> {
    Html(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Atogaki Subtitle</title>
    <style>
      body {
        margin: 0;
        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
        background: #f6f7f9;
        color: #172026;
      }
      main {
        max-width: 760px;
        margin: 72px auto;
        padding: 0 24px;
      }
      h1 {
        margin: 0 0 12px;
        font-size: 32px;
      }
      p {
        margin: 0 0 20px;
        line-height: 1.6;
        color: #4c5963;
      }
      code {
        border-radius: 6px;
        background: #e8edf2;
        padding: 3px 6px;
      }
    </style>
  </head>
  <body>
    <main>
      <h1>Atogaki Subtitle</h1>
      <p>The Web API shell is running. The next batches will add Postgres, background jobs, authentication, and the subtitle editor.</p>
      <p>Health check: <code>/api/health</code></p>
    </main>
  </body>
</html>"#,
    )
}

async fn health(args: ServeArgs) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        database_configured: args.database_url.is_some(),
        jobs_dir: args.jobs_dir.display().to_string(),
        uploads_dir: args.uploads_dir.display().to_string(),
        whisper_model_configured: args.whisper_model.is_some(),
        vad_model_configured: args.vad_model.is_some(),
        workers: args.workers,
    })
}
