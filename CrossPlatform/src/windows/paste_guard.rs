use std::time::{Duration, Instant};

pub(super) fn paste_when_ready<T: Copy + Eq>(
    expected: T,
    mut modifiers_down: impl FnMut() -> bool,
    validate: impl FnOnce() -> Result<(), String>,
    mut foreground: impl FnMut() -> Option<T>,
    send: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_millis(500);
    while modifiers_down() {
        if Instant::now() >= deadline {
            return Err("Release the shortcut keys; text remains on the clipboard.".to_string());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    validate()?;
    if foreground() != Some(expected) {
        return Err("Focus changed. Transcript saved; text remains on the clipboard.".to_string());
    }
    send()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn clipboard_validation_cannot_redirect_paste_or_allow_invalid_data() {
        for invalid in [false, true] {
            let target = Cell::new(1);
            let sent = Cell::new(false);
            let result = paste_when_ready(
                1,
                || false,
                || {
                    target.set(2);
                    if invalid {
                        Err("Clipboard changed".into())
                    } else {
                        Ok(())
                    }
                },
                || Some(target.get()),
                || {
                    sent.set(true);
                    Ok(())
                },
            );
            assert!(result.is_err());
            assert!(!sent.get());
            if invalid {
                assert_eq!(result.unwrap_err(), "Clipboard changed");
            }
        }
    }

    #[test]
    fn reused_window_handle_with_different_pid_never_sends_input() {
        let sent = Cell::new(false);
        assert!(
            paste_when_ready(
                (10, 20),
                || false,
                || Ok(()),
                || Some((10, 21)),
                || {
                    sent.set(true);
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(!sent.get());
    }

    #[test]
    fn switching_during_modifier_wait_never_sends_input() {
        let target = Cell::new(1);
        let waited = Cell::new(false);
        let sent = Cell::new(false);
        let result = paste_when_ready(
            1,
            || {
                if waited.replace(true) {
                    false
                } else {
                    target.set(2);
                    true
                }
            },
            || Ok(()),
            || Some(target.get()),
            || {
                sent.set(true);
                Ok(())
            },
        );
        assert!(result.is_err());
        assert!(waited.get());
        assert!(!sent.get());
    }

    #[test]
    fn changed_or_ineligible_destination_never_sends_input() {
        for current in [None, Some(2)] {
            let sent = Cell::new(false);
            assert!(
                paste_when_ready(
                    1,
                    || false,
                    || Ok(()),
                    || current,
                    || {
                        sent.set(true);
                        Ok(())
                    }
                )
                .is_err()
            );
            assert!(!sent.get());
        }
    }

    #[test]
    fn unchanged_target_sends_once_and_propagates_delivery_errors() {
        let sent = Cell::new(0);
        assert!(
            paste_when_ready(
                1,
                || false,
                || Ok(()),
                || Some(1),
                || {
                    sent.set(sent.get() + 1);
                    Ok(())
                }
            )
            .is_ok()
        );
        assert_eq!(sent.get(), 1);
        assert_eq!(
            paste_when_ready(1, || false, || Ok(()), || Some(1), || Err("blocked".into())),
            Err("blocked".into())
        );
    }
}
