//! Which microphones Koett offers on Linux and how it picks one that works.
//!
//! With a sound server, inputs are its sources, opened through it, so the mic is
//! shared with Discord, OBS and anything else. Raw ALSA can't share: a device the
//! server holds answers "busy". Without a server, each sound card is listed once.
use super::InputDevice;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

const PULSE_PREFIX: &str = "pulseaudio:";
const OPEN_TIMEOUT: Duration = Duration::from_millis(2500);
/// All attempts together, so a press can't hold the engine for long.
const START_BUDGET: Duration = Duration::from_secs(6);
/// How long a source found silent is tried last. Long enough that recordings don't
/// each lose their first seconds, short enough that a fixed mic comes back by itself.
const SILENT_MEMORY: Duration = Duration::from_secs(600);

/// The source last found silent, by id, with `None` meaning the system default.
static SILENT: Mutex<Option<(Option<String>, Instant)>> = Mutex::new(None);

pub(super) fn remember_silent(id: Option<&str>) {
    if let Ok(mut silent) = SILENT.lock() {
        *silent = Some((id.map(str::to_string), Instant::now()));
    }
}

fn silent_source() -> Option<Option<String>> {
    let silent = SILENT.lock().ok()?;
    silent
        .as_ref()
        .filter(|(_, at)| at.elapsed() < SILENT_MEMORY)
        .map(|(id, _)| id.clone())
}

/// The id the system default resolves to right now.
fn default_id() -> Option<String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    bounded(OPEN_TIMEOUT, || {
        cpal::default_host()
            .default_input_device()?
            .id()
            .ok()
            .map(|id| id.to_string())
    })
    .flatten()
}

/// Everything that means the silent source. The system default can resolve to it,
/// as it did on Olle's PC, so naming the source alone would reopen it through "default".
fn silent_names(silent: Option<Option<String>>, default: Option<String>) -> Vec<Option<String>> {
    let Some(silent) = silent else {
        return Vec::new();
    };
    let mut names = vec![silent.clone()];
    if default.is_some() {
        if silent == default {
            names.push(None);
        } else if silent.is_none() {
            names.push(default);
        }
    }
    names
}

/// Monitors record what the speakers play, so they are never a microphone.
fn is_real_source(id: &str) -> bool {
    id.strip_prefix(PULSE_PREFIX)
        .is_some_and(|name| !name.ends_with(".monitor"))
}

/// The card an ALSA id names, for ids that open one card's capture path.
fn alsa_card(id: &str) -> Option<&str> {
    let rest = id.strip_prefix("alsa:")?;
    let (kind, args) = rest.split_once(':')?;
    if !matches!(kind, "sysdefault" | "front" | "hw" | "plughw") {
        return None;
    }
    let card = args.strip_prefix("CARD=")?;
    Some(card.split(',').next().unwrap_or(card))
}

/// No sound server: one `sysdefault` or `front` entry per card. Plugins such as
/// `lavrate` or `jack`, the discard-all `null` and playback-only `usbstream` are
/// not microphones, and `hw:`/`plughw:` repeat a card under its changeable number.
fn alsa_microphones(all: Vec<InputDevice>) -> Vec<InputDevice> {
    let named = |id: &str, kind: &str| id.starts_with(&format!("alsa:{kind}:CARD="));
    let mut kept: Vec<InputDevice> = Vec::new();
    for kind in ["sysdefault", "front"] {
        for device in all.iter().filter(|device| named(&device.id, kind)) {
            let card = alsa_card(&device.id);
            if !kept.iter().any(|other| alsa_card(&other.id) == card) {
                kept.push(InputDevice {
                    id: device.id.clone(),
                    label: device
                        .label
                        .split(['\n', ','])
                        .next()
                        .unwrap_or(&device.label)
                        .trim()
                        .to_string(),
                });
            }
        }
    }
    kept
}

/// Microphones for this machine, real ones only.
pub(super) fn microphones(all: Vec<InputDevice>) -> Vec<InputDevice> {
    if all.iter().any(|device| device.id.starts_with(PULSE_PREFIX)) {
        let mut seen = std::collections::HashSet::new();
        all.into_iter()
            .filter(|device| is_real_source(&device.id) && seen.insert(device.id.clone()))
            .collect()
    } else {
        alsa_microphones(all)
    }
}

/// The long name of an ALSA card from `/proc/asound/cards`, by id or number.
fn card_long_name(cards: &str, card: &str) -> Option<String> {
    cards.lines().find_map(|line| {
        let (number, rest) = line.trim_start().split_once(' ')?;
        let rest = rest.trim_start().strip_prefix('[')?;
        let (id, rest) = rest.split_once(']')?;
        if card != number && card != id.trim() {
            return None;
        }
        let (_, name) = rest.split_once(" - ")?;
        Some(name.trim().to_string())
    })
}

/// The source for a choice saved as a raw ALSA id, matched by card name. A source
/// is a mic on that card only if it names it, so an unknown card stays unmatched.
fn migrated_source(saved: &str, cards: &str, devices: &[InputDevice]) -> Option<String> {
    let name = card_long_name(cards, alsa_card(saved)?)?;
    devices
        .iter()
        .filter(|device| device.id.starts_with(PULSE_PREFIX))
        .find(|device| device.label.starts_with(&name))
        .map(|device| device.id.clone())
}

/// The new id when `saved` is an ALSA id that a sound server source now covers.
pub fn migrated_input(saved: Option<&str>) -> Option<String> {
    let saved = saved?;
    if !saved.starts_with("alsa:") {
        return None;
    }
    let saved = saved.to_string();
    bounded(OPEN_TIMEOUT, move || {
        let cards = std::fs::read_to_string("/proc/asound/cards").ok()?;
        migrated_source(&saved, &cards, &super::input_devices().ok()?)
    })
    .flatten()
}

/// What to try, in order: the user's choice, the system default, then the rest.
/// `None` is the system default. A source recently found silent goes last.
fn candidates(
    selected: Option<&str>,
    devices: &[InputDevice],
    silent: &[Option<String>],
) -> Vec<Option<String>> {
    let mut order = Vec::new();
    if let Some(selected) = selected.filter(|id| devices.iter().any(|device| device.id == *id)) {
        order.push(Some(selected.to_string()));
    }
    order.push(None);
    for device in devices {
        if selected != Some(device.id.as_str()) {
            order.push(Some(device.id.clone()));
        }
    }
    for name in silent {
        if let Some(index) = order.iter().position(|candidate| candidate == name) {
            let last = order.remove(index);
            order.push(last);
        }
    }
    order
}

fn label(id: Option<&str>, devices: &[InputDevice]) -> String {
    id.and_then(|id| devices.iter().find(|device| device.id == id))
        .map_or_else(
            || "system default".to_string(),
            |device| device.label.clone(),
        )
}

/// What the pill says when capture is not on the chosen mic.
fn switched_notice(used: Option<&str>, devices: &[InputDevice]) -> String {
    format!("Microphone unavailable · using {}", label(used, devices))
}

pub(super) fn silent_notice(used: Option<&str>, devices: &[InputDevice]) -> String {
    format!("This mic is silent · using {}", label(used, devices))
}

/// Runs `work` on its own thread and stops waiting after `limit`. A stuck audio
/// call then costs one parked thread and never the engine, which is what keeps
/// every later press working.
pub(super) fn bounded<T: Send + 'static>(
    limit: Duration,
    work: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (send, receive) = std::sync::mpsc::channel();
    thread::Builder::new()
        .name("koett-audio-open".into())
        .spawn(move || {
            let _ = send.send(work());
        })
        .ok()?;
    receive.recv_timeout(limit).ok()
}

/// Tries each candidate through `open` until one starts, within the overall budget.
/// Returns what opened, which candidate, and a notice if it wasn't the chosen mic.
fn first_working<T: Send + 'static>(
    selected: Option<&str>,
    devices: &[InputDevice],
    silent: &[Option<String>],
    limit: Duration,
    budget: Duration,
    open: impl Fn(Option<String>) -> Result<T, String> + Clone + Send + 'static,
) -> Result<(T, Option<String>, Option<String>), String> {
    let started = Instant::now();
    let mut first_error = None;
    let chosen = selected.map(str::to_string);
    let chosen_is_silent = silent.contains(&chosen);
    for candidate in candidates(selected, devices, silent) {
        let left = budget.saturating_sub(started.elapsed());
        if left.is_zero() {
            break;
        }
        let attempt = {
            let open = open.clone();
            let candidate = candidate.clone();
            bounded(limit.min(left), move || open(candidate))
        };
        match attempt {
            Some(Ok(opened)) => {
                let notice = (candidate != chosen).then(|| {
                    if chosen_is_silent {
                        silent_notice(candidate.as_deref(), devices)
                    } else {
                        switched_notice(candidate.as_deref(), devices)
                    }
                });
                return Ok((opened, candidate, notice));
            }
            Some(Err(error)) => {
                first_error.get_or_insert(error);
            }
            None => {
                first_error.get_or_insert("The microphone did not respond".to_string());
            }
        }
    }
    Err(format!(
        "No working microphone: {}",
        first_error.unwrap_or_else(|| "none found".into())
    ))
}

/// Starts capture on the first microphone that opens, in bounded time.
pub(super) fn start_first_working<T: Send + 'static>(
    selected: Option<&str>,
    open: impl Fn(Option<String>) -> Result<T, String> + Clone + Send + 'static,
) -> Result<(T, Option<String>, Option<String>), String> {
    let started = Instant::now();
    let devices = bounded(OPEN_TIMEOUT, super::input_devices)
        .and_then(Result::ok)
        .unwrap_or_default();
    first_working(
        selected,
        &devices,
        &silent_names(silent_source(), default_id()),
        OPEN_TIMEOUT,
        START_BUDGET.saturating_sub(started.elapsed()),
        open,
    )
}

/// Candidates for replacing a mic that failed during a recording.
pub(super) fn replacements(current: Option<&str>) -> (Vec<Option<String>>, Vec<InputDevice>) {
    let devices = bounded(OPEN_TIMEOUT, super::input_devices)
        .and_then(Result::ok)
        .unwrap_or_default();
    let default = default_id();
    let on_default = current.is_some() && default.as_deref() == current;
    let order = candidates(None, &devices, &silent_names(silent_source(), default))
        .into_iter()
        // The system default may be the very source being replaced.
        .filter(|candidate| candidate.as_deref() != current && !(on_default && candidate.is_none()))
        .collect();
    (order, devices)
}

pub(super) fn lost_notice(used: Option<&str>, devices: &[InputDevice]) -> String {
    format!("Microphone lost · using {}", label(used, devices))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, label: &str) -> InputDevice {
        InputDevice {
            id: id.into(),
            label: label.into(),
        }
    }

    fn ids(devices: &[InputDevice]) -> Vec<&str> {
        devices.iter().map(|device| device.id.as_str()).collect()
    }

    const CARDS: &str = " 0 [NVidia         ]: HDA-Intel - HDA NVidia
                      HDA NVidia at 0xf6080000 irq 90
 2 [Generic_1      ]: HDA-Intel - HD-Audio Generic
                      HD-Audio Generic at 0xf7500000 irq 92
 3 [O2             ]: USB-Audio - OBSBOT Meet 2
                      Remo Tech Co., Ltd. OBSBOT Meet 2 at usb-0000:0d:00.0-1, super
 4 [USB            ]: USB-Audio - RØDE PodMic USB
                      RØDE Microphones RØDE PodMic USB at usb-0000:0d:00.0-2";

    #[test]
    fn with_a_server_only_real_sources_are_listed() {
        // Olle's machine: monitors of every output, a virtual source and the mic.
        let list = microphones(vec![
            device(
                "pulseaudio:easyeffects_sink.monitor",
                "Monitor of Easy Effects Sink",
            ),
            device("pulseaudio:easyeffects_source", "Easy Effects Source"),
            device(
                "pulseaudio:alsa_output.usb-Audeze.monitor",
                "Monitor of Audeze",
            ),
            device(
                "pulseaudio:alsa_input.usb-PodMic.mono-fallback",
                "RØDE PodMic USB Mono",
            ),
        ]);
        assert_eq!(
            ids(&list),
            [
                "pulseaudio:easyeffects_source",
                "pulseaudio:alsa_input.usb-PodMic.mono-fallback"
            ]
        );
    }

    #[test]
    fn without_a_server_each_card_appears_once_and_plugins_are_gone() {
        // The entries from Olle's dropdown that a filter has to get right.
        let all = [
            "null",
            "lavrate",
            "samplerate",
            "speexrate",
            "jack",
            "oss",
            "pipewire",
            "pulse",
            "speex",
            "upmix",
            "vdownmix",
            "default",
            "usbstream:CARD=NVidia",
            "usbstream:CARD=USB",
            "hw:CARD=4,DEV=0",
            "plughw:CARD=4,DEV=0",
            "hw:CARD=2,DEV=2",
            "front:CARD=USB,DEV=0",
            "sysdefault:CARD=USB",
            "front:CARD=O2,DEV=0",
            "sysdefault:CARD=O2",
            "front:CARD=OnlyFront,DEV=0",
        ]
        .map(|id| device(&format!("alsa:{id}"), "RØDE PodMic USB, USB Audio\nDefault"));
        let list = microphones(all.to_vec());
        assert_eq!(
            ids(&list),
            [
                "alsa:sysdefault:CARD=USB",
                "alsa:sysdefault:CARD=O2",
                "alsa:front:CARD=OnlyFront,DEV=0"
            ]
        );
        assert_eq!(list[0].label, "RØDE PodMic USB", "no ALSA suffix");
    }

    #[test]
    fn a_saved_alsa_mic_moves_to_its_source_by_number_or_name() {
        let devices = [
            device("pulseaudio:easyeffects_source", "Easy Effects Source"),
            device("pulseaudio:alsa_input.usb-PodMic", "RØDE PodMic USB Mono"),
        ];
        for saved in [
            "alsa:hw:CARD=4,DEV=0",
            "alsa:sysdefault:CARD=USB",
            "alsa:plughw:CARD=4,DEV=0",
        ] {
            assert_eq!(
                migrated_source(saved, CARDS, &devices).as_deref(),
                Some("pulseaudio:alsa_input.usb-PodMic"),
                "{saved}"
            );
        }
        // No source for that card: stay unmatched instead of guessing another mic.
        assert_eq!(
            migrated_source("alsa:hw:CARD=3,DEV=0", CARDS, &devices),
            None
        );
        assert_eq!(
            migrated_source("alsa:hw:CARD=9,DEV=0", CARDS, &devices),
            None
        );
        assert_eq!(migrated_source("alsa:jack", CARDS, &devices), None);
    }

    #[test]
    fn a_choice_that_is_present_comes_first_and_a_missing_one_is_skipped() {
        let devices = [device("a", "A"), device("b", "B")];
        let some = |id: &str| Some(id.to_string());
        assert_eq!(
            candidates(Some("b"), &devices, &[]),
            [some("b"), None, some("a")]
        );
        assert_eq!(
            candidates(Some("gone"), &devices, &[]),
            [None, some("a"), some("b")]
        );
        assert_eq!(
            candidates(None, &devices, &[]),
            [None, some("a"), some("b")]
        );
    }

    #[test]
    fn a_source_found_silent_is_tried_last_but_not_forgotten() {
        let devices = [device("a", "A"), device("b", "B")];
        let some = |id: &str| Some(id.to_string());
        assert_eq!(
            candidates(Some("b"), &devices, &[some("b")]),
            [None, some("a"), some("b")]
        );
        // The silent one can be the system default, as on Olle's PC.
        assert_eq!(
            candidates(None, &devices, &[None]),
            [some("a"), some("b"), None]
        );
        assert_eq!(
            candidates(Some("a"), &devices, &[some("b")]),
            [some("a"), None, some("b")]
        );
    }

    fn start(
        selected: Option<&str>,
        working: &'static [&'static str],
    ) -> Result<(String, Option<String>), String> {
        let devices = [device("a", "Mic A"), device("b", "Mic B")];
        first_working(
            selected,
            &devices,
            &[],
            Duration::from_millis(100),
            Duration::from_millis(400),
            move |candidate| {
                let name = candidate.unwrap_or_else(|| "default".into());
                if working.contains(&name.as_str()) {
                    Ok(name)
                } else if name == "hangs" {
                    thread::sleep(Duration::from_secs(30));
                    Ok(name)
                } else {
                    Err(format!("{name} is busy"))
                }
            },
        )
        .map(|(opened, _, notice)| (opened, notice))
    }

    #[test]
    fn a_working_choice_is_kept_and_says_nothing() {
        assert_eq!(start(Some("b"), &["b", "default"]), Ok(("b".into(), None)));
        assert_eq!(start(None, &["default", "a"]), Ok(("default".into(), None)));
    }

    #[test]
    fn a_failed_choice_falls_back_and_says_where() {
        assert_eq!(
            start(Some("b"), &["default"]),
            Ok((
                "default".into(),
                Some("Microphone unavailable · using system default".into())
            ))
        );
        assert_eq!(
            start(None, &["a"]),
            Ok((
                "a".into(),
                Some("Microphone unavailable · using Mic A".into())
            ))
        );
    }

    #[test]
    fn the_system_default_counts_as_the_silent_source_it_resolves_to() {
        // Olle's case: the default was the silent source, so "default" must go last too.
        let some = |id: &str| Some(id.to_string());
        let silent = silent_names(Some(some("easy")), some("easy"));
        assert_eq!(silent, [some("easy"), None]);
        let devices = [device("easy", "Easy"), device("mic", "Mic")];
        assert_eq!(
            candidates(Some("easy"), &devices, &silent),
            [some("mic"), some("easy"), None]
        );
        // The default itself found silent also names what it resolved to.
        assert_eq!(silent_names(Some(None), some("easy")), [None, some("easy")]);
        // A different default is unaffected.
        assert_eq!(
            silent_names(Some(some("easy")), some("mic")),
            [some("easy")]
        );
        assert!(silent_names(None, some("mic")).is_empty());
    }

    #[test]
    fn a_chosen_mic_known_to_be_silent_says_so() {
        let devices = [device("a", "Mic A"), device("b", "Mic B")];
        let silent = [Some("b".to_string())];
        let (opened, _, notice) = first_working(
            Some("b"),
            &devices,
            &silent,
            Duration::from_millis(100),
            Duration::from_millis(400),
            |candidate| Ok::<_, String>(candidate.unwrap_or_else(|| "default".into())),
        )
        .unwrap();
        assert_eq!(opened, "default");
        assert_eq!(
            notice.as_deref(),
            Some("This mic is silent · using system default")
        );
    }

    #[test]
    fn nothing_working_is_a_clear_error() {
        let error = start(Some("b"), &[]).unwrap_err();
        assert_eq!(error, "No working microphone: b is busy");
    }

    #[test]
    fn a_stuck_open_costs_only_its_time_limit() {
        let started = Instant::now();
        assert_eq!(
            bounded(Duration::from_millis(50), || {
                thread::sleep(Duration::from_secs(30));
            }),
            None
        );
        assert!(started.elapsed() < Duration::from_secs(5));
        // The whole start stays inside its budget with every attempt stuck.
        let devices = [device("hangs", "Hangs"), device("hangs2", "Hangs too")];
        let started = Instant::now();
        let result: Result<(u8, _, _), _> = first_working(
            Some("hangs"),
            &devices,
            &[],
            Duration::from_millis(100),
            Duration::from_millis(250),
            |_| {
                thread::sleep(Duration::from_secs(30));
                Ok(0)
            },
        );
        assert_eq!(
            result.unwrap_err(),
            "No working microphone: The microphone did not respond"
        );
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
