use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::ExitStatus,
};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::{
    application::TranscriptionOptions,
    domain::{TranscriptSegment, glossary},
    infrastructure::child_process::sidecar_command,
};

#[derive(Debug, Deserialize)]
struct WhisperJson {
    transcription: Vec<WhisperItem>,
}

#[derive(Debug, Deserialize)]
struct WhisperItem {
    offsets: Offsets,
    text: String,
}

#[derive(Debug, Deserialize)]
struct Offsets {
    from: u64,
    to: u64,
}

pub async fn transcribe(
    whisper_cli: &Path,
    options: &TranscriptionOptions,
    wav: &Path,
    output_prefix: &Path,
) -> Result<Vec<TranscriptSegment>> {
    let model = &options.model;
    if !model.exists() {
        anyhow::bail!("Whisper model does not exist: {}", model.display());
    }
    if let Some(vad_model) = options.vad_model.as_deref()
        && !vad_model.exists()
    {
        anyhow::bail!("VAD model does not exist: {}", vad_model.display());
    }
    let prompt = glossary::build_whisper_prompt(options)?;

    if !options.no_gpu {
        eprintln!("whisper-cli: requesting GPU device 0 (Metal on supported macOS builds)");
    }

    let first = run_whisper(
        whisper_cli,
        options,
        wav,
        output_prefix,
        prompt.as_deref(),
        options.no_gpu,
    )
    .await;

    if let Err(error) = first {
        if options.no_gpu || !error.looks_gpu_related() {
            return Err(error.into());
        }

        eprintln!(
            "whisper-cli failed in GPU/Metal mode; retrying with --no-gpu. Original failure: {}",
            error.summary()
        );
        run_whisper(
            whisper_cli,
            options,
            wav,
            output_prefix,
            prompt.as_deref(),
            true,
        )
        .await
        .map_err(anyhow::Error::from)?;
    }

    let json_path = json_path(output_prefix);
    let data =
        fs::read(&json_path).with_context(|| format!("failed to read {}", json_path.display()))?;
    let decoded: WhisperJson =
        serde_json::from_slice(&data).context("failed to parse whisper json")?;

    Ok(decoded
        .transcription
        .into_iter()
        .filter_map(|item| {
            let text = item.text.trim().to_string();
            if text.is_empty() {
                return None;
            }
            Some(TranscriptSegment::new(
                item.offsets.from,
                item.offsets.to,
                text,
            ))
        })
        .collect())
}

async fn run_whisper(
    whisper_cli: &Path,
    options: &TranscriptionOptions,
    wav: &Path,
    output_prefix: &Path,
    prompt: Option<&str>,
    force_cpu: bool,
) -> std::result::Result<(), WhisperFailure> {
    let args = build_args(options, wav, output_prefix, prompt, force_cpu);
    let mut command = sidecar_command(whisper_cli);
    command.kill_on_drop(true);
    let output = command
        .args(args)
        .output()
        .await
        .map_err(|error| WhisperFailure::start(error.to_string()))?;

    if output.status.success() {
        return Ok(());
    }

    Err(WhisperFailure::process(
        output.status,
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ))
}

fn build_args(
    options: &TranscriptionOptions,
    wav: &Path,
    output_prefix: &Path,
    prompt: Option<&str>,
    force_cpu: bool,
) -> Vec<OsString> {
    let mut args = vec![
        "-m".into(),
        options.model.as_os_str().to_os_string(),
        "-l".into(),
        options.source_language.whisper_code().into(),
        "-sns".into(),
        "-nth".into(),
        format!("{:.2}", options.no_speech_threshold).into(),
    ];

    if options.max_len > 0 {
        args.push("-ml".into());
        args.push(options.max_len.to_string().into());
    }
    if options.split_on_word {
        args.push("-sow".into());
    }
    if options.output_json_full {
        args.push("-ojf".into());
    } else {
        args.push("-oj".into());
    }
    if force_cpu {
        args.push("--no-gpu".into());
    } else {
        args.push("--device".into());
        args.push("0".into());
    }
    if let Some(prompt) = prompt {
        args.push("--prompt".into());
        args.push(prompt.into());
    }
    args.push("-otxt".into());

    if let Some(vad_model) = options.vad_model.as_deref() {
        args.push("--vad".into());
        args.push("--vad-model".into());
        args.push(vad_model.as_os_str().to_os_string());
        args.push("--vad-threshold".into());
        args.push(format!("{:.2}", options.vad_threshold).into());
        args.push("--vad-min-speech-duration-ms".into());
        args.push(options.vad_min_speech_ms.to_string().into());
        args.push("--vad-min-silence-duration-ms".into());
        args.push(options.vad_min_silence_ms.to_string().into());
        args.push("--vad-max-speech-duration-s".into());
        args.push(options.vad_max_speech_s.to_string().into());
        args.push("--vad-speech-pad-ms".into());
        args.push(options.vad_speech_pad_ms.to_string().into());
    }

    args.push("-of".into());
    args.push(output_prefix.as_os_str().to_os_string());
    args.push(wav.as_os_str().to_os_string());
    args
}

#[derive(Debug)]
struct WhisperFailure {
    status: Option<ExitStatus>,
    stdout: String,
    stderr: String,
    start_error: Option<String>,
}

impl WhisperFailure {
    fn start(error: String) -> Self {
        Self {
            status: None,
            stdout: String::new(),
            stderr: String::new(),
            start_error: Some(error),
        }
    }

    fn process(status: ExitStatus, stdout: String, stderr: String) -> Self {
        Self {
            status: Some(status),
            stdout,
            stderr,
            start_error: None,
        }
    }

    fn looks_gpu_related(&self) -> bool {
        if self
            .status
            .is_some_and(|status| status.to_string().contains("signal"))
        {
            return true;
        }

        let combined = format!("{}\n{}", self.stdout, self.stderr).to_lowercase();
        ["metal", "gpu", "ggml_metal"]
            .iter()
            .any(|needle| combined.contains(needle))
    }

    fn summary(&self) -> String {
        if let Some(error) = self.start_error.as_deref() {
            return format!("failed to start whisper-cli: {error}");
        }
        let status = self
            .status
            .map(|status| status.to_string())
            .unwrap_or_else(|| "unknown status".to_string());
        let tail = self
            .stderr
            .lines()
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" ");
        if tail.is_empty() {
            format!("exit {status}")
        } else {
            format!("exit {status}: {tail}")
        }
    }
}

impl From<WhisperFailure> for anyhow::Error {
    fn from(error: WhisperFailure) -> Self {
        if let Some(start_error) = error.start_error {
            return anyhow::anyhow!("failed to start whisper-cli: {start_error}");
        }

        anyhow::anyhow!(
            "whisper-cli failed with {}\nstdout:\n{}\nstderr:\n{}",
            error
                .status
                .map(|status| status.to_string())
                .unwrap_or_else(|| "unknown status".to_string()),
            error.stdout,
            error.stderr
        )
    }
}

fn json_path(prefix: &Path) -> PathBuf {
    let mut path = prefix.to_path_buf();
    path.set_extension("json");
    path
}

#[cfg(test)]
mod tests {
    use super::build_args;
    use crate::application::TranscriptionOptions;
    use crate::domain::LanguageCode;
    use std::path::Path;

    fn string_args(source_language: LanguageCode, force_cpu: bool) -> Vec<String> {
        build_args(
            &TranscriptionOptions::new("model.bin".into(), source_language),
            Path::new("audio.wav"),
            Path::new("transcript"),
            None,
            force_cpu,
        )
        .into_iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect()
    }

    #[test]
    fn requests_the_first_gpu_device_by_default() {
        let args = string_args(LanguageCode::Japanese, false);

        assert!(args.windows(2).any(|pair| pair == ["--device", "0"]));
        assert!(!args.iter().any(|argument| argument == "--no-gpu"));
    }

    #[test]
    fn cpu_retry_disables_gpu_instead_of_selecting_a_device() {
        let args = string_args(LanguageCode::Japanese, true);

        assert!(args.iter().any(|argument| argument == "--no-gpu"));
        assert!(!args.iter().any(|argument| argument == "--device"));
    }

    #[test]
    fn maps_english_to_the_whisper_language_argument() {
        let args = string_args(LanguageCode::English, false);

        assert!(args.windows(2).any(|pair| pair == ["-l", "en"]));
    }
}
