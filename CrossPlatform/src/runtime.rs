use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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

#[derive(Clone)]
pub enum EngineCommand {
    #[cfg(target_os = "linux")]
    Media(String),
    #[cfg(target_os = "linux")]
    CancelMedia,
    #[cfg(target_os = "linux")]
    Ask,
    #[cfg(target_os = "linux")]
    CancelAsk,
    #[cfg(target_os = "linux")]
    AskStart,
    #[cfg(target_os = "linux")]
    AskStop,
    Toggle,
    Start,
    Stop,
    /// Discards the active dictation: no transcription, paste or history.
    Cancel,
    SwitchModel(model::LocalModel),
    Formatting(crate::formatting::Formatting),
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
    #[cfg(target_os = "linux")]
    Ask(serde_json::Value),
    #[cfg(target_os = "linux")]
    PrepareAsk(Sender<()>),
    ActiveModel(String),
    Status(AppStatus),
    Notice(String),
    FormattingBusy(bool),
    RecordingStarted(AudioLevels),
    ModelProgress(ModelProgress),
    TranscriptReady {
        model: String,
        transcript: Transcript,
        /// The dictation this belongs to; see [`DictationSessions`].
        session: u64,
    },
}

/// Numbers each dictation so a Cancel can invalidate it immediately, from any
/// thread, even while the engine is still blocked decoding it after Stop.
#[derive(Default)]
pub struct DictationSessions {
    current: AtomicU64,
    cancelled: AtomicU64,
}

impl DictationSessions {
    fn begin(&self) -> u64 {
        self.current.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub fn current(&self) -> u64 {
        self.current.load(Ordering::Acquire)
    }

    /// Marks the latest dictation cancelled; its transcript must not be
    /// pasted or saved, wherever it is in the pipeline.
    pub fn cancel_current(&self) {
        self.cancelled.store(self.current(), Ordering::Release);
    }

    pub fn is_cancelled(&self, session: u64) -> bool {
        session != 0 && self.cancelled.load(Ordering::Acquire) == session
    }
}

pub struct EngineHandle {
    pub commands: Sender<EngineCommand>,
    pub updates: Receiver<EngineUpdate>,
    cancelled: Arc<AtomicBool>,
    ask_cancelled: Arc<AtomicBool>,
    sessions: Arc<DictationSessions>,
    worker: Option<JoinHandle<()>>,
}

impl EngineHandle {
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn controlled() -> (Self, Receiver<EngineCommand>, Sender<EngineUpdate>) {
        let (commands, received) = mpsc::channel();
        let (updates, incoming) = mpsc::channel();
        (
            Self {
                commands,
                updates: incoming,
                cancelled: Arc::new(AtomicBool::new(false)),
                ask_cancelled: Arc::new(AtomicBool::new(false)),
                sessions: Arc::new(DictationSessions::default()),
                worker: None,
            },
            received,
            updates,
        )
    }

    /// Invalidates the active dictation at once, before the queued Cancel
    /// command reaches a worker that may be busy finishing a Stop.
    pub fn cancel_dictation(&self) {
        self.sessions.cancel_current();
    }

    pub fn dictation_cancelled(&self, session: u64) -> bool {
        self.sessions.is_cancelled(session)
    }

    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn begin_test_session(&self) -> u64 {
        self.sessions.begin()
    }

    pub fn cancel_ask(&self) {
        self.ask_cancelled.store(true, Ordering::Release);
    }
    pub fn stop(mut self) {
        self.cancel_ask();
        self.cancelled.store(true, Ordering::Release);
        crate::formatting::CANCELLED.store(true, Ordering::Release);
        model::SWITCH_CANCELLED.store(true, Ordering::Release);
        let _ = self.commands.send(EngineCommand::Quit);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

enum CaptureSource {
    Microphone,
    #[cfg(target_os = "windows")]
    WavFixture {
        path: std::path::PathBuf,
        fail_asr: bool,
    },
}

#[cfg(target_os = "windows")]
pub(crate) fn start_wav_lab(
    settings: Settings,
    path: std::path::PathBuf,
    fail_asr: bool,
) -> EngineHandle {
    start_source(settings, CaptureSource::WavFixture { path, fail_asr })
}

pub fn start(settings: Settings) -> EngineHandle {
    start_source(settings, CaptureSource::Microphone)
}

fn start_source(settings: Settings, source: CaptureSource) -> EngineHandle {
    let (command_sender, command_receiver) = mpsc::channel();
    let (update_sender, update_receiver) = mpsc::channel();
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = cancelled.clone();
    let ask_cancelled = Arc::new(AtomicBool::new(false));
    let worker_ask_cancelled = ask_cancelled.clone();
    let sessions = Arc::new(DictationSessions::default());
    let worker_sessions = sessions.clone();
    model::SWITCH_CANCELLED.store(false, Ordering::Release);
    let worker = thread::spawn(move || {
        run(
            settings,
            command_receiver,
            update_sender,
            worker_cancelled,
            worker_ask_cancelled,
            worker_sessions,
            source,
        )
    });

    EngineHandle {
        commands: command_sender,
        updates: update_receiver,
        cancelled,
        ask_cancelled,
        sessions,
        worker: Some(worker),
    }
}

fn run(
    mut settings: Settings,
    commands: Receiver<EngineCommand>,
    updates: Sender<EngineUpdate>,
    cancelled: Arc<AtomicBool>,
    ask_cancelled: Arc<AtomicBool>,
    sessions: Arc<DictationSessions>,
    source: CaptureSource,
) {
    #[cfg(not(all(target_os = "linux", feature = "ask")))]
    let _ = ask_cancelled;
    let mut status = AppStatus::Starting;
    send_status(&updates, &status);

    if cancelled.load(Ordering::Acquire) {
        return;
    }
    let prepared = prepare_engine(&settings, &model::SWITCH_CANCELLED, &updates);
    let (model_directory, worker) = match prepared {
        Ok(prepared) => prepared,
        Err(error)
            if settings.model_directory.is_none()
                && settings.selected_model != model::LocalModel::Compact =>
        {
            let _ = updates.send(EngineUpdate::Notice(format!(
                "Selected model failed: {error}. Falling back to Parakeet 110M."
            )));
            settings.selected_model = model::LocalModel::Compact;
            match prepare_engine(&settings, &model::SWITCH_CANCELLED, &updates) {
                Ok(prepared) => prepared,
                Err(error) => {
                    send_error(&updates, error);
                    return;
                }
            }
        }
        Err(error) => {
            send_error(&updates, error);
            return;
        }
    };
    let mut model = model_name(&model_directory);
    if cancelled.load(Ordering::Acquire) {
        return;
    }
    let mut transcription = worker;
    let mut dictionary = DictionarySync::new(settings.dictionary.clone());

    let _ = updates.send(EngineUpdate::ActiveModel(model.clone()));
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
    let mut cleaner = None;
    let mut preparation = None;
    if settings.formatting.enabled {
        preparation = Some(start_formatting(settings.formatting, updates.clone()));
    }
    let mut capture_started = None::<std::time::Instant>;
    let mut recording_limit = Duration::from_secs(60 * 60);
    let mut limit_warned = false;
    let mut capture_levels = None::<AudioLevels>;
    let mut recorder = None;
    let mut background: Option<BackgroundSession> = None;
    #[cfg(target_os = "linux")]
    let mut media_operation: Option<MediaOperation> = None;
    #[cfg(all(target_os = "linux", feature = "ask"))]
    let mut ask_image: Option<Vec<u8>> = None;
    #[cfg(all(target_os = "linux", feature = "ask"))]
    let mut answer: Option<crate::assistant::Answer> = None;
    #[cfg(all(target_os = "linux", feature = "ask"))]
    let mut screen_capture: Option<crate::assistant::screen::Capture> = None;
    #[cfg(all(target_os = "linux", feature = "ask"))]
    let mut speech: Option<crate::assistant::speech::Speech> = None;
    loop {
        #[cfg(all(target_os = "linux", feature = "ask"))]
        if (ask_image.is_some() || answer.is_some())
            && let Some(active) = &speech
        {
            for notice in active.notices.try_iter() {
                let _ = updates.send(EngineUpdate::Ask(
                    serde_json::json!({"speech_notice": notice}),
                ));
            }
        }
        #[cfg(all(target_os = "linux", feature = "ask"))]
        if ask_cancelled.load(Ordering::Acquire) {
            speech = None;
        }
        #[cfg(all(target_os = "linux", feature = "ask"))]
        if let Some(active) = &answer {
            for event in active.updates.try_iter() {
                let value = match event {
                    crate::assistant::Update::Delta(text) => {
                        let value = serde_json::json!({"delta": text});
                        let _ = updates.send(EngineUpdate::Ask(value.clone()));
                        if let Some(active) = &mut speech {
                            active.append(&text);
                        }
                        continue;
                    }
                    crate::assistant::Update::Finished(result) => match result {
                        Ok(()) => {
                            if let Some(active) = &mut speech {
                                active.finish();
                            }
                            serde_json::json!({"phase": "Done"})
                        }
                        Err(error) => {
                            speech = None;
                            serde_json::json!({"error": error})
                        }
                    },
                };
                let _ = updates.send(EngineUpdate::Ask(value));
            }
        }
        #[cfg(target_os = "linux")]
        if let Some((fetch, download, reply)) = &mut media_operation {
            if reply.is_none() {
                match fetch.reply.try_recv() {
                    Ok(Ok(audio)) => {
                        let _ = updates.send(EngineUpdate::Notice("Transcribing locally…".into()));
                        match transcription
                            .media_request(audio.pcm.clone(), fetch.cancelled.clone())
                        {
                            Ok(result) => {
                                *download = Some(audio);
                                *reply = Some(result);
                            }
                            Err(error) => {
                                media_failed(&updates, error, &fetch.cancelled);
                                media_operation = None;
                                status = AppStatus::Ready;
                                send_status(&updates, &status);
                            }
                        }
                    }
                    Ok(Err(error)) => {
                        media_failed(&updates, error, &fetch.cancelled);
                        media_operation = None;
                        status = AppStatus::Ready;
                        send_status(&updates, &status);
                    }
                    Err(_) => {}
                }
            } else if let Some(Ok(result)) = reply.as_ref().map(|r| r.try_recv()) {
                match result {
                    Ok(text) => {
                        if let Some(audio) = download {
                            match crate::media_transcription::save(
                                &audio.title,
                                &audio.source,
                                &model,
                                &text,
                            ) {
                                Ok(path)
                                    if path.parent().and_then(Path::file_name)
                                        == Some(std::ffi::OsStr::new("Failed Transcripts")) =>
                                {
                                    let _ = updates.send(EngineUpdate::Notice(
                                        "Media saved in transcript recovery".into(),
                                    ));
                                }
                                Ok(_) => {}
                                Err(error) => {
                                    let _ = updates.send(EngineUpdate::Notice(format!("Media storage failed: {error}. Copy the result before quitting")));
                                }
                            }
                        }
                        let _ = updates.send(EngineUpdate::Notice(format!("@media-result:{text}")));
                    }
                    Err(error) => {
                        media_failed(&updates, error, &fetch.cancelled);
                    }
                }
                media_operation = None;
                status = AppStatus::Ready;
                send_status(&updates, &status);
            }
        }
        if status == AppStatus::Ready
            && let Some((receiver, _)) = &preparation
            && let Ok(result) = receiver.try_recv()
        {
            let (_, worker) = preparation.take().expect("pending preparation");
            match result {
                Ok((mut candidate, formatting)) => {
                    candidate.supervisor = Some(worker);
                    let saved = if crate::formatting::CANCELLED.load(Ordering::Acquire) {
                        Err("S1-mini setup cancelled".into())
                    } else {
                        Settings::update(|current| {
                            current.formatting = formatting;
                        })
                    };
                    match saved {
                        Ok(()) => {
                            cleaner = Some(candidate);
                            settings.formatting = formatting;
                            let _ = updates.send(EngineUpdate::Notice("S1-mini ready".into()));
                        }
                        Err(error) => {
                            let _ = updates.send(EngineUpdate::Notice(error));
                        }
                    }
                }
                Err(error) => {
                    let _ = worker.join();
                    let _ = updates.send(EngineUpdate::Notice(format!("Raw active: {error}")));
                }
            }
            let _ = updates.send(EngineUpdate::FormattingBusy(false));
            send_status(&updates, &status);
        }
        #[cfg(all(target_os = "linux", feature = "ask"))]
        let screen_result = screen_capture
            .as_ref()
            .and_then(|capture| capture.result.try_recv().ok());
        #[cfg(all(target_os = "linux", feature = "ask"))]
        let capture_start = if let Some(result) = screen_result {
            screen_capture = None;
            match result {
                Ok(image) => {
                    ask_image = Some(image);
                    let _ = updates.send(EngineUpdate::Ask(
                        serde_json::json!({"reset": true, "phase": "Listening…", "screen": true}),
                    ));
                    true
                }
                Err(error) => {
                    speech = None;
                    let _ = updates.send(EngineUpdate::Ask(
                        serde_json::json!({"error": error, "screen": false}),
                    ));
                    false
                }
            }
        } else {
            false
        };
        #[cfg(not(all(target_os = "linux", feature = "ask")))]
        let capture_start = false;
        let command = if capture_start {
            EngineCommand::Start
        } else if status == AppStatus::Recording {
            let wait = limit_wait(
                capture_started,
                capture_levels.as_ref(),
                recording_limit,
                limit_warned,
            );
            match commands.recv_timeout(wait) {
                Ok(command) => command,
                // Pausing doesn't wake this wait, so look again before acting: paused
                // time doesn't count toward the limit.
                Err(RecvTimeoutError::Timeout)
                    if !limit_wait(
                        capture_started,
                        capture_levels.as_ref(),
                        recording_limit,
                        limit_warned,
                    )
                    .is_zero() =>
                {
                    continue;
                }
                Err(RecvTimeoutError::Timeout) if !limit_warned => {
                    limit_warned = true;
                    let _ = updates.send(EngineUpdate::Notice(LIMIT_WARNING.to_string()));
                    continue;
                }
                Err(RecvTimeoutError::Timeout) => {
                    recording_limit_reached(&updates, recording_limit)
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match commands.recv_timeout(Duration::from_millis(50)) {
                Ok(command) => command,
                Err(RecvTimeoutError::Timeout) => {
                    dictionary.sync(&transcription);
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        };
        #[cfg(all(target_os = "linux", feature = "ask"))]
        let stopping_question = matches!(command, EngineCommand::AskStop | EngineCommand::Stop)
            || matches!(command, EngineCommand::Ask | EngineCommand::Toggle)
                && ask_image.is_some()
                && status == AppStatus::Recording;
        #[cfg(all(target_os = "linux", feature = "ask"))]
        if !(capture_start || stopping_question) {
            speech = None;
        }
        #[cfg(all(target_os = "linux", feature = "ask"))]
        let command = match command {
            EngineCommand::AskStart if status == AppStatus::Ready => EngineCommand::Ask,
            EngineCommand::AskStop if status == AppStatus::Recording && ask_image.is_some() => {
                EngineCommand::Ask
            }
            EngineCommand::AskStart | EngineCommand::AskStop => continue,
            other => other,
        };
        #[cfg(all(target_os = "linux", feature = "ask"))]
        let command = if ask_image.is_some()
            && status == AppStatus::Recording
            && matches!(
                command,
                EngineCommand::Start | EngineCommand::Toggle | EngineCommand::Stop
            ) {
            ask_image = None;
            drop(recorder.take());
            drop(background.take());
            capture_started = None;
            if let Some(media) = &media {
                media_notice(&updates, media.resume());
            }
            status = AppStatus::Ready;
            send_status(&updates, &status);
            let _ = updates.send(EngineUpdate::Ask(
                serde_json::json!({"phase": "Cancelled", "speech_notice": ""}),
            ));
            if matches!(command, EngineCommand::Stop) {
                continue;
            }
            EngineCommand::Start
        } else {
            command
        };
        #[cfg(all(target_os = "linux", feature = "ask"))]
        let command = match command {
            EngineCommand::Ask if status == AppStatus::Recording && ask_image.is_some() => {
                EngineCommand::Stop
            }
            EngineCommand::Ask if status == AppStatus::Ready => {
                answer = None;
                ask_cancelled.store(false, Ordering::Release);
                if screen_capture.is_some() {
                    continue;
                }
                match crate::assistant::Configuration::load().and_then(|config| {
                    config.validate()?;
                    if config.enabled {
                        Ok(())
                    } else {
                        Err("Enable Ask in provider configuration first".into())
                    }
                }) {
                    Ok(()) => {
                        let (hidden, ready) = std::sync::mpsc::channel();
                        let _ = updates.send(EngineUpdate::PrepareAsk(hidden));
                        if ready.recv_timeout(Duration::from_secs(2)).is_ok()
                            && !ask_cancelled.load(Ordering::Acquire)
                        {
                            screen_capture = Some(crate::assistant::screen::Capture::default());
                            match crate::assistant::speech::Configuration::load() {
                                Ok(config) => {
                                    speech = crate::assistant::speech::Speech::prepare(config)
                                }
                                Err(error) => {
                                    let _ = updates.send(EngineUpdate::Ask(
                                        serde_json::json!({"speech_notice": error}),
                                    ));
                                }
                            }
                        } else {
                            let _ = updates.send(EngineUpdate::Ask(serde_json::json!({"error": "Could not prepare screen capture; try Ask again"})));
                        }
                    }
                    Err(error) => {
                        let _ =
                            updates.send(EngineUpdate::Ask(serde_json::json!({"error": error})));
                    }
                }
                continue;
            }
            EngineCommand::Ask => continue,
            EngineCommand::CancelAsk => {
                ask_cancelled.store(true, Ordering::Release);
                screen_capture = None;
                speech = None;
                answer = None;
                if ask_image.take().is_some() && status == AppStatus::Recording {
                    if let Some(media) = &media {
                        media_notice(&updates, media.resume());
                    }
                    capture_started = None;
                    drop(recorder.take());
                    drop(background.take());
                    status = AppStatus::Ready;
                    send_status(&updates, &status);
                }
                let _ = updates.send(EngineUpdate::Ask(
                    serde_json::json!({"phase": "Cancelled", "speech_notice": ""}),
                ));
                continue;
            }
            other => other,
        };
        #[cfg(all(target_os = "linux", feature = "ask"))]
        if !matches!(
            command,
            EngineCommand::Start
                | EngineCommand::Stop
                | EngineCommand::Toggle
                | EngineCommand::Ask
                | EngineCommand::CancelAsk
        ) {
            screen_capture = None;
        }
        #[cfg(all(target_os = "linux", feature = "ask"))]
        if matches!(command, EngineCommand::Start | EngineCommand::Toggle)
            && !capture_start
            && ask_image.is_none()
        {
            screen_capture = None;
            if answer.take().is_some() {
                let _ = updates.send(EngineUpdate::Ask(
                    serde_json::json!({"phase": "Cancelled", "speech_notice": ""}),
                ));
            }
        }
        match command {
            #[cfg(target_os = "linux")]
            EngineCommand::Ask
            | EngineCommand::CancelAsk
            | EngineCommand::AskStart
            | EngineCommand::AskStop => continue,
            #[cfg(target_os = "linux")]
            EngineCommand::Media(source) => {
                #[cfg(feature = "ask")]
                {
                    answer = None;
                }
                if status != AppStatus::Ready || preparation.is_some() {
                    continue;
                }
                let progress_updates = updates.clone();
                match crate::media_transcription::Fetch::start(source, move |phase| {
                    let _ = progress_updates.send(EngineUpdate::Notice(phase.into()));
                }) {
                    Ok(fetch) => {
                        media_operation = Some((fetch, None, None));
                        status = AppStatus::Transcribing;
                        send_status(&updates, &status);
                    }
                    Err(error) => {
                        let _ = updates.send(EngineUpdate::Notice(error));
                    }
                }
            }
            #[cfg(target_os = "linux")]
            EngineCommand::CancelMedia => {
                if let Some((fetch, _, _)) = &media_operation {
                    fetch.cancelled.store(true, Ordering::Release);
                }
            }
            EngineCommand::Quit => break,
            EngineCommand::Formatting(formatting) if status == AppStatus::Ready => {
                if preparation.is_some() {
                    continue;
                }
                if formatting.enabled && cleaner.is_none() {
                    preparation = Some(start_formatting(formatting, updates.clone()));
                } else {
                    let saved = Settings::update(|current| {
                        current.formatting = formatting;
                    });
                    match saved {
                        Ok(()) => {
                            if !formatting.enabled {
                                cleaner = None;
                            }
                            settings.formatting = formatting;
                        }
                        Err(error) => {
                            let _ = updates.send(EngineUpdate::Notice(format!(
                                "S1-mini settings were not saved: {error}"
                            )));
                        }
                    }
                    let _ = updates.send(EngineUpdate::FormattingBusy(false));
                    send_status(&updates, &status);
                }
            }
            EngineCommand::Formatting(_) => {
                let _ = updates.send(EngineUpdate::Notice(
                    "Stop recording before changing formatting".into(),
                ));
            }

            EngineCommand::SwitchModel(selection) if status == AppStatus::Ready => {
                if !begin_model_switch(&cancelled, &model::SWITCH_CANCELLED) {
                    break;
                }
                status = AppStatus::Starting;
                send_status(&updates, &status);
                let candidate =
                    model::ensure_model(selection, &model::SWITCH_CANCELLED, |progress| {
                        let _ = updates.send(EngineUpdate::ModelProgress(progress));
                    })
                    .and_then(|directory| {
                        let worker = load_worker(
                            &directory,
                            selection,
                            dictionary.current(),
                            &model::SWITCH_CANCELLED,
                            &updates,
                        )?;
                        Ok((directory, worker))
                    });
                match candidate {
                    Ok((directory, worker)) if !model::SWITCH_CANCELLED.load(Ordering::Acquire) => {
                        let saved = Settings::update(|current| {
                            current.selected_model = selection;
                            current.model_directory = None;
                        });
                        match saved {
                            Ok(()) => {
                                transcription.stop();
                                transcription = worker;
                                model = model_name(&directory);
                                let _ = updates.send(EngineUpdate::ActiveModel(model.clone()));
                            }
                            Err(error) => {
                                worker.stop();
                                let _ = updates
                                    .send(EngineUpdate::Notice(format!("@model-notice:{error}")));
                            }
                        }
                    }
                    Ok((_, worker)) => worker.stop(),
                    Err(error) => {
                        let _ =
                            updates.send(EngineUpdate::Notice(format!("@model-notice:{error}")));
                    }
                }
                status = AppStatus::Ready;
                send_status(&updates, &status);
            }
            EngineCommand::SwitchModel(_) => {
                let _ = updates.send(EngineUpdate::Notice(
                    "@model-notice:Stop recording before changing models".into(),
                ));
            }
            EngineCommand::Toggle | EngineCommand::Start if status == AppStatus::Ready => {
                #[cfg(target_os = "windows")]
                send_status(&updates, &AppStatus::Starting);
                let mut current = match Settings::load() {
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
                // A mic saved as a raw ALSA id moves to the same mic's sound server source.
                #[cfg(target_os = "linux")]
                if let Some(migrated) =
                    crate::audio::inputs::migrated_input(current.microphone.as_deref())
                {
                    let previous = current.microphone.clone();
                    current.microphone = migrated;
                    let saved = Settings::update(|latest| {
                        if latest.microphone == previous {
                            latest.microphone = current.microphone.clone();
                        }
                    });
                    if let Err(error) = saved {
                        eprintln!("Could not save the migrated microphone: {error}");
                    }
                    settings.microphone = current.microphone.clone();
                }
                let input_resolution = match &source {
                    // Linux picks a working mic while starting and never changes the choice.
                    #[cfg(target_os = "linux")]
                    CaptureSource::Microphone => Ok(current.microphone.clone()),
                    #[cfg(not(target_os = "linux"))]
                    CaptureSource::Microphone => {
                        crate::audio::resolve_recording_input(current.microphone.as_deref())
                    }
                    #[cfg(target_os = "windows")]
                    CaptureSource::WavFixture { .. } => Ok(current.microphone.clone()),
                };
                let mut input_notice = None;
                match input_resolution.as_ref() {
                    Ok(replacement) if replacement != &current.microphone => {
                        let previous = current.microphone.clone();
                        current.microphone = replacement.clone();
                        let saved = Settings::update(|latest| {
                            if latest.microphone == previous {
                                latest.microphone = current.microphone.clone();
                            }
                        });
                        settings.microphone = current.microphone.clone();
                        let notice = match saved {
                            Ok(()) => {
                                "Selected microphone unavailable; replacement input saved".into()
                            }
                            Err(error) => format!(
                                "Selected microphone unavailable; using replacement input for this recording. Could not save: {error}"
                            ),
                        };
                        input_notice = Some(notice);
                    }
                    Ok(_) => {}
                    Err(_) => {}
                }
                #[cfg(target_os = "linux")]
                if input_resolution.is_ok()
                    && current.pause_media
                    && let Some(media) = &media
                {
                    media_notice(&updates, media.pause());
                }
                match input_resolution
                    .and_then(|_| match &source {
                        #[cfg(target_os = "linux")]
                        CaptureSource::Microphone => {
                            let switches = updates.clone();
                            MicrophoneRecorder::start_resilient(
                                current.microphone.as_deref(),
                                move |notice| {
                                    let _ = switches.send(EngineUpdate::Notice(notice));
                                },
                            )
                            .map(|(started, audio_chunks, notice)| {
                                if notice.is_some() {
                                    input_notice = notice;
                                }
                                (started, audio_chunks)
                            })
                        }
                        #[cfg(not(target_os = "linux"))]
                        CaptureSource::Microphone => {
                            MicrophoneRecorder::start_streaming_with_device(
                                current.microphone.as_deref(),
                            )
                        }
                        #[cfg(target_os = "windows")]
                        CaptureSource::WavFixture { path, .. } => {
                            crate::audio::windows_capture::start_wav_proxy(path)
                        }
                    })
                    .and_then(|(started, audio_chunks)| {
                        #[cfg(all(target_os = "linux", feature = "ask"))]
                        let offline = ask_image.is_some();
                        #[cfg(not(all(target_os = "linux", feature = "ask")))]
                        let offline = false;
                        let session = if offline {
                            transcription.record_offline(started.sample_rate(), audio_chunks)?
                        } else {
                            transcription.record(started.sample_rate(), audio_chunks)?
                        };
                        Ok((started, session))
                    }) {
                    Ok((started, session)) => {
                        sessions.begin();
                        capture_started = Some(std::time::Instant::now());
                        recording_limit =
                            Duration::from_secs(u64::from(current.recording_limit_minutes) * 60);
                        limit_warned = false;
                        let levels = started.levels();
                        capture_levels = Some(levels.clone());
                        recorder = Some(started);
                        background = Some(session);
                        status = status
                            .next(AppEvent::Toggle, None)
                            .expect("ready can always start recording");
                        send_status(&updates, &status);
                        let _ = updates.send(EngineUpdate::RecordingStarted(levels));
                    }
                    Err(error) => {
                        #[cfg(all(target_os = "linux", feature = "ask"))]
                        {
                            if ask_image.take().is_some() {
                                speech = None;
                                let _ = updates
                                    .send(EngineUpdate::Ask(serde_json::json!({"error": error})));
                            }
                        }
                        #[cfg(target_os = "linux")]
                        if let Some(media) = &media {
                            media_notice(&updates, media.resume());
                        }
                        recover_from_error(&updates, &mut status, error);
                    }
                }
                if let Some(notice) = input_notice {
                    let _ = updates.send(EngineUpdate::Notice(notice));
                }
            }
            EngineCommand::Toggle | EngineCommand::Stop if status == AppStatus::Recording => {
                status = status
                    .next(AppEvent::Toggle, None)
                    .expect("recording can always stop");
                send_status(&updates, &status);

                #[cfg(all(target_os = "linux", feature = "ask"))]
                if ask_image.is_some() {
                    let _ = updates.send(EngineUpdate::Ask(
                        serde_json::json!({"phase": "Transcribing locally…"}),
                    ));
                }
                capture_started = None;
                let audio = recorder
                    .take()
                    .ok_or_else(|| "the microphone recorder is missing".to_string())
                    .and_then(MicrophoneRecorder::finish_with_warning);
                #[cfg(target_os = "linux")]
                if let Some(media) = &media {
                    media_notice(&updates, media.resume());
                }
                let result = (|| {
                    let session = background
                        .take()
                        .ok_or_else(|| "background transcription is missing".to_string())?;
                    let (audio, warning) = audio?;
                    #[cfg(target_os = "windows")]
                    if matches!(&source, CaptureSource::WavFixture { fail_asr: true, .. }) {
                        drop(session);
                        transcription.stop_for_windows_lab();
                        return transcription.transcribe(audio);
                    }
                    if let Some(warning) = warning {
                        let _ = updates.send(EngineUpdate::Notice(warning));
                        drop(session);
                        transcription.transcribe(audio)
                    } else {
                        transcription.finish(session, audio)
                    }
                })();
                match result {
                    Ok(mut transcript) => {
                        #[cfg(all(target_os = "linux", feature = "ask"))]
                        if let Some(image) = ask_image.take() {
                            let storage =
                                crate::history::append_question_resilient(&model, &transcript.text);
                            let warning = match storage {
                                Ok(None) => None,
                                Ok(Some(_)) => Some("Question saved in recovery".to_string()),
                                Err(error) => Some(error),
                            };
                            if ask_cancelled.load(Ordering::Acquire) {
                                let _ = updates.send(EngineUpdate::Ask(serde_json::json!({"question": transcript.text, "warning": warning, "phase": "Cancelled"})));
                                status = AppStatus::Ready;
                                send_status(&updates, &status);
                                continue;
                            }
                            let _ = updates.send(EngineUpdate::Ask(serde_json::json!({"question": transcript.text, "warning": warning, "phase": "Answering…"})));
                            if transcript.text.trim().is_empty() {
                                speech = None;
                                let _ = updates.send(EngineUpdate::Ask(serde_json::json!({"error": "No question heard; try Ask again"})));
                                status = AppStatus::Ready;
                                send_status(&updates, &status);
                                continue;
                            }
                            match crate::assistant::Configuration::load() {
                                Ok(config) => {
                                    answer = Some(crate::assistant::Answer::start(
                                        transcript.text,
                                        image,
                                        config,
                                        ask_cancelled.clone(),
                                    ))
                                }
                                Err(error) => {
                                    let _ = updates.send(EngineUpdate::Ask(
                                        serde_json::json!({"error": error}),
                                    ));
                                }
                            }
                            status = AppStatus::Ready;
                            send_status(&updates, &status);
                            continue;
                        }

                        let session = sessions.current();
                        if sessions.is_cancelled(session) {
                            let _ =
                                updates.send(EngineUpdate::Notice("Dictation cancelled".into()));
                            status = status
                                .next(AppEvent::TranscriptionFinished, None)
                                .expect("transcription can always finish");
                            send_status(&updates, &status);
                            continue;
                        }
                        let spelling = Settings::load()
                            .map(|saved| saved.dictionary)
                            .unwrap_or_else(|_| dictionary.current().to_vec());
                        let prepared = prepare_dictation_text(&mut transcript, &spelling, |text| {
                            if settings.formatting.enabled && !text.is_empty() {
                                cleaner
                                    .as_mut()
                                    .map(|active| active.clean(text, settings.formatting))
                            } else {
                                None
                            }
                        });
                        if let Err(error) = prepared {
                            let _ =
                                updates.send(EngineUpdate::Notice(format!("Raw active: {error}")));
                            if cleaner.as_ref().is_some_and(|active| active.stalled()) {
                                cleaner = None;
                            }
                        }
                        let _ = updates.send(EngineUpdate::TranscriptReady {
                            model: model.clone(),
                            transcript,
                            session,
                        });
                        status = status
                            .next(AppEvent::TranscriptionFinished, None)
                            .expect("transcription can always finish");
                        send_status(&updates, &status);
                    }
                    Err(error) => {
                        #[cfg(all(target_os = "linux", feature = "ask"))]
                        {
                            if ask_image.take().is_some() {
                                speech = None;
                                let _ = updates
                                    .send(EngineUpdate::Ask(serde_json::json!({"error": error})));
                            }
                        }
                        recover_from_error(&updates, &mut status, error);
                    }
                }
            }
            EngineCommand::Cancel if status == AppStatus::Recording => {
                sessions.cancel_current();
                drop(recorder.take());
                drop(background.take());
                capture_started = None;
                #[cfg(all(target_os = "linux", feature = "ask"))]
                if ask_image.take().is_some() {
                    speech = None;
                    let _ = updates.send(EngineUpdate::Ask(
                        serde_json::json!({"phase": "Cancelled", "speech_notice": ""}),
                    ));
                }
                #[cfg(target_os = "linux")]
                if let Some(media) = &media {
                    media_notice(&updates, media.resume());
                }
                status = AppStatus::Ready;
                send_status(&updates, &status);
                let _ = updates.send(EngineUpdate::Notice("Dictation cancelled".into()));
            }
            // A Cancel that queued behind Stop was already applied through
            // `EngineHandle::cancel_dictation`; nothing is left to stop.
            EngineCommand::Toggle
            | EngineCommand::Start
            | EngineCommand::Stop
            | EngineCommand::Cancel => {}
        }
    }
    crate::formatting::CANCELLED.store(true, Ordering::Release);
    if let Some((receiver, worker)) = preparation {
        drop(receiver);
        let _ = worker.join();
    }
    drop(cleaner);
    drop(background.take());
    drop(recorder.take());
    #[cfg(target_os = "linux")]
    if let Some(media) = &media {
        media_notice(&updates, media.resume());
    }
    #[cfg(target_os = "linux")]
    drop(media_operation);
    transcription.stop();
}

type FormattingPreparation = (
    Receiver<Result<(crate::formatting::Cleaner, crate::formatting::Formatting), String>>,
    JoinHandle<()>,
);
fn start_formatting(
    formatting: crate::formatting::Formatting,
    updates: Sender<EngineUpdate>,
) -> FormattingPreparation {
    crate::formatting::CANCELLED.store(false, Ordering::Release);
    let _ = updates.send(EngineUpdate::FormattingBusy(true));
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        let result = crate::formatting::Cleaner::prepare(|message| {
            let _ = updates.send(EngineUpdate::Notice(message));
        });
        match result {
            Ok(mut cleaner) => {
                let (keepalive, lifetime) = mpsc::channel();
                cleaner.keepalive = Some(keepalive);
                let _ = sender.send(Ok((cleaner, formatting)));
                let _ = lifetime.recv();
            }
            Err(error) => {
                let _ = sender.send(Err(error));
            }
        }
    });
    (receiver, worker)
}

fn prepare_engine(
    settings: &Settings,
    cancelled: &AtomicBool,
    updates: &Sender<EngineUpdate>,
) -> Result<(std::path::PathBuf, TranscriptionWorker), String> {
    let directory = match &settings.model_directory {
        Some(path) => path.clone(),
        None => model::ensure_model(settings.selected_model, cancelled, |progress| {
            let _ = updates.send(EngineUpdate::ModelProgress(progress));
        })?,
    };
    let worker = load_worker(
        &directory,
        settings.selected_model,
        &settings.dictionary,
        cancelled,
        updates,
    )?;
    Ok((directory, worker))
}

fn load_worker(
    directory: &Path,
    selection: model::LocalModel,
    dictionary: &[crate::dictionary::DictionaryEntry],
    cancelled: &AtomicBool,
    updates: &Sender<EngineUpdate>,
) -> Result<TranscriptionWorker, String> {
    if selection == model::LocalModel::Nemotron {
        if !cfg!(any(target_os = "linux", target_os = "windows")) {
            return Err(
                "Nemotron streaming is currently supported on Linux and Windows only".into(),
            );
        }
        let nemotron = crate::transcription::NemotronTranscriber::load(directory)?;
        let backup = model::ensure_model(model::LocalModel::ParakeetV2, cancelled, |progress| {
            let _ = updates.send(EngineUpdate::ModelProgress(progress));
        })?;
        TranscriptionWorker::start_with_nemotron(
            ParakeetTranscriber::load_with_dictionary(&backup, 2, dictionary)?,
            Some(nemotron),
        )
    } else {
        TranscriptionWorker::start(ParakeetTranscriber::load_with_dictionary(
            directory, 2, dictionary,
        )?)
    }
}

/// Follows the saved dictionary while idle, so Settings edits bias the next dictation.
fn prepare_dictation_text(
    transcript: &mut Transcript,
    entries: &[crate::dictionary::DictionaryEntry],
    cleanup: impl FnOnce(&str) -> Option<Result<String, String>>,
) -> Result<(), String> {
    if transcript.text.trim().is_empty() {
        return Ok(());
    }
    let before = transcript.text.clone();
    let spelled = crate::dictionary::apply(&before, entries);
    let (text, result, cleaned) = match cleanup(&spelled) {
        Some(Ok(text)) => (crate::dictionary::apply(&text, entries), Ok(()), true),
        Some(Err(error)) => (spelled, Err(error), false),
        None => (spelled, Ok(()), false),
    };
    if transcript.raw.is_none() && (text != before || cleaned) {
        transcript.raw = Some(before);
    }
    transcript.text = text;
    result
}

#[cfg(test)]
mod dictionary_cleanup_tests {
    use super::*;
    fn transcript() -> Transcript {
        Transcript {
            raw: None,
            text: "Use fuid audio".into(),
            words: vec![],
            audio_duration: Duration::ZERO,
            transcription: Duration::ZERO,
        }
    }
    #[test]
    fn empty_asr_never_reaches_cleanup_or_changes_raw_recovery() {
        for empty in ["", " \n\t", "\u{2003}"] {
            let mut text = transcript();
            text.text = empty.into();
            text.raw = Some(empty.into());
            prepare_dictation_text(&mut text, &[], |_| panic!("empty ASR reached cleanup"))
                .unwrap();
            assert_eq!(text.text, empty);
            assert_eq!(text.raw.as_deref(), Some(empty));
        }
    }

    #[test]
    fn cleanup_sees_spelled_text_and_output_is_spelled_again() {
        let dictionary = [crate::dictionary::DictionaryEntry::new(
            "FluidAudio",
            &["fuid audio"],
        )];
        let mut text = transcript();
        prepare_dictation_text(&mut text, &dictionary, |input| {
            assert_eq!(input, "Use FluidAudio");
            Some(Ok("Use fuid audio!".into()))
        })
        .unwrap();
        assert_eq!(text.text, "Use FluidAudio!");
        assert_eq!(text.raw.as_deref(), Some("Use fuid audio"));
    }
    #[test]
    fn cleanup_failure_retains_spelled_raw_and_original_recovery_text() {
        let dictionary = [crate::dictionary::DictionaryEntry::new(
            "FluidAudio",
            &["fuid audio"],
        )];
        let mut text = transcript();
        assert!(
            prepare_dictation_text(&mut text, &dictionary, |_| Some(Err("timeout".into())))
                .is_err()
        );
        assert_eq!(text.text, "Use FluidAudio");
        assert_eq!(text.raw.as_deref(), Some("Use fuid audio"));
    }
}

struct DictionarySync {
    entries: Vec<crate::dictionary::DictionaryEntry>,
    checked: std::time::Instant,
}

impl DictionarySync {
    fn new(entries: Vec<crate::dictionary::DictionaryEntry>) -> Self {
        Self {
            entries,
            checked: std::time::Instant::now(),
        }
    }

    fn current(&self) -> &[crate::dictionary::DictionaryEntry] {
        &self.entries
    }

    fn sync(&mut self, worker: &TranscriptionWorker) {
        if self.checked.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.checked = std::time::Instant::now();
        if let Ok(saved) = Settings::load()
            && saved.dictionary != self.entries
        {
            self.entries = saved.dictionary;
            worker.set_dictionary(self.entries.clone());
        }
    }
}

/// A failed media job says so on the pill. Cancelling it was the user's own doing.
#[cfg(target_os = "linux")]
fn media_failed(updates: &Sender<EngineUpdate>, error: String, cancelled: &AtomicBool) {
    let notice = if cancelled.load(Ordering::Acquire) {
        error
    } else {
        format!("{MEDIA_ERROR}{error}")
    };
    let _ = updates.send(EngineUpdate::Notice(notice));
}

#[cfg(target_os = "linux")]
pub const MEDIA_ERROR: &str = "@media-error:";

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

pub const LIMIT_WARNING: &str = "1 minute left";
const LIMIT_WARNING_BEFORE: Duration = Duration::from_secs(60);
const PAUSE_POLL: Duration = Duration::from_millis(250);

fn limit_notice(limit: Duration) -> String {
    format!("Stopped at the {}-minute limit", limit.as_secs() / 60)
}

pub fn is_limit_notice(text: &str) -> bool {
    text.starts_with("Stopped at the ") && text.ends_with("-minute limit")
}

/// How long the recording loop may wait; zero means the next limit step is due.
/// Paused recordings keep polling and never come due.
fn limit_wait(
    started: Option<std::time::Instant>,
    levels: Option<&AudioLevels>,
    limit: Duration,
    warned: bool,
) -> Duration {
    if levels.is_some_and(AudioLevels::paused) {
        return PAUSE_POLL;
    }
    let recorded = started
        .map(|started| started.elapsed())
        .unwrap_or_default()
        .saturating_sub(levels.map_or(Duration::ZERO, AudioLevels::paused_time));
    until_next_limit_step(limit, recorded, warned)
}

/// How long to wait for a command before the warning, then before the stop.
fn until_next_limit_step(limit: Duration, elapsed: Duration, warned: bool) -> Duration {
    if warned {
        limit
    } else {
        limit.saturating_sub(LIMIT_WARNING_BEFORE)
    }
    .saturating_sub(elapsed)
}

fn recording_limit_reached(updates: &Sender<EngineUpdate>, limit: Duration) -> EngineCommand {
    if std::env::var_os("KOETT_CAPTURE_TIMINGS").is_some()
        && let Ok(elapsed) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
    {
        eprintln!("capture_limit_stop_unix_ns={}", elapsed.as_nanos());
    }
    let _ = updates.send(EngineUpdate::Notice(limit_notice(limit)));
    EngineCommand::Toggle
}

fn send_status(updates: &Sender<EngineUpdate>, status: &AppStatus) {
    let _ = updates.send(EngineUpdate::Status(status.clone()));
}

fn send_error(updates: &Sender<EngineUpdate>, error: String) {
    let _ = updates.send(EngineUpdate::Status(AppStatus::Error(error)));
}

fn begin_model_switch(shutdown: &AtomicBool, setup_cancelled: &AtomicBool) -> bool {
    if shutdown.load(Ordering::Acquire) {
        return false;
    }
    setup_cancelled.store(false, Ordering::Release);
    if shutdown.load(Ordering::Acquire) {
        setup_cancelled.store(true, Ordering::Release);
        return false;
    }
    true
}

fn model_name(directory: &Path) -> String {
    directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("local-model")
        .to_string()
}

#[cfg(target_os = "linux")]
type MediaOperation = (
    crate::media_transcription::Fetch,
    Option<crate::media_transcription::Download>,
    Option<Receiver<Result<String, String>>>,
);

#[cfg(test)]
mod tests {
    use std::time::Duration;

    #[test]
    fn recording_limit_stops_the_recording_and_says_why() {
        let (updates, received) = std::sync::mpsc::channel();
        assert!(matches!(
            super::recording_limit_reached(&updates, Duration::from_secs(60 * 60)),
            super::EngineCommand::Toggle
        ));
        assert!(matches!(
            received.try_recv(),
            Ok(super::EngineUpdate::Notice(message))
                if message == "Stopped at the 60-minute limit"
                    && super::is_limit_notice(&message)
        ));
        assert!(!super::is_limit_notice(super::LIMIT_WARNING));
    }

    #[test]
    fn paused_time_does_not_count_toward_the_limit() {
        let levels = crate::audio::AudioLevels::new();
        let limit = Duration::from_secs(60 * 60);
        let started = Some(std::time::Instant::now() - Duration::from_secs(59 * 60 + 1));
        assert!(super::limit_wait(started, Some(&levels), limit, false).is_zero());
        levels.set_paused(true);
        assert_eq!(
            super::limit_wait(started, Some(&levels), limit, false),
            super::PAUSE_POLL
        );
        std::thread::sleep(Duration::from_millis(30));
        levels.set_paused(false);
        assert!(levels.paused_time() >= Duration::from_millis(30));
    }

    #[test]
    fn the_warning_comes_a_minute_before_the_stop() {
        let limit = Duration::from_secs(60 * 60);
        let wait = |elapsed, warned| {
            super::until_next_limit_step(limit, Duration::from_secs(elapsed), warned).as_secs()
        };
        assert_eq!(wait(0, false), 59 * 60);
        assert_eq!(wait(11 * 60, false), 48 * 60);
        assert_eq!(wait(59 * 60, false), 0);
        assert_eq!(wait(59 * 60, true), 60);
        assert_eq!(wait(60 * 60, true), 0);
        assert_eq!(wait(61 * 60, true), 0);
    }

    use std::path::Path;

    use super::{begin_model_switch, model_name};
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn shutdown_cannot_be_reset_by_queued_model_setup() {
        let shutdown = AtomicBool::new(true);
        let setup = AtomicBool::new(true);
        assert!(!begin_model_switch(&shutdown, &setup));
        assert!(setup.load(Ordering::Acquire));
        shutdown.store(false, Ordering::Release);
        assert!(begin_model_switch(&shutdown, &setup));
        assert!(!setup.load(Ordering::Acquire));
    }

    #[test]
    fn model_name_uses_the_model_folder() {
        assert_eq!(
            model_name(Path::new("models/parakeet-110m")),
            "parakeet-110m"
        );
    }
}
