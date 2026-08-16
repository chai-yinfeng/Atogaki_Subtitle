use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;

const CACHE_MAGIC: &[u8; 8] = b"ATOWAV01";
const CACHE_HEADER_BYTES: u64 = 48;
const TARGET_BUCKETS_PER_SECOND: u32 = 100;
const MIN_POINTS: usize = 64;
const MAX_POINTS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WaveformPeak {
    pub min: f32,
    pub max: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WaveformWindow {
    pub duration_ms: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub point_duration_ms: f64,
    pub peaks: Vec<WaveformPeak>,
}

#[derive(Debug, Clone, Copy)]
struct WavInfo {
    sample_rate: u32,
    data_offset: u64,
    data_bytes: u64,
}

#[derive(Debug, Clone, Copy)]
struct CacheHeader {
    source_bytes: u64,
    source_modified_secs: u64,
    sample_rate: u32,
    samples_per_bucket: u32,
    duration_ms: u64,
    bucket_count: u64,
}

pub fn load_waveform_window(
    wav_path: &Path,
    cache_path: &Path,
    start_ms: i64,
    end_ms: i64,
    point_count: usize,
) -> Result<WaveformWindow> {
    let source_metadata = fs::metadata(wav_path)
        .with_context(|| format!("failed to read waveform source {}", wav_path.display()))?;
    let source_bytes = source_metadata.len();
    let source_modified_secs = source_metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_secs());
    let expected_source = (source_bytes, source_modified_secs);
    let header = match read_cache_header(cache_path).filter(|header| {
        header.source_bytes == expected_source.0
            && header.source_modified_secs == expected_source.1
            && header.sample_rate > 0
            && header.samples_per_bucket > 0
            && header.bucket_count > 0
    }) {
        Some(header) => header,
        None => build_cache(wav_path, cache_path, source_bytes, source_modified_secs)?,
    };
    read_window(cache_path, header, start_ms, end_ms, point_count)
}

fn build_cache(
    wav_path: &Path,
    cache_path: &Path,
    source_bytes: u64,
    source_modified_secs: u64,
) -> Result<CacheHeader> {
    let mut source = BufReader::new(
        File::open(wav_path)
            .with_context(|| format!("failed to open waveform source {}", wav_path.display()))?,
    );
    let info = read_wav_info(&mut source)?;
    let samples_per_bucket = (info.sample_rate / TARGET_BUCKETS_PER_SECOND).max(1);
    let sample_count = info.data_bytes / 2;
    let bucket_count = sample_count.div_ceil(u64::from(samples_per_bucket));
    if bucket_count == 0 {
        bail!("waveform source contains no PCM samples");
    }
    let duration_ms = sample_count
        .saturating_mul(1_000)
        .div_ceil(u64::from(info.sample_rate));
    let header = CacheHeader {
        source_bytes,
        source_modified_secs,
        sample_rate: info.sample_rate,
        samples_per_bucket,
        duration_ms,
        bucket_count,
    };

    let temporary_path = temporary_cache_path(cache_path);
    let result = (|| -> Result<()> {
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create waveform cache directory {}",
                    parent.display()
                )
            })?;
        }
        let mut output = BufWriter::new(File::create(&temporary_path).with_context(|| {
            format!(
                "failed to create waveform cache {}",
                temporary_path.display()
            )
        })?);
        write_cache_header(&mut output, header)?;
        source.seek(SeekFrom::Start(info.data_offset))?;
        let mut remaining_samples = sample_count;
        while remaining_samples > 0 {
            let current_bucket = remaining_samples.min(u64::from(samples_per_bucket));
            let mut minimum = i16::MAX;
            let mut maximum = i16::MIN;
            for _ in 0..current_bucket {
                let mut bytes = [0_u8; 2];
                source.read_exact(&mut bytes)?;
                let sample = i16::from_le_bytes(bytes);
                minimum = minimum.min(sample);
                maximum = maximum.max(sample);
            }
            output.write_all(&minimum.to_le_bytes())?;
            output.write_all(&maximum.to_le_bytes())?;
            remaining_samples -= current_bucket;
        }
        output.flush()?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary_path);
        return Err(error);
    }
    if cache_path.exists() {
        fs::remove_file(cache_path).with_context(|| {
            format!("failed to replace waveform cache {}", cache_path.display())
        })?;
    }
    fs::rename(&temporary_path, cache_path)
        .with_context(|| format!("failed to install waveform cache {}", cache_path.display()))?;
    Ok(header)
}

fn read_window(
    cache_path: &Path,
    header: CacheHeader,
    requested_start_ms: i64,
    requested_end_ms: i64,
    requested_points: usize,
) -> Result<WaveformWindow> {
    let duration_ms = i64::try_from(header.duration_ms).unwrap_or(i64::MAX);
    let start_ms = requested_start_ms.clamp(0, duration_ms.saturating_sub(1));
    let end_ms = requested_end_ms.clamp(start_ms + 1, duration_ms);
    let bucket_numerator = u64::from(header.sample_rate).saturating_mul(1);
    let bucket_denominator = u64::from(header.samples_per_bucket).saturating_mul(1_000);
    let first_bucket = u64::try_from(start_ms)
        .unwrap_or(0)
        .saturating_mul(bucket_numerator)
        / bucket_denominator;
    let last_bucket = u64::try_from(end_ms)
        .unwrap_or(header.duration_ms)
        .saturating_mul(bucket_numerator)
        .div_ceil(bucket_denominator)
        .min(header.bucket_count);
    let visible_bucket_count = last_bucket.saturating_sub(first_bucket).max(1);
    let visible_bucket_count_usize =
        usize::try_from(visible_bucket_count).context("visible waveform window is too large")?;
    let point_count = requested_points
        .clamp(MIN_POINTS, MAX_POINTS)
        .min(visible_bucket_count_usize);

    let mut input = BufReader::new(
        File::open(cache_path)
            .with_context(|| format!("failed to open waveform cache {}", cache_path.display()))?,
    );
    input.seek(SeekFrom::Start(
        CACHE_HEADER_BYTES + first_bucket.saturating_mul(4),
    ))?;
    let mut raw = Vec::with_capacity(visible_bucket_count_usize);
    for _ in 0..visible_bucket_count {
        let mut bytes = [0_u8; 4];
        input.read_exact(&mut bytes)?;
        raw.push((
            i16::from_le_bytes([bytes[0], bytes[1]]),
            i16::from_le_bytes([bytes[2], bytes[3]]),
        ));
    }

    let mut peaks = Vec::with_capacity(point_count);
    for point in 0..point_count {
        let from = point * raw.len() / point_count;
        let to = ((point + 1) * raw.len() / point_count).max(from + 1);
        let (minimum, maximum) = raw[from..to]
            .iter()
            .fold((i16::MAX, i16::MIN), |(minimum, maximum), peak| {
                (minimum.min(peak.0), maximum.max(peak.1))
            });
        peaks.push(WaveformPeak {
            min: f32::from(minimum) / 32_768.0,
            max: f32::from(maximum) / 32_767.0,
        });
    }
    Ok(WaveformWindow {
        duration_ms,
        start_ms,
        end_ms,
        point_duration_ms: (end_ms - start_ms) as f64 / point_count as f64,
        peaks,
    })
}

fn read_wav_info(reader: &mut (impl Read + Seek)) -> Result<WavInfo> {
    let mut riff = [0_u8; 12];
    reader.read_exact(&mut riff)?;
    if &riff[0..4] != b"RIFF" || &riff[8..12] != b"WAVE" {
        bail!("waveform source is not a RIFF/WAVE file");
    }
    let mut format: Option<(u16, u16, u32, u16)> = None;
    let mut data: Option<(u64, u64)> = None;
    loop {
        let mut chunk_header = [0_u8; 8];
        match reader.read_exact(&mut chunk_header) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error.into()),
        }
        let chunk_size = u64::from(u32::from_le_bytes([
            chunk_header[4],
            chunk_header[5],
            chunk_header[6],
            chunk_header[7],
        ]));
        let chunk_data_offset = reader.stream_position()?;
        match &chunk_header[0..4] {
            b"fmt " => {
                if chunk_size < 16 {
                    bail!("WAV fmt chunk is too short");
                }
                let mut fmt = [0_u8; 16];
                reader.read_exact(&mut fmt)?;
                format = Some((
                    u16::from_le_bytes([fmt[0], fmt[1]]),
                    u16::from_le_bytes([fmt[2], fmt[3]]),
                    u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]),
                    u16::from_le_bytes([fmt[14], fmt[15]]),
                ));
            }
            b"data" => data = Some((chunk_data_offset, chunk_size)),
            _ => {}
        }
        reader.seek(SeekFrom::Start(
            chunk_data_offset + chunk_size + (chunk_size % 2),
        ))?;
        if format.is_some() && data.is_some() {
            break;
        }
    }
    let (audio_format, channels, sample_rate, bits_per_sample) =
        format.ok_or_else(|| anyhow!("WAV fmt chunk is missing"))?;
    if audio_format != 1 || channels != 1 || bits_per_sample != 16 || sample_rate == 0 {
        bail!(
            "waveform source must be mono 16-bit PCM; found format {audio_format}, channels {channels}, sample rate {sample_rate}, bits {bits_per_sample}"
        );
    }
    let (data_offset, data_bytes) = data.ok_or_else(|| anyhow!("WAV data chunk is missing"))?;
    if data_bytes < 2 || data_bytes % 2 != 0 {
        bail!("WAV PCM data has an invalid length");
    }
    Ok(WavInfo {
        sample_rate,
        data_offset,
        data_bytes,
    })
}

fn read_cache_header(path: &Path) -> Option<CacheHeader> {
    let mut input = File::open(path).ok()?;
    let mut magic = [0_u8; 8];
    input.read_exact(&mut magic).ok()?;
    if &magic != CACHE_MAGIC {
        return None;
    }
    Some(CacheHeader {
        source_bytes: read_u64(&mut input).ok()?,
        source_modified_secs: read_u64(&mut input).ok()?,
        sample_rate: read_u32(&mut input).ok()?,
        samples_per_bucket: read_u32(&mut input).ok()?,
        duration_ms: read_u64(&mut input).ok()?,
        bucket_count: read_u64(&mut input).ok()?,
    })
}

fn write_cache_header(output: &mut impl Write, header: CacheHeader) -> Result<()> {
    output.write_all(CACHE_MAGIC)?;
    output.write_all(&header.source_bytes.to_le_bytes())?;
    output.write_all(&header.source_modified_secs.to_le_bytes())?;
    output.write_all(&header.sample_rate.to_le_bytes())?;
    output.write_all(&header.samples_per_bucket.to_le_bytes())?;
    output.write_all(&header.duration_ms.to_le_bytes())?;
    output.write_all(&header.bucket_count.to_le_bytes())?;
    Ok(())
}

fn read_u32(input: &mut impl Read) -> Result<u32> {
    let mut bytes = [0_u8; 4];
    input.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(input: &mut impl Read) -> Result<u64> {
    let mut bytes = [0_u8; 8];
    input.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn temporary_cache_path(cache_path: &Path) -> PathBuf {
    let file_name = cache_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("waveform-v1.bin");
    cache_path.with_file_name(format!("{file_name}.part-{}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write, time::SystemTime};

    use super::load_waveform_window;

    #[test]
    fn builds_and_reuses_a_windowed_waveform_cache() {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("atogaki-waveform-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        let wav = root.join("audio.wav");
        let cache = root.join("waveform-v1.bin");
        write_pcm_wav(
            &wav,
            1_000,
            &[0, 8_000, -16_000, 24_000, -32_000, 16_000, 0, 4_000],
        );

        let first = load_waveform_window(&wav, &cache, 0, 8, 64).unwrap();
        assert_eq!(first.duration_ms, 8);
        assert_eq!(first.start_ms, 0);
        assert_eq!(first.end_ms, 8);
        assert_eq!(first.peaks.len(), 1);
        assert!(first.peaks[0].min < -0.9);
        assert!(first.peaks[0].max > 0.7);
        assert!(cache.is_file());

        let second = load_waveform_window(&wav, &cache, 2, 7, 64).unwrap();
        assert_eq!(second.start_ms, 2);
        assert_eq!(second.end_ms, 7);

        fs::remove_dir_all(root).unwrap();
    }

    fn write_pcm_wav(path: &std::path::Path, sample_rate: u32, samples: &[i16]) {
        let data_bytes = u32::try_from(samples.len() * 2).unwrap();
        let mut output = fs::File::create(path).unwrap();
        output.write_all(b"RIFF").unwrap();
        output.write_all(&(36 + data_bytes).to_le_bytes()).unwrap();
        output.write_all(b"WAVEfmt ").unwrap();
        output.write_all(&16_u32.to_le_bytes()).unwrap();
        output.write_all(&1_u16.to_le_bytes()).unwrap();
        output.write_all(&1_u16.to_le_bytes()).unwrap();
        output.write_all(&sample_rate.to_le_bytes()).unwrap();
        output.write_all(&(sample_rate * 2).to_le_bytes()).unwrap();
        output.write_all(&2_u16.to_le_bytes()).unwrap();
        output.write_all(&16_u16.to_le_bytes()).unwrap();
        output.write_all(b"data").unwrap();
        output.write_all(&data_bytes.to_le_bytes()).unwrap();
        for sample in samples {
            output.write_all(&sample.to_le_bytes()).unwrap();
        }
    }
}
