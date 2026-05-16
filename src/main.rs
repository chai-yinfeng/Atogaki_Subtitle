mod cli;
mod config;
mod deepl;
mod job;
mod media;
mod segment;
mod subtitle;
mod whisper;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = config::AppConfig::from_cli(&cli);

    match cli.command {
        Command::Devices => media::list_capture_devices(&config.ffmpeg).await,
        Command::Record(args) => media::record_audio(&config.ffmpeg, &args).await,
        Command::Transcribe(args) => {
            let job = job::Job::create(args.output_dir.as_deref())?;
            let wav = media::extract_wav(&config.ffmpeg, &args.input, &job.audio_wav).await?;
            let raw =
                whisper::transcribe(&config.whisper_cli, &args.model, &wav, &job.prefix).await?;
            let refined = segment::refine(raw);
            job.write_segments(&refined)?;
            subtitle::write_srt(&job.ja_srt, &refined, subtitle::SubtitleTrack::Japanese)?;
            println!("Job written to {}", job.dir.display());
            Ok(())
        }
        Command::Translate(args) => {
            let job = job::Job::open(args.job_dir)?;
            let mut segments = job.read_segments()?;
            let key = args
                .deepl_auth_key
                .or(config.deepl_auth_key)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "DeepL key missing. Set DEEPL_AUTH_KEY or pass --deepl-auth-key"
                    )
                })?;
            deepl::translate_segments(&key, &mut segments).await?;
            job.write_segments(&segments)?;
            subtitle::write_srt(&job.zh_srt, &segments, subtitle::SubtitleTrack::Chinese)?;
            subtitle::write_ass(&job.bilingual_ass, &segments)?;
            println!("Translated subtitles written to {}", job.dir.display());
            Ok(())
        }
        Command::Export(args) => {
            let job = job::Job::open(args.job_dir)?;
            let segments = job.read_segments()?;
            subtitle::write_srt(&job.ja_srt, &segments, subtitle::SubtitleTrack::Japanese)?;
            subtitle::write_srt(&job.zh_srt, &segments, subtitle::SubtitleTrack::Chinese)?;
            subtitle::write_ass(&job.bilingual_ass, &segments)?;
            println!("Subtitles written to {}", job.dir.display());
            Ok(())
        }
        Command::Render(args) => {
            let job = job::Job::open(args.job_dir)?;
            media::render_with_ass(
                &config.ffmpeg,
                &args.input,
                &job.bilingual_ass,
                &args.output,
            )
            .await?;
            println!("Rendered {}", args.output.display());
            Ok(())
        }
        Command::Process(args) => {
            let job = job::Job::create(args.output_dir.as_deref())?;
            let wav = media::extract_wav(&config.ffmpeg, &args.input, &job.audio_wav).await?;
            let raw =
                whisper::transcribe(&config.whisper_cli, &args.model, &wav, &job.prefix).await?;
            let mut segments = segment::refine(raw);
            subtitle::write_srt(&job.ja_srt, &segments, subtitle::SubtitleTrack::Japanese)?;

            if let Some(key) = args.deepl_auth_key.or(config.deepl_auth_key) {
                deepl::translate_segments(&key, &mut segments).await?;
                subtitle::write_srt(&job.zh_srt, &segments, subtitle::SubtitleTrack::Chinese)?;
                subtitle::write_ass(&job.bilingual_ass, &segments)?;
                if let Some(render_output) = args.render_output.as_deref() {
                    media::render_with_ass(
                        &config.ffmpeg,
                        &args.input,
                        &job.bilingual_ass,
                        render_output,
                    )
                    .await?;
                }
            } else {
                eprintln!("DeepL key missing; wrote Japanese transcript only.");
            }

            job.write_segments(&segments)?;
            println!("Job written to {}", job.dir.display());
            Ok(())
        }
    }
}
