mod abi;
mod provider;
use super::pill_state::{Phase, State};
use std::cell::{Cell, RefCell};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use windows::Win32::{
    Foundation::*,
    UI::{Accessibility::*, WindowsAndMessaging::*},
};

// WM_GETOBJECT can re-enter while the app holds its state borrow during a UIA event.
thread_local! { static REGISTERED: RefCell<Option<Registration>> = const { RefCell::new(None) }; }
struct Registration {
    provider: IRawElementProviderSimple,
    surface: Arc<Surface>,
}
pub(super) fn object(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    let provider = REGISTERED.with(|slot| slot.borrow().as_ref().map(|r| r.provider.clone()));
    provider.map(|provider| unsafe { UiaReturnRawElementProvider(hwnd, wparam, lparam, &provider) })
}

pub(super) fn destroyed(hwnd: HWND) {
    let registered = REGISTERED.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.as_ref().is_some_and(|r| r.surface.hwnd() == hwnd) {
            slot.take()
        } else {
            None
        }
    });
    if let Some(registered) = registered {
        registered.surface.alive.store(false, Ordering::Release);
        unsafe {
            let _ = UiaDisconnectProvider(&registered.provider);
            UiaReturnRawElementProvider(
                hwnd,
                WPARAM(0),
                LPARAM(0),
                None::<&IRawElementProviderSimple>,
            );
        }
    }
}

#[derive(Clone, Copy)]
struct Action {
    id: usize,
    label: &'static str,
    x: i32,
    width: i32,
}
const ACTIONS: [Action; 6] = [
    Action {
        id: 101,
        label: "Cancel",
        x: 40,
        width: 60,
    },
    Action {
        id: 102,
        label: "Pause",
        x: 102,
        width: 72,
    },
    Action {
        id: 103,
        label: "History",
        x: 176,
        width: 60,
    },
    Action {
        id: 105,
        label: "Paste",
        x: 190,
        width: 50,
    },
    Action {
        id: 104,
        label: "Copy",
        x: 244,
        width: 51,
    },
    Action {
        id: 106,
        label: "Stop",
        x: 0,
        width: 40,
    },
];
#[derive(Clone, Default, PartialEq)]
struct Snapshot {
    mask: u8,
    paused: bool,
    copied: bool,
    name: String,
}
impl Snapshot {
    fn new(state: &State) -> Self {
        let active = matches!(state.phase, Phase::Recording | Phase::Transcribing);
        let mut mask = 0;
        for (index, action) in ACTIONS.iter().enumerate() {
            let show = match action.id {
                101 => active,
                103 => active && !state.media_result,
                102 => active && matches!(state.phase, Phase::Recording),
                106 => matches!(state.phase, Phase::Recording),
                104 => matches!(state.phase, Phase::Done(_) | Phase::NotPasted { .. }),
                105 => {
                    matches!(state.phase, Phase::NotPasted { .. })
                        || state.repeat_paste && matches!(state.phase, Phase::Done(_))
                }
                _ => false,
            };
            if show {
                mask |= 1 << index;
            }
        }
        Self {
            mask,
            paused: state.paused,
            copied: state.copied,
            name: match &state.phase {
                Phase::Hidden => "Koett".into(),
                Phase::Recording if state.paused => "Koett Paused".into(),
                Phase::Recording => "Koett Recording".into(),
                Phase::Transcribing => "Koett Transcribing".into(),
                Phase::Done(_) if state.copied => "Koett Copied".into(),
                Phase::Done(preview) => format!(
                    "Koett Done. {}",
                    preview.chars().take(80).collect::<String>()
                ),
                Phase::NotPasted { saved } => if *saved {
                    "Koett Not pasted. Saved"
                } else {
                    "Koett Not pasted. Not saved. Copy to keep"
                }
                .into(),
                Phase::Error { message, .. } => format!("Koett. {message}"),
            },
        }
    }
    fn visible(&self, index: usize) -> bool {
        self.mask & (1 << index) != 0
    }
    fn label(&self, index: usize) -> &'static str {
        match ACTIONS[index].id {
            102 if self.paused => "Resume",
            104 if self.copied => "Copied",
            _ => ACTIONS[index].label,
        }
    }
}
struct Surface {
    hwnd: usize,
    alive: AtomicBool,
    snapshot: Mutex<Snapshot>,
}
impl Surface {
    fn hwnd(&self) -> HWND {
        HWND(self.hwnd as *mut _)
    }
    fn snapshot(&self) -> Snapshot {
        self.snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    fn available(&self) -> windows::core::Result<()> {
        if self.alive.load(Ordering::Acquire) && unsafe { IsWindow(Some(self.hwnd())) }.as_bool() {
            Ok(())
        } else {
            Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                UIA_E_ELEMENTNOTAVAILABLE as i32,
            )))
        }
    }
}
pub(super) struct Buttons {
    provider: IRawElementProviderSimple,
    surface: Arc<Surface>,
    key: Cell<Option<super::pill_state::PaintKey>>,
}
impl Buttons {
    pub fn new(parent: HWND) -> Result<Self, String> {
        let surface = Arc::new(Surface {
            hwnd: parent.0 as usize,
            alive: AtomicBool::new(true),
            snapshot: Mutex::new(Snapshot::default()),
        });
        let provider = provider::Node {
            surface: surface.clone(),
            index: None,
            root: None,
        }
        .into_simple()
        .map_err(|e| e.to_string())?;
        REGISTERED.with(|slot| {
            *slot.borrow_mut() = Some(Registration {
                provider: provider.clone(),
                surface: surface.clone(),
            })
        });
        Ok(Self {
            provider,
            surface,
            key: Cell::new(None),
        })
    }
    pub fn sync(&self, _parent: HWND, state: &State) {
        let key = state.paint_key();
        if self.key.get() == Some(key) {
            return;
        }
        self.key.set(Some(key));
        let next = Snapshot::new(state);
        let (changed, announce) = {
            let mut current = self
                .surface
                .snapshot
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let changed = *current != next;
            let announce = current.name != next.name && !matches!(state.phase, Phase::Hidden);
            *current = next;
            (changed, announce)
        };
        if changed {
            unsafe {
                let _ = UiaRaiseStructureChangedEvent(
                    &self.provider,
                    StructureChangeType_ChildrenInvalidated,
                    std::ptr::null_mut(),
                    0,
                );
            }
        }
        if announce {
            unsafe {
                let _ = UiaRaiseAutomationEvent(&self.provider, UIA_LiveRegionChangedEventId);
            }
        }
    }
    pub fn allows(&self, id: usize) -> bool {
        let snapshot = self.surface.snapshot();
        ACTIONS
            .iter()
            .enumerate()
            .any(|(i, action)| action.id == id && snapshot.visible(i))
    }
}
impl Drop for Buttons {
    fn drop(&mut self) {
        self.surface.alive.store(false, Ordering::Release);
        destroyed(self.surface.hwnd());
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn media_copy_does_not_offer_a_previous_dictation_paste() {
        let mut state = State::default();
        state.set(Phase::Done("Media words".into()), 0);
        state.media_result = true;
        assert_eq!(Snapshot::new(&state).mask, 0b10000);
        state.set(Phase::Transcribing, 1);
        state.media_result = true;
        assert_eq!(Snapshot::new(&state).mask, 0b00001);
        state.set(Phase::Done("Drawing words".into()), 2);
        state.repeat_paste = true;
        assert_eq!(Snapshot::new(&state).mask, 0b11000);
    }
    #[test]
    fn actions_follow_visible_pill_states_and_labels() {
        let mut state = State::default();
        assert_eq!(Snapshot::new(&state).mask, 0);
        state.set(Phase::Recording, 0);
        assert!(!state.hovered);
        assert_eq!(Snapshot::new(&state).mask, 0b100111);
        state.hover(true, 1);
        assert_eq!(Snapshot::new(&state).mask, 0b100111);
        state.hover(false, 2);
        assert_eq!(Snapshot::new(&state).mask, 0b100111);
        state.paused = true;
        assert_eq!(Snapshot::new(&state).label(1), "Resume");
        state.set(Phase::Transcribing, 2);
        assert_eq!(Snapshot::new(&state).mask, 0b00101);
        state.set(Phase::Done("test".into()), 3);
        assert_eq!(Snapshot::new(&state).mask, 0b10000);
        state.copy(4);
        assert_eq!(Snapshot::new(&state).label(4), "Copied");
        state.set(Phase::NotPasted { saved: true }, 5);
        assert_eq!(Snapshot::new(&state).mask, 0b11000);
        state.set(Phase::NotPasted { saved: false }, 6);
        assert_eq!(Snapshot::new(&state).mask, 0b11000);
        assert!(Snapshot::new(&state).name.contains("Not saved"));
    }
}
