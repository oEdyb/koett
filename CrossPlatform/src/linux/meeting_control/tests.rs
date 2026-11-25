use super::*;
use crate::linux::window::Update;
use crate::meeting::bundle::Bundle;
use crate::meeting::bundle::records::{
    ExportProjection, HistoryProjection, HistoryReceipt, Intent, Manifest, Projections,
    Publication, Seal,
};
use crate::meeting::{CaptureHealth, Offset, SketchScope, Tracks};
use std::sync::{Arc, Barrier};
use std::thread;

const ID: &str = "550e8400-e29b-41d4-a716-446655440000";
const STARTED: u64 = 1764000312000;

struct TestRoot(std::path::PathBuf);

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn info() -> SessionInfo {
    SessionInfo {
        id: MeetingId::parse(ID).unwrap(),
        started_at_ms: STARTED,
    }
}

fn stop_key() -> StopKey {
    StopKey {
        id: info().id,
        generation: 3,
    }
}

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

#[test]
fn each_cli_command_maps_to_the_contract_request() {
    use MeetingRequest::*;
    let id = info().id;
    let key = stop_key();
    for (input, request) in [
        (vec!["start", "--id", ID], Start(id)),
        (vec!["stop", ID], Stop(id)),
        (vec!["resume", ID, "3"], Resume(key)),
        (vec!["finish", ID, "3"], Finish(key)),
        (vec!["cancel", ID, "3"], Cancel(key)),
        (vec!["recover", ID], Recover { id, backend: None }),
        (
            vec!["recover", ID, "--backend", "gpu"],
            Recover {
                id,
                backend: Some(Backend::Gpu),
            },
        ),
        (
            vec!["recover", ID, "--backend", "parakeet"],
            Recover {
                id,
                backend: Some(Backend::Parakeet),
            },
        ),
        (vec!["toggle"], Shortcut),
        (vec!["status"], Status(None)),
        (vec!["status", ID], Status(Some(id))),
    ] {
        assert!(matches!(parse(&args(&input)), Ok(Command::Daemon(actual)) if actual == request));
    }
    assert!(matches!(
        parse(&args(&["start"])),
        Ok(Command::Daemon(Start(_)))
    ));
    assert!(matches!(
        parse(&args(&["list"])),
        Ok(Command::List {
            recoverable: false,
            limit: 50
        })
    ));
    assert!(matches!(
        parse(&args(&["list", "--recoverable", "--limit", "7"])),
        Ok(Command::List {
            recoverable: true,
            limit: 7
        })
    ));
    assert!(matches!(parse(&args(&["get", ID])), Ok(Command::Get(actual)) if actual == id));
    assert!(matches!(
        parse(&args(&["search", "meeting", "--limit", "1"])),
        Ok(Command::Search { query, limit: 1 }) if query == "meeting"
    ));
    for input in [
        &["asr-check"][..],
        &["asr-check", "--fetch"],
        &["recover", ID, "--backend", "gpu"],
    ] {
        assert_eq!(
            offline(parse(&args(input)).unwrap(), Ok(false))["error"]["code"],
            "helper_unavailable"
        );
    }
    for input in [
        &["stop", "invalid"][..],
        &["resume", ID, "-1"],
        &["recover", ID, "--backend", "other"],
        &["list", "--limit", "-1"],
        &["toggle", "ignored"],
    ] {
        assert!(parse(&args(input)).is_err());
    }
}

#[test]
fn live_reply_goldens_cover_start_replay_stop_resume_finish_cancel_recover_status_and_toggle() {
    let recording = MeetingView::Recording {
        info: info(),
        health: CaptureHealth::default(),
    };
    let confirm = MeetingView::ConfirmStop {
        info: info(),
        key: stop_key(),
        health: CaptureHealth {
            tracks: Tracks {
                you: SourceHealth::Receiving,
                other: SourceHealth::Reconnecting,
            },
            disk_low: false,
        },
    };
    let recording_json = json!({"id":ID,"state":"recording","started_at_ms":STARTED,
        "capture":{"you":"receiving","other":"receiving"}});
    let confirm_json = json!({"id":ID,"state":"confirm_stop","started_at_ms":STARTED,"stop_token":"3",
        "capture":{"you":"receiving","other":"reconnecting"}});
    let processing = MeetingView::Processing {
        info: info(),
        done: 0.0,
    };
    for (input, view, data) in [
        (vec!["start"], recording.clone(), recording_json.clone()),
        (
            vec!["start", "--id", ID],
            recording.clone(),
            recording_json.clone(),
        ),
        (vec!["stop", ID], confirm.clone(), confirm_json.clone()),
        (vec!["resume", ID, "3"], recording, recording_json),
        (
            vec!["finish", ID, "3"],
            MeetingView::Closing {
                info: info(),
                goal: CloseGoal::Finish,
            },
            json!({"id":ID,"state":"closing","started_at_ms":STARTED,"goal":"finish"}),
        ),
        (
            vec!["cancel", ID, "3"],
            MeetingView::Closing {
                info: info(),
                goal: CloseGoal::Hold,
            },
            json!({"id":ID,"state":"closing","started_at_ms":STARTED,"goal":"hold"}),
        ),
        (
            vec!["recover", ID],
            processing,
            json!({"id":ID,"state":"processing","started_at_ms":STARTED,"done":0.0}),
        ),
        (vec!["status", ID], confirm.clone(), confirm_json.clone()),
        (vec!["toggle"], confirm, confirm_json),
    ] {
        let reply = socket_call(&input, Reply::View(view));
        assert_eq!(
            reply,
            json!({"version":1,"ok":true,"data":data}),
            "{input:?}"
        );
    }
    let reply = socket_call(
        &["finish", ID, "2"],
        Reply::Error(ReplyError::StaleStopToken),
    );
    assert_eq!(
        reply,
        json!({"version":1,"ok":false,"error":{"code":"stale_stop_token",
        "message":"Use the token from the current stop confirmation."}})
    );
}

fn socket_call(values: &[&str], reply: Reply) -> Value {
    let (actions, calls) = mpsc::channel();
    let (client, mut server) = UnixStream::pair().unwrap();
    let worker = thread::spawn(move || {
        let TrayAction::Meeting(call) = calls.recv().unwrap() else {
            panic!("expected meeting call")
        };
        call.test_reply(reply);
    });
    serve(&mut server, &serde_json::to_vec(values).unwrap(), &actions);
    drop(server);
    worker.join().unwrap();
    read_reply(client).unwrap()
}

#[test]
fn offline_list_get_and_status_return_disk_data_and_paths_without_transcript_text() {
    let root =
        TestRoot(std::env::temp_dir().join(format!("koett-meeting-control-{}", MeetingId::new())));
    let root = root.0.as_path();
    let mut held_info = info();
    held_info.id = MeetingId::new();
    held_info.started_at_ms += 1;
    let manifest = |info| Manifest {
        info,
        mic: None,
        backend_pref: Backend::Parakeet,
        keep_audio: true,
        export_dir: None,
    };
    let held = Bundle::create(root, manifest(held_info.clone())).unwrap();
    held.write_intent(&Intent {
        cutoff: Offset(32_000),
        goal: CloseGoal::Hold,
    })
    .unwrap();
    held.write_seal(&Seal {
        cutoff: Offset(32_000),
        tracks: Tracks::default(),
        mark_issues: vec![],
    })
    .unwrap();
    drop(held);

    let complete = Bundle::create(root, manifest(info())).unwrap();
    let secret = "Transcript text stays inside the bundle";
    complete
        .publish(
            secret,
            Publication {
                history: HistoryReceipt {
                    id: format!("meeting:{ID}"),
                    created_at_ms: STARTED,
                    model: "parakeet".into(),
                    text_sha256: "unused".into(),
                },
                assets: vec!["assets/mark-0000008000-0001.png".into()],
                backends: [("parakeet".into(), 1)].into(),
                fallback: false,
            },
        )
        .unwrap();
    complete
        .write_projections(&Projections {
            history: HistoryProjection::Indexed,
            export: ExportProjection::Disabled,
        })
        .unwrap();
    let markdown = complete.directory().join("transcript.md");
    let mark = complete.directory().join("assets/mark-0000008000-0001.png");
    std::fs::write(complete.directory().join("audio/you.journal"), b"retained").unwrap();
    drop(complete);

    assert_eq!(
        with_daemon(list(root, true, 1), Ok(false)),
        json!({
            "version":1,"ok":true,"data":{"daemon":false,"items":[{
                "id": held_info.id.to_string(), "state": "recoverable", "started_at_ms": STARTED + 1,
                "duration_ms": 2000, "reason": "held_by_user"
            }]}
        })
    );
    let artifact = with_daemon(get(root, info().id), Ok(false));
    assert_eq!(
        artifact,
        json!({"version":1,"ok":true,"data":{
            "daemon":false,"id":ID,"state":"complete","markdown":markdown,"assets":[mark],
            "backends":{"parakeet":1},"fallback":false,"audio_retained":true,
            "projections":{"history":"indexed","export":"disabled"}
        }})
    );
    assert_eq!(
        with_daemon(disk_status(root, Some(held_info.id)), Ok(false)),
        json!({
            "version":1,"ok":true,"data":{"daemon":false,"id":held_info.id.to_string(),
            "state":"recoverable","started_at_ms":STARTED + 1,"reason":"held_by_user"}
        })
    );
    assert!(!artifact.to_string().contains(secret));
    assert_eq!(get(root, held_info.id), Err(ReplyError::ArtifactPending));
}

#[test]
fn socket_enforces_exact_request_and_reply_size_boundaries() {
    use super::super::control::{MAX_REQUEST, read_request};
    for bytes in [MAX_REQUEST, MAX_REQUEST + 1] {
        let (mut server, mut client) = UnixStream::pair().unwrap();
        client.write_all(&vec![b'x'; bytes]).unwrap();
        client.shutdown(Shutdown::Write).unwrap();
        assert_eq!(
            read_request(&mut server).map(|value| value.len()),
            (bytes == MAX_REQUEST).then_some(bytes)
        );
    }
    let empty = json!("").to_string().len();
    let exact = json!("x".repeat(MAX_REPLY as usize - empty));
    let large = json!("x".repeat(MAX_REPLY as usize + 1 - empty));
    assert_eq!(bounded_reply(exact.clone()).len() as u64, MAX_REPLY);
    assert_eq!(read_reply(exact.to_string().as_bytes()).unwrap(), exact);
    assert_eq!(
        read_reply(large.to_string().as_bytes()).unwrap()["error"]["code"],
        "reply_too_large"
    );
    assert_eq!(
        read_reply(bounded_reply(large).as_bytes()).unwrap()["error"]["code"],
        "reply_too_large"
    );
}

#[test]
fn ticket_timeout_and_claim_race_have_one_winner() {
    let mut observed = (false, false);
    for round in 0..512 {
        let at = Instant::now();
        let ticket = Ticket::new(at + Duration::from_secs(2));
        let peer = ticket.clone();
        let barrier = Arc::new(Barrier::new(2));
        let start = barrier.clone();
        let claimant = thread::spawn(move || {
            start.wait();
            if round % 2 == 0 {
                thread::yield_now();
            }
            peer.claim(at)
        });
        barrier.wait();
        if round % 2 != 0 {
            thread::sleep(Duration::from_micros(50));
        }
        let code = ticket.timeout().code();
        let claimed = claimant.join().unwrap();
        assert_eq!(
            code,
            if claimed {
                "outcome_unknown"
            } else {
                "engine_busy"
            }
        );
        assert!(!ticket.claim(at));
        observed.0 |= claimed;
        observed.1 |= !claimed;
    }
    assert_eq!(observed, (true, true));
    let now = Instant::now();
    let deadline = Ticket::new(now);
    assert!(!deadline.claim(now));
    assert_eq!(deadline.timeout(), ReplyError::EngineBusy);
}

#[test]
fn meeting_updates_do_not_arm_the_dictation_sketch() {
    use crate::meeting::MeetingUpdate;
    let (ui, updates) = mpsc::channel();
    let views = [
        MeetingView::Idle,
        MeetingView::Recording {
            info: info(),
            health: CaptureHealth::default(),
        },
        MeetingView::ConfirmStop {
            info: info(),
            health: CaptureHealth::default(),
            key: stop_key(),
        },
        MeetingView::Closing {
            info: info(),
            goal: CloseGoal::Finish,
        },
        MeetingView::Processing {
            info: info(),
            done: 0.0,
        },
    ];
    for view in views {
        super::super::route_meeting_update(
            MeetingUpdate::View(view.clone(), 2),
            &ui,
            |seen, count| {
                assert_eq!(seen, &view);
                assert_eq!(count, 2);
            },
        );
        assert!(matches!(updates.try_recv(), Ok(Update::Meeting(seen)) if seen == view));
    }
    super::super::route_meeting_update(MeetingUpdate::Sketch(SketchScope::Closed), &ui, |_, _| ());
    assert!(matches!(
        updates.try_recv(),
        Ok(Update::MeetingSketch(SketchScope::Closed))
    ));
    super::super::route_meeting_update(
        MeetingUpdate::Issue("Recording source disconnected".into()),
        &ui,
        |_, _| (),
    );
    assert!(
        matches!(updates.try_recv(), Ok(Update::Status(status)) if !status.starts_with("Recording"))
    );
    super::super::route_meeting_update(MeetingUpdate::HistoryChanged, &ui, |_, _| ());
    assert!(
        matches!(updates.try_recv(), Ok(Update::Status(status)) if !status.starts_with("Recording"))
    );
}

#[test]
#[ignore = "private XDG socket fixture for the hardware-free meeting recover gate"]
fn s5_headless_fixture_daemon() {
    use crate::meeting::MeetingController;
    use crate::runtime::{EngineCommand, EngineUpdate};
    use crate::settings::Settings;
    use std::sync::atomic::AtomicU8;

    let runtime = std::env::var_os("XDG_RUNTIME_DIR").unwrap();
    assert!(Path::new(&runtime).join("s5-private-fixture").is_file());
    let display = std::env::var("WAYLAND_DISPLAY").unwrap();
    let stop = Path::new(&runtime).join(format!("{display}.stop"));
    let keep_running = std::env::var_os("KOETT_S5_KEEP_DAEMON").is_some();
    let listener =
        std::os::unix::net::UnixListener::bind(super::super::control::path().unwrap()).unwrap();
    let (actions, incoming) = mpsc::channel();
    thread::spawn(move || {
        for connection in listener.incoming() {
            let mut stream = connection.unwrap();
            if let Some(bytes) = super::super::control::read_request(&mut stream)
                && let Some(args) = bytes.strip_prefix(ROUTE)
            {
                let values: Vec<String> = serde_json::from_slice(args).unwrap();
                assert!(matches!(
                    parse(&values),
                    Ok(Command::Daemon(
                        MeetingRequest::Recover { .. } | MeetingRequest::Status(_)
                    ))
                ));
                serve(&mut stream, args, &actions);
            }
        }
    });

    static ACTIVITY: AtomicU8 = AtomicU8::new(0);
    let (commands, events) = mpsc::channel();
    let (updates, notices) = mpsc::channel();
    let settings = Settings::load().unwrap();
    let mut controller = MeetingController::new(commands, updates, &ACTIVITY);
    let mut ran_job = false;
    loop {
        if let Ok(TrayAction::Meeting(call)) = incoming.try_recv() {
            controller.dispatch(call, true, &settings);
        }
        while let Ok(EngineCommand::MeetingEvent(event)) = events.try_recv() {
            controller.receive(event, Instant::now());
        }
        for notice in notices.try_iter() {
            match notice {
                EngineUpdate::Meeting(crate::meeting::MeetingUpdate::View(
                    MeetingView::Processing { .. },
                    _,
                )) => ran_job = true,
                EngineUpdate::Meeting(crate::meeting::MeetingUpdate::View(
                    MeetingView::Idle,
                    _,
                )) if ran_job && !keep_running => return,
                _ => {}
            }
        }
        if stop.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn search_skips_and_counts_unreadable_hits_alongside_complete_meetings() {
    use crate::history::HistoryRecord;
    use std::fs;

    let root =
        TestRoot(std::env::temp_dir().join(format!("koett-meeting-search-{}", MeetingId::new())));
    let store_path = root.0.join("history.sqlite3");
    let meetings = root.0.join("meetings");
    let complete = Bundle::create(
        &meetings,
        Manifest {
            info: info(),
            mic: None,
            backend_pref: Backend::Parakeet,
            keep_audio: true,
            export_dir: None,
        },
    )
    .unwrap();
    let meeting_id = format!("meeting:{ID}");
    complete
        .publish(
            "# Meeting",
            Publication {
                history: HistoryReceipt {
                    id: meeting_id.clone(),
                    created_at_ms: STARTED,
                    model: "parakeet".into(),
                    text_sha256: "unused".into(),
                },
                assets: vec![],
                backends: Default::default(),
                fallback: false,
            },
        )
        .unwrap();
    drop(complete);

    let damaged_id = MeetingId::new();
    let damaged = meetings.join(damaged_id.to_string());
    fs::create_dir(&damaged).unwrap();
    fs::write(damaged.join("manifest.json"), "broken").unwrap();
    let store = HistoryStore::open(&store_path).unwrap();
    for (id, created_at_ms) in [(meeting_id, 1), (format!("meeting:{damaged_id}"), 2)] {
        store
            .save(&HistoryRecord {
                id,
                created_at_ms,
                model: "parakeet".into(),
                text: "Samoa surviving search hit".into(),
                raw_text: None,
            })
            .unwrap();
    }
    drop(store);

    assert!(matches!(
        get(&meetings, damaged_id),
        Err(ReplyError::DiskUnreadable(_))
    ));
    let result = search(&meetings, &store_path, "Samoa", 10).unwrap();
    assert_eq!(result["skipped_unreadable"], 1);
    assert_eq!(result["items"].as_array().unwrap().len(), 1);
    assert_eq!(result["items"][0]["id"], ID);
    assert!(
        result["items"][0]["snippet"]
            .as_str()
            .unwrap()
            .contains("Samoa")
    );
}
