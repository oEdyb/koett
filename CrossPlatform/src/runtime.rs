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
    },
}

pub struct EngineHandle {
    pub commands: Sender<EngineCommand>,
    pub updates: Receiver<EngineUpdate>,
    cancelled: Arc<AtomicBool>,
    ask_cancelled: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl EngineHandle {
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

pub fn start(settings: Settings) -> EngineHandle {
    let (command_sender, command_receiver) = mpsc::channel();
    let (update_sender, update_receiver) = mpsc::channel();
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = cancelled.clone();
    let ask_cancelled = Arc::new(AtomicBool::new(false));
    let worker_ask_cancelled = ask_cancelled.clone();
    model::SWITCH_CANCELLED.store(false, Ordering::Release);
    let worker = thread::spawn(move || {
        run(
            settings,
            command_receiver,
            update_sender,
            worker_cancelled,
            worker_ask_cancelled,
        )
    });

    EngineHandle {
        commands: command_sender,
        updates: update_receiver,
        cancelled,
        ask_cancelled,
        worker: Some(worker),
    }
}

fn run(
    mut settings: Settings,
    commands: Receiver<EngineCommand>,
    updates: Sender<EngineUpdate>,
    cancelled: Arc<AtomicBool>,
    ask_cancelled: Arc<AtomicBool>,
) {
    #[cfg(not(target_os = "linux"))]
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
    let mut recorder = None;
    let mut background: Option<BackgroundSession> = None;
    #[cfg(target_os = "linux")]
    let mut media_operation: Option<MediaOperation> = None;
    #[cfg(target_os = "linux")]
    let mut ask_image: Option<Vec<u8>> = None;
    #[cfg(target_os = "linux")]
    let mut answer: Option<crate::assistant::Answer> = None;
    #[cfg(target_os = "linux")]
    let mut screen_capture: Option<crate::assistant::screen::Capture> = None;
    #[cfg(target_os = "linux")]
    let mut speech: Option<crate::assistant::speech::Speech> = None;
    loop {
        #[cfg(target_os = "linux")]
        if (ask_image.is_some() || answer.is_some())
            && let Some(active) = &speech
        {
            for notice in active.notices.try_iter() {
                let _ = updates.send(EngineUpdate::Ask(
                    serde_json::json!({"speech_notice": notice}),
                ));
            }
        }
        #[cfg(target_os = "linux")]
        if ask_cancelled.load(Ordering::Acquire) {
            speech = None;
        }
        #[cfg(target_os = "linux")]
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
                                let _ = updates.send(EngineUpdate::Notice(error));
                                media_operation = None;
                                status = AppStatus::Ready;
                                send_status(&updates, &status);
                            }
                        }
                    }
                    Ok(Err(error)) => {
                        let _ = updates.send(EngineUpdate::Notice(error));
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
                        let _ = updates.send(EngineUpdate::Notice(error));
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
                        Settings::load().and_then(|mut current| {
                            current.formatting = formatting;
                            current.save()
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
        #[cfg(target_os = "linux")]
        let screen_result = screen_capture
            .as_ref()
            .and_then(|capture| capture.result.try_recv().ok());
        #[cfg(target_os = "linux")]
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
        #[cfg(not(target_os = "linux"))]
        let capture_start = false;
        let command = if capture_start {
            EngineCommand::Start
        } else if status == AppStatus::Recording {
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
            match commands.recv_timeout(Duration::from_millis(50)) {
                Ok(command) => command,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        };
        #[cfg(target_os = "linux")]
        let stopping_question = matches!(command, EngineCommand::AskStop | EngineCommand::Stop)
            || matches!(command, EngineCommand::Ask | EngineCommand::Toggle)
                && ask_image.is_some()
                && status == AppStatus::Recording;
        #[cfg(target_os = "linux")]
        if !(capture_start || stopping_question) {
            speech = None;
        }
        #[cfg(target_os = "linux")]
        let command = match command {
            EngineCommand::AskStart if status == AppStatus::Ready => EngineCommand::Ask,
            EngineCommand::AskStop if status == AppStatus::Recording && ask_image.is_some() => {
                EngineCommand::Ask
            }
            EngineCommand::AskStart | EngineCommand::AskStop => continue,
            other => other,
        };
        #[cfg(target_os = "linux")]
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
        #[cfg(target_os = "linux")]
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
        #[cfg(target_os = "linux")]
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
        #[cfg(target_os = "linux")]
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
            | EngineCommand::AskStop => unreachable!(),
            #[cfg(target_os = "linux")]
            EngineCommand::Media(source) => {
                answer = None;
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
                    let saved = Settings::load().and_then(|mut current| {
                        current.formatting = formatting;
                        current.save()
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
                        let worker =
                            load_worker(&directory, selection, &model::SWITCH_CANCELLED, &updates)?;
                        Ok((directory, worker))
                    });
                match candidate {
                    Ok((directory, worker)) if !model::SWITCH_CANCELLED.load(Ordering::Acquire) => {
                        let saved = Settings::load().and_then(|mut current| {
                            current.selected_model = selection;
                            current.model_directory = None;
                            current.save()
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
                                let _ = updates.send(EngineUpdate::Notice(error));
                            }
                        }
                    }
                    Ok((_, worker)) => worker.stop(),
                    Err(error) => {
                        let _ = updates.send(EngineUpdate::Notice(error));
                    }
                }
                status = AppStatus::Ready;
                send_status(&updates, &status);
            }
            EngineCommand::SwitchModel(_) => {
                let _ = updates.send(EngineUpdate::Notice(
                    "Stop recording before changing models".into(),
                ));
            }
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
                        #[cfg(target_os = "linux")]
                        let offline = ask_image.is_some();
                        #[cfg(not(target_os = "linux"))]
                        let offline = false;
                        let session = if offline {
                            transcription.record_offline(started.sample_rate(), audio_chunks)?
                        } else {
                            transcription.record(started.sample_rate(), audio_chunks)?
                        };
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
            }
            EngineCommand::Toggle | EngineCommand::Stop if status == AppStatus::Recording => {
                status = status
                    .next(AppEvent::Toggle, None)
                    .expect("recording can always stop");
                send_status(&updates, &status);

                #[cfg(target_os = "linux")]
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
                        #[cfg(target_os = "linux")]
                        if let Some(image) = ask_image.take() {
                            let storage = crate::history::append_transcript_resilient(
                                &model,
                                &transcript.text,
                            );
                            let warning = match storage {
                                Ok(None) => None,
                                Ok(Some(_)) => Some("Question saved in recovery".to_string()),
                                Err(_) => Some(
                                    "Question storage failed; copy it before closing".to_string(),
                                ),
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

                        if settings.formatting.enabled
                            && !transcript.text.is_empty()
                            && let Some(cleaner) = cleaner.as_mut()
                        {
                            match cleaner.clean(&transcript.text, settings.formatting) {
                                Ok(text) => {
                                    transcript.raw =
                                        Some(std::mem::replace(&mut transcript.text, text));
                                }
                                Err(error) => {
                                    let _ = updates
                                        .send(EngineUpdate::Notice(format!("Raw active: {error}")));
                                }
                            }
                        }
                        let _ = updates.send(EngineUpdate::TranscriptReady {
                            model: model.clone(),
                            transcript,
                        });
                        status = status
                            .next(AppEvent::TranscriptionFinished, None)
                            .expect("transcription can always finish");
                        send_status(&updates, &status);
                    }
                    Err(error) => {
                        #[cfg(target_os = "linux")]
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
            EngineCommand::Toggle | EngineCommand::Start | EngineCommand::Stop => {}
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
    let worker = load_worker(&directory, settings.selected_model, cancelled, updates)?;
    Ok((directory, worker))
}

fn load_worker(
    directory: &Path,
    selection: model::LocalModel,
    cancelled: &AtomicBool,
    updates: &Sender<EngineUpdate>,
) -> Result<TranscriptionWorker, String> {
    if selection == model::LocalModel::Nemotron {
        if !cfg!(target_os = "linux") {
            return Err("Nemotron streaming is currently supported on Linux only".into());
        }
        let nemotron = crate::transcription::NemotronTranscriber::load(directory)?;
        let backup = model::ensure_model(model::LocalModel::ParakeetV2, cancelled, |progress| {
            let _ = updates.send(EngineUpdate::ModelProgress(progress));
        })?;
        TranscriptionWorker::start_with_nemotron(
            ParakeetTranscriber::load(&backup, 2)?,
            Some(nemotron),
        )
    } else {
        TranscriptionWorker::start(ParakeetTranscriber::load(directory, 2)?)
    }
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
