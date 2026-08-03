use anyhow::Result;
use atogaki_subtitle::{
    application::{
        JobRunner, TranscriptionOptions, TranslationOptions,
        job_spec::{
            ApplyGlossarySpec, ExportSpec, ProcessSpec, RenderSpec, RerenderSpec, TranscribeSpec,
            TranslateSpec,
        },
    },
    domain::render::RenderOptions,
    infrastructure::{config::AppConfig, media},
    interface::{
        self,
        cli::{Cli, Command, RenderArgsCommon},
    },
};
use clap::Parser;

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
            let translation = TranslationOptions::new(
                transcription.source_language.clone(),
                args.target_language,
            );
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
        soft_subtitles: args.soft_subtitles,
    }
}
