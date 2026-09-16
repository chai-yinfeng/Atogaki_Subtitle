use anyhow::Result;
use atogaki_subtitle::{
    application::{
        JobRunner, TranscriptionOptions, TranslationOptions,
        job_spec::{
            ApplyGlossarySpec, ExportSpec, ProcessSpec, RenderSpec, RerenderSpec, TranscribeSpec,
            TranslateSpec,
        },
        translate_transcript,
    },
    domain::TranscriptSegment,
    domain::render::RenderOptions,
    infrastructure::{
        config::AppConfig,
        media,
        network::NetworkClientConfig,
        openai_compatible::OpenAiGenerationConfig,
        openai_compatible::{OpenAiCompatibleConfig, OpenAiCompatibleTranslationProvider},
    },
    interface::{
        self,
        cli::{Cli, Command, RenderArgsCommon},
    },
};
use clap::Parser;
use std::{fs, time::Instant};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = AppConfig::from_cli(&cli);
    let runner = JobRunner::new(config.clone());

    match cli.command {
        Command::ApplyGlossary(args) => {
            let job = runner.apply_glossary(ApplyGlossarySpec {
                job_dir: args.job_dir,
                glossary: args.glossary,
                keep_translations: args.keep_translations,
            })?;
            println!("Glossary applied to {}", job.dir.display());
            Ok(())
        }
        Command::Devices => media::list_capture_devices(&config.ffmpeg).await,
        Command::EvaluateAsr(args) => {
            let report = atogaki_subtitle::application::evaluate_asr_files(
                &args.reference,
                &args.hypothesis,
            )?;
            let json = serde_json::to_string_pretty(&report)?;
            if let Some(output) = args.output {
                fs::write(&output, format!("{json}\n"))?;
            }
            println!("{json}");
            Ok(())
        }
        Command::EvaluateTranslation(args) => {
            let data = fs::read(&args.input)?;
            let mut segments: Vec<TranscriptSegment> = serde_json::from_slice(&data)?;
            for segment in &mut segments {
                segment.ensure_id();
                segment.set_translation(None);
            }
            let provider = OpenAiCompatibleTranslationProvider::with_network_config(
                OpenAiCompatibleConfig {
                    provider_id: "translation-evaluation".to_string(),
                    provider_name: args.provider_name,
                    api_key: Some(args.api_key),
                    base_url: args.base_url,
                    model: args.model,
                    style_instruction: args.style_instruction,
                    disable_deepseek_thinking: false,
                    generation: OpenAiGenerationConfig {
                        temperature: Some(0.7),
                        top_p: Some(0.6),
                        top_k: Some(20),
                        repetition_penalty: Some(1.05),
                        max_tokens: Some(2_048),
                        strict_json_schema: true,
                    },
                },
                &NetworkClientConfig::new("direct", None)?,
            )?;
            let options = TranslationOptions::new(args.source_language, args.target_language)
                .with_protected_terms(args.protected_terms);
            let started = Instant::now();
            let summary = translate_transcript(&provider, &options, &mut segments).await?;
            let output = serde_json::json!({
                "schema_version": 1,
                "elapsed_ms": i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX),
                "summary": summary,
                "options": options,
                "segments": segments,
            });
            if let Some(parent) = args.output.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(
                &args.output,
                format!("{}\n", serde_json::to_string_pretty(&output)?),
            )?;
            println!(
                "Translation evaluation written to {}",
                args.output.display()
            );
            Ok(())
        }
        Command::Record(args) => media::record_audio(&config.ffmpeg, &args).await,
        Command::Rerender(args) => {
            let output = args.output.clone();
            runner
                .rerender(RerenderSpec {
                    job_dir: args.job_dir,
                    input: args.input,
                    output: args.output,
                    render: render_options(args.render),
                })
                .await?;
            if let Some(output) = output {
                println!("Rendered {}", output.display());
            } else {
                println!("Rendered from saved job paths");
            }
            Ok(())
        }
        Command::Serve(args) => interface::web::serve(args).await,
        Command::Transcribe(args) => {
            let job = runner
                .transcribe(TranscribeSpec {
                    input: args.input,
                    output_dir: args.output_dir,
                    transcription: args.whisper.into(),
                })
                .await?;
            println!("Job written to {}", job.dir.display());
            Ok(())
        }
        Command::Translate(args) => {
            let job = runner
                .translate(TranslateSpec {
                    job_dir: args.job_dir,
                    deepl_auth_key: args.deepl_auth_key,
                    translation: TranslationOptions::new(
                        args.source_language,
                        args.target_language,
                    ),
                })
                .await?;
            println!("Translated subtitles written to {}", job.dir.display());
            Ok(())
        }
        Command::Export(args) => {
            let job = runner.export(ExportSpec {
                job_dir: args.job_dir,
            })?;
            println!("Subtitles written to {}", job.dir.display());
            Ok(())
        }
        Command::Render(args) => {
            let output = args.output.clone();
            runner
                .render(RenderSpec {
                    input: args.input,
                    job_dir: args.job_dir,
                    output,
                    render: render_options(args.render),
                })
                .await?;
            println!("Rendered {}", args.output.display());
            Ok(())
        }
        Command::Process(args) => {
            let transcription: TranscriptionOptions = args.whisper.into();
            let translation =
                TranslationOptions::new(transcription.source_language, args.target_language);
            let job = runner
                .process(ProcessSpec {
                    input: args.input,
                    output_dir: args.output_dir,
                    render_output: args.render_output,
                    deepl_auth_key: args.deepl_auth_key,
                    transcription,
                    translation,
                    render: render_options(args.render),
                })
                .await?;
            println!("Job written to {}", job.dir.display());
            Ok(())
        }
    }
}

fn render_options(args: RenderArgsCommon) -> RenderOptions {
    RenderOptions {
        video_crf: args.video_crf,
        video_preset: args.video_preset,
        target_video_bitrate_bps: None,
        soft_subtitles: args.soft_subtitles,
    }
}
