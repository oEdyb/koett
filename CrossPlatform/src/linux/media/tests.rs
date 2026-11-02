use super::*;
use std::sync::Mutex as StateMutex;
use zbus::object_server::SignalEmitter;

const TEST_NAME: &str = "org.mpris.MediaPlayer2.KoettTest";

#[derive(Default)]
struct State {
    status: String,
    can_pause: bool,
    pauses: usize,
    plays: usize,
    refuse: bool,
    stall: bool,
}

struct Player(Arc<StateMutex<State>>);

#[zbus::interface(name = "org.mpris.MediaPlayer2.Player", crate = "ashpd::zbus")]
impl Player {
    async fn pause(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        let (refuse, stall) = {
            let state = self.0.lock().unwrap();
            (state.refuse, state.stall)
        };
        if refuse {
            return Err(zbus::fdo::Error::Failed("Refused".into()));
        }
        if stall {
            tokio::time::sleep(Duration::from_secs(2)).await;
            return Ok(());
        }
        {
            let mut state = self.0.lock().unwrap();
            state.pauses += 1;
            state.status = "Paused".into();
        }
        self.playback_status_changed(&emitter).await?;
        Ok(())
    }

    fn play(&self) {
        let mut state = self.0.lock().unwrap();
        state.plays += 1;
        state.status = "Playing".into();
    }

    #[zbus(property)]
    fn playback_status(&self) -> String {
        self.0.lock().unwrap().status.clone()
    }
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        self.0.lock().unwrap().can_pause
    }
    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        HashMap::from([(
            "mpris:trackid".into(),
            zbus::zvariant::ObjectPath::try_from("/track/one")
                .unwrap()
                .into(),
        )])
    }
}

/// Launch the child test on an entirely disposable bus, never the user's bus.
#[test]
fn disposable_mpris_bus() {
    let output = std::process::Command::new("dbus-run-session")
        .arg("--")
        .arg(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &format!(
                "{}::mpris_disposable_child",
                module_path!().split_once("::").unwrap().1
            ),
            "--nocapture",
        ])
        .env("KOETT_DISPOSABLE_MPRIS_TEST", "1")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("running 1 test"));
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn mpris_disposable_child() {
    if std::env::var("KOETT_DISPOSABLE_MPRIS_TEST").as_deref() != Ok("1") {
        return;
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let state = Arc::new(StateMutex::new(State {
            status: "Playing".into(),
            can_pause: true,
            ..State::default()
        }));
        let service = zbus::connection::Builder::session()
            .unwrap()
            .name(TEST_NAME)
            .unwrap()
            .serve_at(PATH, Player(state.clone()))
            .unwrap()
            .build()
            .await
            .unwrap();
        let controller = MediaController::new().unwrap();

        // Sync public calls inside Tokio prove there is no nested runtime.
        controller.pause().unwrap();
        controller.pause().unwrap();
        assert_eq!(state.lock().unwrap().pauses, 1);
        controller.resume().unwrap();
        controller.resume().unwrap();
        assert_eq!(state.lock().unwrap().plays, 1);

        // Already paused and incapable players are untouched.
        state.lock().unwrap().status = "Paused".into();
        controller.pause().unwrap();
        controller.resume().unwrap();
        assert_eq!(state.lock().unwrap().pauses, 1);
        {
            let mut state = state.lock().unwrap();
            state.status = "Playing".into();
            state.can_pause = false;
        }
        controller.pause().unwrap();
        controller.resume().unwrap();
        assert_eq!(state.lock().unwrap().pauses, 1);
        state.lock().unwrap().can_pause = true;

        // A failed Pause does not authorize Play.
        state.lock().unwrap().refuse = true;
        assert!(controller.pause().is_err());
        state.lock().unwrap().status = "Paused".into();
        controller.resume().unwrap();
        assert_eq!(state.lock().unwrap().plays, 1);
        state.lock().unwrap().refuse = false;

        // A user playing then pausing again invalidates ownership, even
        // though the final property snapshot matches Koett's pause.
        state.lock().unwrap().status = "Playing".into();
        controller.pause().unwrap();
        for status in ["Playing", "Paused"] {
            let changed: Properties = HashMap::from([(
                "PlaybackStatus".into(),
                zbus::zvariant::Str::from(status).into(),
            )]);
            service
                .emit_signal(
                    None::<&str>,
                    PATH,
                    "org.freedesktop.DBus.Properties",
                    "PropertiesChanged",
                    &(PLAYER, changed, Vec::<String>::new()),
                )
                .await
                .unwrap();
        }
        controller.resume().unwrap();
        assert_eq!(state.lock().unwrap().plays, 1);

        // A seek while paused also revokes ownership.
        state.lock().unwrap().status = "Playing".into();
        controller.pause().unwrap();
        service
            .emit_signal(None::<&str>, PATH, PLAYER, "Seeked", &(100_i64,))
            .await
            .unwrap();
        controller.resume().unwrap();
        assert_eq!(state.lock().unwrap().plays, 1);

        // Losing and reacquiring even the same unique owner is intervention.
        state.lock().unwrap().status = "Playing".into();
        controller.pause().unwrap();
        service.release_name(TEST_NAME).await.unwrap();
        service.request_name(TEST_NAME).await.unwrap();
        controller.resume().unwrap();
        assert_eq!(state.lock().unwrap().plays, 1);

        // Metadata/property changes revoke ownership, even with unchanged status.
        state.lock().unwrap().status = "Playing".into();
        controller.pause().unwrap();
        let metadata: Properties = HashMap::from([(
            "mpris:trackid".into(),
            zbus::zvariant::ObjectPath::try_from("/track/two")
                .unwrap()
                .into(),
        )]);
        let changed: Properties = HashMap::from([("Metadata".into(), metadata.into())]);
        service
            .emit_signal(
                None::<&str>,
                PATH,
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                &(PLAYER, changed, Vec::<String>::new()),
            )
            .await
            .unwrap();
        controller.resume().unwrap();
        assert_eq!(state.lock().unwrap().plays, 1);

        // A replacement with the same name and properties is not our player.
        state.lock().unwrap().status = "Playing".into();
        controller.pause().unwrap();
        service.release_name(TEST_NAME).await.unwrap();
        let replacement_state = Arc::new(StateMutex::new(State {
            status: "Paused".into(),
            can_pause: true,
            ..State::default()
        }));
        let replacement = zbus::connection::Builder::session()
            .unwrap()
            .name(TEST_NAME)
            .unwrap()
            .serve_at(PATH, Player(replacement_state.clone()))
            .unwrap()
            .build()
            .await
            .unwrap();
        controller.resume().unwrap();
        assert_eq!(replacement_state.lock().unwrap().plays, 0);
        replacement.release_name(TEST_NAME).await.unwrap();
        service.request_name(TEST_NAME).await.unwrap();

        // Disappearance cannot transfer pause ownership to another player.
        state.lock().unwrap().status = "Playing".into();
        controller.pause().unwrap();
        service.release_name(TEST_NAME).await.unwrap();
        assert!(controller.resume().is_err());
        service.request_name(TEST_NAME).await.unwrap();
        controller.resume().unwrap();
        assert_eq!(state.lock().unwrap().plays, 1);

        // A nonresponding service never holds up capture indefinitely.
        {
            let mut state = state.lock().unwrap();
            state.status = "Playing".into();
            state.stall = true;
        }
        let started = Instant::now();
        assert!(controller.pause().is_err());
        assert!(started.elapsed() < ACK_WAIT + Duration::from_millis(100));
        controller.resume().unwrap();
        assert_eq!(state.lock().unwrap().plays, 1);
        state.lock().unwrap().stall = false;

        state.lock().unwrap().status = "Playing".into();
        controller.pause().unwrap();
        controller.shutdown().unwrap();
        controller.shutdown().unwrap();
        assert_eq!(state.lock().unwrap().plays, 2);
        let cleanup = MediaController::new().unwrap();
        cleanup.pause().unwrap();
        drop(cleanup);
        assert_eq!(state.lock().unwrap().plays, 3);
    });
}
