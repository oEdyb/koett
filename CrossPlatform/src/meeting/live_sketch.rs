use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::bundle::Bundle;
use super::*;

pub(crate) fn finish_with_sketches(
    root: &Path,
    zero: Instant,
    finish_at: Instant,
    draw: impl FnOnce(MeetingScope, &Path),
) -> (PathBuf, Seal) {
    let info = SessionInfo {
        id: MeetingId::new(),
        started_at_ms: 123,
    };
    let attempt = Attempt(1);
    let epoch = MeetingEpoch(zero);
    let mut bundle = Bundle::create(
        root,
        Manifest {
            info: info.clone(),
            mic: None,
            backend_pref: Backend::Parakeet,
            keep_audio: false,
            export_dir: None,
        },
    )
    .unwrap();
    let marks = bundle.start_marks().unwrap();
    let directory = bundle.directory().to_owned();
    let (active, effects, _) = next(
        Active::Opening { info, attempt },
        Event::Opened(
            attempt,
            Ok(OpenedCapture {
                capture: LiveCapture::Silent(session::SilentCapture {
                    marks,
                    bundle: Arc::new(bundle),
                }),
                epoch,
                health: CaptureHealth::default(),
                levels: Tracks::default(),
            }),
        ),
    );
    let [
        Effect::Notify(MeetingUpdate::Sketch(SketchScope::Open(scope))),
        ..,
    ] = effects.as_slice()
    else {
        panic!("missing live meeting sketch scope");
    };
    draw(scope.clone(), &directory);
    let Active::Recording(live) = active else {
        panic!("meeting did not start recording");
    };
    let spec = CloseSpec {
        cutoff: live.epoch.offset(finish_at),
        goal: CloseGoal::Finish,
    };
    let SealedBundle::Locked { bundle, seal } = session::drain(live.capture, spec).unwrap() else {
        panic!("meeting did not seal its bundle");
    };
    let rendered = transcript::render_marks(&bundle, &seal).unwrap();
    bundle
        .publish(&rendered.markdown, rendered.publication)
        .unwrap();
    bundle.apply_retention().unwrap();
    (directory, seal)
}
