mod application;
mod domain;
mod infrastructure;
mod interface;

use anyhow::Result;
use application::{
    JobRunner,
    job_spec::{ExportSpec, ProcessSpec, RenderSpec, TranscribeSpec, TranslateSpec},
};
use clap::Parser;
use infrastructure::{config::AppConfig, media};
use interface::cli::{Cli, Command};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = AppConfig::from_cli(&cli);
    let runner = JobRunner::new(config.clone());

    match cli.command {
        Command::Devices => media::list_capture_devices(&config.ffmpeg).await,
        Command::Record(args) => media::record_audio(&config.ffmpeg, &args).await,
        Command::Transcribe(args) => {
            let job = runner
                .transcribe(TranscribeSpec {
                    input: args.input,
                    output_dir: args.output_dir,
                    whisper: args.whisper,
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
                })
                .await?;
            println!("Rendered {}", args.output.display());
            Ok(())
        }
        Command::Process(args) => {
            let job = runner
                .process(ProcessSpec {
                    input: args.input,
                    output_dir: args.output_dir,
                    render_output: args.render_output,
                    deepl_auth_key: args.deepl_auth_key,
                    whisper: args.whisper,
                })
                .await?;
            println!("Job written to {}", job.dir.display());
            Ok(())
        }
    }
}
