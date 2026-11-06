//! Restarts Koett after an update and proves the new version came up. This runs from
//! the version being replaced, which stays on disk, so a release that dies at startup
//! can't take its own recovery with it.
use super::{go_back, launch, load_state, save_state};
use std::path::Path;
use std::time::{Duration, Instant};

/// Long enough for a model to load from a slow disk.
const COME_UP_LIMIT: Duration = Duration::from_secs(180);
/// A process that dies right after saying it's ready didn't come up.
const STAY_UP: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(200);
const OLD_PROCESS_LIMIT: Duration = Duration::from_secs(60);

/// `kill -0` calls a zombie alive, and a parent that doesn't reap leaves one.
fn process_gone(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/status")).map_or(true, |status| {
        status
            .lines()
            .find_map(|line| line.strip_prefix("State:"))
            .is_some_and(|state| state.trim_start().starts_with('Z'))
    })
}

/// True once `healthy` has held for `stay_up` with the process alive.
fn came_up(
    mut alive: impl FnMut() -> bool,
    mut healthy: impl FnMut() -> bool,
    limit: Duration,
    stay_up: Duration,
    poll: Duration,
) -> bool {
    let started = Instant::now();
    let mut since = None::<Instant>;
    while started.elapsed() < limit {
        if !alive() {
            // Someone else may have started Koett first, in which case this one left.
            return healthy();
        }
        if since.is_some_and(|since: Instant| since.elapsed() >= stay_up) {
            return true;
        }
        if since.is_none() && healthy() {
            since = Some(Instant::now());
        }
        std::thread::sleep(poll);
    }
    false
}

fn reports_started(build: u64) -> bool {
    super::super::control::request("update-status")
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .is_some_and(|status| status["started"] == true && status["build"] == build)
}

/// `koett update-watch PID PREFIX BUILD FROM MODE`. Build 0 only restarts, for a rollback.
pub fn run(arguments: &[String]) -> Result<(), String> {
    let [pid, prefix, build, from, mode] = arguments else {
        return Err("usage: koett update-watch PID PREFIX BUILD FROM MODE".into());
    };
    let pid: u32 = pid.parse().map_err(|_| "Bad process id".to_string())?;
    let build: u64 = build.parse().map_err(|_| "Bad build".to_string())?;
    let prefix = Path::new(prefix);
    let waited = Instant::now();
    while !process_gone(pid) {
        if waited.elapsed() > OLD_PROCESS_LIMIT {
            return Err("Koett did not quit".into());
        }
        std::thread::sleep(POLL);
    }
    let mut child = launch(prefix, mode);
    if build == 0 {
        return child.map(drop);
    }
    let confirmed = child.as_mut().is_ok_and(|child| {
        came_up(
            || matches!(child.try_wait(), Ok(None)),
            || reports_started(build),
            COME_UP_LIMIT,
            STAY_UP,
            POLL,
        )
    });
    if confirmed {
        return Ok(());
    }
    if let Ok(child) = &mut child {
        let _ = child.kill();
        let _ = child.wait();
    }
    let mut state = load_state();
    state.failed = Some(format!(
        "Koett went back to the previous version because build {build} didn't start"
    ));
    let _ = save_state(&state);
    go_back(prefix, from, build)?;
    launch(prefix, mode).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    const LIMIT: Duration = Duration::from_millis(200);
    const STAY: Duration = Duration::from_millis(30);
    const POLL: Duration = Duration::from_millis(5);

    #[test]
    fn a_process_that_stays_up_healthy_counts() {
        assert!(came_up(|| true, || true, LIMIT, STAY, POLL));
    }

    #[test]
    fn a_crash_counts_as_not_up() {
        assert!(!came_up(|| false, || false, LIMIT, STAY, POLL));
    }

    #[test]
    fn a_hang_runs_out_of_time() {
        assert!(!came_up(|| true, || false, LIMIT, STAY, POLL));
    }

    #[test]
    fn dying_right_after_saying_ready_counts_as_not_up() {
        let asks = Cell::new(0);
        let alive = || {
            asks.set(asks.get() + 1);
            asks.get() < 3
        };
        // Once it's gone, nothing answers.
        let healthy = || asks.get() < 3;
        assert!(!came_up(alive, healthy, LIMIT, STAY, POLL));
    }

    #[test]
    fn another_healthy_instance_is_fine() {
        assert!(came_up(|| false, || true, LIMIT, STAY, POLL));
    }

    #[test]
    fn this_process_is_not_gone_and_a_missing_one_is() {
        assert!(!process_gone(std::process::id()));
        assert!(process_gone(u32::MAX));
    }
}
