use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::audio::AudioRecording;

static NEXT: AtomicU64 = AtomicU64::new(0);

pub(super) fn retain(audio: &AudioRecording) -> Result<std::path::PathBuf, String> {
    let directory = crate::paths::failed_transcripts_directory()?
        .parent()
        .ok_or("Missing data directory")?
        .join("Failed Recordings");
    retain_in(audio, &directory)
}

fn retain_in(
    audio: &AudioRecording,
    directory: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    for _ in 0..1000 {
        let path = directory.join(format!(
            "recording-{}-{}.wav",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = match options.open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        };
        let mut file = std::io::BufWriter::with_capacity(64 * 1024, file);
        let size = u32::try_from(audio.samples.len() * 2).map_err(|e| e.to_string())?;
        let rate = u32::try_from(audio.sample_rate).map_err(|e| e.to_string())?;
        let result = (|| -> std::io::Result<()> {
            file.write_all(b"RIFF")?;
            file.write_all(&(size + 36).to_le_bytes())?;
            file.write_all(b"WAVEfmt \x10\0\0\0\x01\0\x01\0")?;
            file.write_all(&rate.to_le_bytes())?;
            file.write_all(&(rate * 2).to_le_bytes())?;
            file.write_all(b"\x02\0\x10\0data")?;
            file.write_all(&size.to_le_bytes())?;
            for sample in &audio.samples {
                file.write_all(&((sample.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
            }
            file.flush()?;
            file.get_ref().sync_all()
        })();
        result.map_err(|e| format!("Could not preserve audio at {}: {e}", path.display()))?;
        return Ok(path);
    }
    Err("Could not allocate recovery recording".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_audio_is_readable_private_and_collision_safe() {
        let directory =
            std::env::temp_dir().join(format!("koett-nemotron-recovery-{}", std::process::id()));
        let audio = AudioRecording::new(48000, vec![0.25; 4800]).unwrap();
        let first = retain_in(&audio, &directory).unwrap();
        let second = retain_in(&audio, &directory).unwrap();
        assert_ne!(first, second);
        let restored = AudioRecording::read_wav(&first).unwrap();
        assert_eq!(restored.sample_rate, 48000);
        assert_eq!(restored.samples.len(), audio.samples.len());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&first).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(directory).unwrap();
    }
}
