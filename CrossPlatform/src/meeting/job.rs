use std::sync::atomic::{AtomicBool, Ordering};

use crate::settings::Settings;

use super::bundle::Bundle;
use super::bundle::records::{Job, JobError, Seal};
use super::bundle::scan::DiskState;
use super::session::seal;
use super::transcript;
use super::{
    Backend, JobInput, JobSource, Published, RecoverableBundle, RecoveryReason, SealedBundle,
};

pub(super) fn process_job(
    job: JobInput,
    cancelled: &AtomicBool,
) -> Result<Published, (RecoverableBundle, RecoveryReason)> {
    let (bundle, sealed, backend, replay) = match job.source {
        JobSource::Finished(SealedBundle::Locked { bundle, seal }) => {
            (bundle, Some(seal), None, false)
        }
        JobSource::Recover {
            bundle: RecoverableBundle::Locked(bundle),
            backend,
        } => (bundle, None, backend, backend.is_none()),
        #[cfg(test)]
        _ => return Err((RecoverableBundle::Fixture, RecoveryReason::Interrupted)),
    };
    run(&bundle, sealed, backend, replay, cancelled)
        .map_err(|reason| (RecoverableBundle::Locked(bundle), reason))
}

fn run(
    bundle: &Bundle,
    sealed: Option<Seal>,
    backend: Option<Backend>,
    replay: bool,
    cancelled: &AtomicBool,
) -> Result<Published, RecoveryReason> {
    check_cancelled(cancelled)?;
    bundle
        .clear_job_directory()
        .map_err(|error| RecoveryReason::DecodeFailed(error.to_string()))?;
    let state = bundle.read_state();
    if replay
        && state.needs_replay()
        && let Some((artifact, projections)) = bundle.replay_publication()
    {
        return Ok(Published {
            artifact,
            projections,
        });
    }
    if matches!(
        state,
        DiskState::Complete {
            audio_retained: false,
            ..
        }
    ) {
        return Err(RecoveryReason::PublishFailed(
            "Meeting audio is no longer retained".into(),
        ));
    }
    let attempts = bundle
        .read_job()
        .ok()
        .flatten()
        .map_or(1, |job| job.attempts.saturating_add(1));
    let decoded = (|| {
        let sealed = match sealed {
            Some(seal) => seal,
            None => match bundle.read_seal().map_err(|err| err.to_string())? {
                Some(seal) => seal,
                None => {
                    let cutoff = bundle
                        .read_intent()
                        .map_err(|err| err.to_string())?
                        .map(|intent| intent.cutoff);
                    seal(bundle, cutoff, Vec::new()).map_err(|err| err.to_string())?
                }
            },
        };
        let settings = Settings::load()?;
        let rendered = transcript::transcribe(
            bundle,
            &sealed,
            backend.unwrap_or(bundle.manifest().backend_pref),
            &settings,
            cancelled,
        )?;
        Ok::<_, String>((sealed.cutoff, rendered))
    })();
    check_cancelled(cancelled)?;
    let (cutoff, rendered) =
        decoded.map_err(|error| record_failure(bundle, attempts, JobError::Decode(error)))?;
    let receipt = bundle
        .publish(&rendered.markdown, rendered.publication)
        .map_err(|error| record_failure(bundle, attempts, JobError::Publish(error.to_string())))?;
    bundle.finish_publication(&receipt, &rendered.markdown, cutoff);
    let (artifact, projections) = bundle.completed_artifact().ok_or_else(|| {
        RecoveryReason::PublishFailed("Could not read the committed meeting transcript".into())
    })?;
    Ok(Published {
        artifact,
        projections,
    })
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), RecoveryReason> {
    if cancelled.load(Ordering::Acquire) {
        Err(RecoveryReason::Interrupted)
    } else {
        Ok(())
    }
}

fn record_failure(bundle: &Bundle, attempts: u64, last_error: JobError) -> RecoveryReason {
    let reason = match &last_error {
        JobError::Decode(error) => RecoveryReason::DecodeFailed(error.clone()),
        JobError::Publish(error) => RecoveryReason::PublishFailed(error.clone()),
    };
    if let Err(error) = bundle.write_job(&Job {
        attempts,
        last_error,
    }) {
        eprintln!(
            "Could not record failed meeting job {}: {error}",
            bundle.manifest().info.id
        );
    }
    reason
}
