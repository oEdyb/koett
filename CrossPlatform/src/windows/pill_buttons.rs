mod abi;
mod provider;
use super::pill_state::{Phase, State};
use std::cell::RefCell;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use windows::Win32::{
    Foundation::*,
    UI::{Accessibility::*, WindowsAndMessaging::*},
};

// WM_GETOBJECT can re-enter while the app holds its state borrow during a UIA event.
thread_local! { static REGISTERED: RefCell<Option<IRawElementProviderSimple>> = const { RefCell::new(None) }; }
pub(super) fn object(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    REGISTERED.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|provider| unsafe { UiaReturnRawElementProvider(hwnd, wparam, lparam, provider) })
    })
}

#[derive(Clone, Copy)]
struct Action {
    id: usize,
    label: &'static str,
    x: i32,
    width: i32,
}
const ACTIONS: [Action; 5] = [
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
];
#[derive(Clone, Copy, Default, PartialEq)]
struct Snapshot {
    mask: u8,
    paused: bool,
    copied: bool,
}
impl Snapshot {
    fn new(state: &State) -> Self {
        let active = matches!(state.phase, Phase::Recording | Phase::Transcribing) && state.hovered;
        let mut mask = 0;
        for (index, action) in ACTIONS.iter().enumerate() {
            let show = match action.id {
                101 | 103 => active,
                102 => active && matches!(state.phase, Phase::Recording),
                104 => matches!(state.phase, Phase::Done(_) | Phase::NotPasted),
                105 => matches!(state.phase, Phase::NotPasted),
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
        }
    }
    fn visible(self, index: usize) -> bool {
        self.mask & (1 << index) != 0
    }
    fn label(self, index: usize) -> &'static str {
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
        *self.snapshot.lock().unwrap_or_else(|e| e.into_inner())
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
        REGISTERED.with(|slot| *slot.borrow_mut() = Some(provider.clone()));
        Ok(Self { provider, surface })
    }
    pub fn sync(&self, _parent: HWND, state: &State) {
        let next = Snapshot::new(state);
        let changed = {
            let mut current = self
                .surface
                .snapshot
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let changed = *current != next;
            *current = next;
            changed
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
        REGISTERED.with(|slot| {
            slot.borrow_mut().take();
        });
        unsafe {
            let _ = UiaDisconnectProvider(&self.provider);
            UiaReturnRawElementProvider(
                self.surface.hwnd(),
                WPARAM(0),
                LPARAM(0),
                None::<&IRawElementProviderSimple>,
            );
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actions_follow_visible_pill_states_and_labels() {
        let mut state = State::default();
        assert_eq!(Snapshot::new(&state).mask, 0);
        state.set(Phase::Recording, 0);
        state.hover(true, 1);
        assert_eq!(Snapshot::new(&state).mask, 0b00111);
        state.paused = true;
        assert_eq!(Snapshot::new(&state).label(1), "Resume");
        state.set(Phase::Transcribing, 2);
        assert_eq!(Snapshot::new(&state).mask, 0b00101);
        state.set(Phase::Done("test".into()), 3);
        assert_eq!(Snapshot::new(&state).mask, 0b10000);
        state.copy(4);
        assert_eq!(Snapshot::new(&state).label(4), "Copied");
        state.set(Phase::NotPasted, 5);
        assert_eq!(Snapshot::new(&state).mask, 0b11000);
    }
}
