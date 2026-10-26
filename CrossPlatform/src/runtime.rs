use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::audio::{AudioLevels, MicrophoneRecorder};
use crate::model::{self, ModelProgress};
use crate::settings::Settings;
use crate::state::{AppEvent, AppStatus};
use crate::transcription::{
    BackgroundSession, ParakeetTranscriber, Transcript, TranscriptionWorker,
};

#[derive(Clone, Copy)]
pub enum EngineCommand {
    Toggle,
    Start,
    Stop,
    Quit,
}

impl From<crate::state::RecordingIntent> for EngineCommand {
    fn from(intent: crate::state::RecordingIntent) -> Self {
        match intent {
            crate::state::RecordingIntent::Start => Self::Start,
            crate::state::RecordingIntent::Stop => Self::Stop,
            crate::state::RecordingIntent::Toggle => Self::Toggle,
        }
    }
}

pub enum EngineUpdate {
    Status(AppStatus),
    Notice(String),
    RecordingStarted(AudioLevels),
    ModelProgress(ModelProgress),
    TranscriptReady {
        model: String,
        transcript: Transcript,
    },
}

pub struct EngineHandle {
    pub commands: Sender<EngineCommand>,
    pub updates: Receiver<EngineUpdate>,
    cancelled: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl EngineHandle {
    pub fn stop(mut self) {
        self.cancelled.store(true, Ordering::Release);
        let _ = self.commands.send(EngineCommand::Quit);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn start(settings: Settings) -> EngineHandle {
    let (command_sender, command_receiver) = mpsc::channel();
    let (update_sender, update_receiver) = mpsc::channel();
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = cancelled.clone();
    let worker =
        thread::spawn(move || run(settings, command_receiver, update_sender, worker_cancelled));

    EngineHandle {
        commands: command_sender,
        updates: update_receiver,
        cancelled,
        worker: Some(worker),
    }
}

fn run(
    mut settings: Settings,
    commands: Receiver<EngineCommand>,
    updates: Sender<EngineUpdate>,
    cancelled: Arc<AtomicBool>,
) {
    let mut status = AppStatus::Starting;
    send_status(&updates, &status);

    let uses_default_model = settings.model_directory.is_none();
    let model_directory = match settings.model_directory.clone() {
        Some(path) => Ok(path),
        None => model::ensure_default_model(&cancelled, |progress| {
            let _ = updates.send(EngineUpdate::ModelProgress(progress));
        }),
    };
    let model_directory = match model_directory {
        Ok(path) => path,
        Err(error) => {
            send_error(&updates, error);
            return;
        }
    };
    if cancelled.load(Ordering::Acquire) {
        return;
    }
    let model = model_name(&model_directory);
    let transcriber = match ParakeetTranscriber::load(&model_directory, 2) {
        Ok(transcriber) => transcriber,
        Err(error) => {
            let error = if uses_default_model {
                match model::quarantine_default_model_after_load_failure(&model_directory) {
                    Ok(Some(quarantine)) => format!(
                        "the model failed to load and its cache failed verification. Koett moved it to {}. Restart Koett to download a verified copy",
                        quarantine.display()
                    ),
                    Ok(None) => error,
                    Err(verification_error) => {
                        format!(
                            "{error}. Model cache verification also failed: {verification_error}"
                        )
                    }
                }
            } else {
                error
            };
            send_error(&updates, error);
            return;
        }
    };
    if cancelled.load(Ordering::Acquire) {
        return;
    }
    let transcription = match TranscriptionWorker::start(transcriber) {
        Ok(worker) => worker,
        Err(error) => {
            send_error(&updates, error);
            return;
        }
    };

    status = match status.next(AppEvent::Prepared, None) {
        Ok(status) => status,
        Err(error) => {
            send_error(&updates, error);
            return;
        }
    };
    send_status(&updates, &status);

    #[cfg(target_os = "linux")]
    let media = match crate::linux::media::MediaController::new() {
        Ok(media) => Some(media),
        Err(error) => {
            let _ = updates.send(EngineUpdate::Notice(format!(
                "Media control unavailable: {error}"
            )));
            None
        }
    };
    let mut capture_started = None::<std::time::Instant>;
    let mut recorder = None;
    let mut background: Option<BackgroundSession> = None;
    loop {
        let command = if status == AppStatus::Recording {
            match commands.recv_timeout(
                Duration::from_secs(10 * 60).saturating_sub(
                    capture_started
                        .map(|started| started.elapsed())
                        .unwrap_or_default(),
                ),
            ) {
                Ok(command) => command,
                Err(RecvTimeoutError::Timeout) => EngineCommand::Toggle,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match commands.recv() {
                Ok(command) => command,
                Err(_) => break,
            }
        };
        match command {
            EngineCommand::Quit => break,
            EngineCommand::Toggle | EngineCommand::Start if status == AppStatus::Ready => {
                let current = match Settings::load() {
                    Ok(current) => {
                        settings = current.clone();
                        current
                    }
                    Err(error) => {
                        let _ = updates.send(EngineUpdate::Notice(format!(
                            "Settings unavailable; using last valid input settings: {error}"
                        )));
                        settings.clone()
                    }
                };
                #[cfg(target_os = "linux")]
                if current.pause_media
                    && let Some(media) = &media
                {
                    media_notice(&updates, media.pause());
                }
                match MicrophoneRecorder::start_streaming_with_device(current.microphone.as_deref())
                    .and_then(|(started, audio_chunks)| {
                        let session = transcription.record(started.sample_rate(), audio_chunks)?;
                        Ok((started, session))
                    }) {
                    Ok((started, session)) => {
                        capture_started = Some(std::time::Instant::now());
                        let levels = started.levels();
                        recorder = Some(started);
                        background = Some(session);
                        status = status
                            .next(AppEvent::Toggle, None)
                            .expect("ready can always start recording");
                        send_status(&updates, &status);
                        let _ = updates.send(EngineUpdate::RecordingStarted(levels));
                    }
                    Err(error) => {
                        #[cfg(target_os = "linux")]
                        if let Some(media) = &media {
                            media_notice(&updates, media.resume());
                        }
                        recover_from_error(&updates, &mut status, error);
                    }
                }
            }
            EngineCommand::Toggle | EngineCommand::Stop if status == AppStatus::Recording => {
                status = status
                    .next(AppEvent::Toggle, None)
                    .expect("recording can always stop");
                send_status(&updates, &status);

                capture_started = None;
                let audio = recorder
                    .take()
                    .ok_or_else(|| "the microphone recorder is missing".to_string())
                    .and_then(MicrophoneRecorder::finish);
                #[cfg(target_os = "linux")]
                if let Some(media) = &media {
                    media_notice(&updates, media.resume());
                }
                let result = (|| {
                    let session = background
                        .take()
                        .ok_or_else(|| "background transcription is missing".to_string())?;
                    transcription.finish(session, audio?)
                })();
                match result {
                    Ok(transcript) => {
                        let _ = updates.send(EngineUpdate::TranscriptReady {
                            model: model.clone(),
                            transcript,
                        });
                        status = status
                            .next(AppEvent::TranscriptionFinished, None)
                            .expect("transcription can always finish");
                        send_status(&updates, &status);
                    }
                    Err(error) => recover_from_error(&updates, &mut status, error),
                }
            }
            EngineCommand::Toggle | EngineCommand::Start | EngineCommand::Stop => {}
        }
    }
    drop(background.take());
    drop(recorder.take());
    #[cfg(target_os = "linux")]
    if let Some(media) = &media {
        media_notice(&updates, media.resume());
    }
    transcription.stop();
}

#[cfg(target_os = "linux")]
fn media_notice(updates: &Sender<EngineUpdate>, result: Result<(), String>) {
    if let Err(error) = result {
        let _ = updates.send(EngineUpdate::Notice(format!("Media control: {error}")));
    }
}

fn recover_from_error(updates: &Sender<EngineUpdate>, status: &mut AppStatus, error: String) {
    *status = status
        .clone()
        .next(AppEvent::Failed, Some(error))
        .expect("all states can fail");
    send_status(updates, status);
    *status = status
        .clone()
        .next(AppEvent::Recovered, None)
        .expect("an error can recover");
}

fn send_status(updates: &Sender<EngineUpdate>, status: &AppStatus) {
    let _ = updates.send(EngineUpdate::Status(status.clone()));
}

fn send_error(updates: &Sender<EngineUpdate>, error: String) {
    let _ = updates.send(EngineUpdate::Status(AppStatus::Error(error)));
}

fn model_name(directory: &Path) -> String {
    directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("local-model")
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::model_name;

    #[test]
    fn model_name_uses_the_model_folder() {
        assert_eq!(
            model_name(Path::new("models/parakeet-110m")),
            "parakeet-110m"
        );
    }
}
