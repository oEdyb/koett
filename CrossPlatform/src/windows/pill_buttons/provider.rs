use super::{ACTIONS, Surface, abi::*};
use std::{ffi::c_void, ptr, sync::Arc};
use windows::{
    Win32::{
        Foundation::*,
        System::{Com::SAFEARRAY, Ole::*, Variant::*},
        UI::{Accessibility::*, HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
    },
    core::{HRESULT, IUnknown, IUnknownImpl, Interface, Result, implement},
};

#[implement(Simple, Fragment, Root, IInvokeProvider)]
pub(super) struct Node {
    pub surface: Arc<Surface>,
    pub index: Option<usize>,
    pub root: Option<IRawElementProviderSimple>,
}
impl Node {
    pub fn into_simple(self) -> Result<IRawElementProviderSimple> {
        let provider: Simple = self.into();
        provider.cast()
    }
}
impl Node_Impl {
    fn root_provider(&self) -> Result<IRawElementProviderSimple> {
        match &self.root {
            Some(root) => Ok(root.clone()),
            None => self.to_interface::<Simple>().cast(),
        }
    }
    fn child(&self, index: usize) -> Result<IUnknown> {
        Node {
            surface: self.surface.clone(),
            index: Some(index),
            root: Some(self.root_provider()?),
        }
        .into_simple()?
        .cast::<IRawElementProviderFragment>()?
        .cast()
    }
    fn rectangle(&self) -> Result<UiaRect> {
        self.surface.available()?;
        let hwnd = self.surface.hwnd();
        let mut bounds = RECT::default();
        unsafe {
            GetWindowRect(hwnd, &mut bounds)?;
        }
        let scale = unsafe { GetDpiForWindow(hwnd) }.max(96) as f64 / 96.0;
        Ok(match self.index {
            Some(index) => {
                let action = ACTIONS[index];
                UiaRect {
                    left: bounds.left as f64 + action.x as f64 * scale,
                    top: bounds.top as f64 + 8.0 * scale,
                    width: action.width as f64 * scale,
                    height: 32.0 * scale,
                }
            }
            None => UiaRect {
                left: bounds.left as f64,
                top: bounds.top as f64,
                width: (bounds.right - bounds.left) as f64,
                height: (bounds.bottom - bounds.top) as f64,
            },
        })
    }
    fn neighbor(&self, direction: NavigateDirection) -> Result<Option<IUnknown>> {
        self.surface.available()?;
        let snapshot = self.surface.snapshot();
        let visible: Vec<_> = (0..ACTIONS.len())
            .filter(|i| snapshot.visible(*i))
            .collect();
        let index = match (self.index, direction) {
            (None, d) if d == NavigateDirection_FirstChild => visible.first().copied(),
            (None, d) if d == NavigateDirection_LastChild => visible.last().copied(),
            (Some(_), d) if d == NavigateDirection_Parent => {
                return self
                    .root_provider()?
                    .cast::<IRawElementProviderFragment>()?
                    .cast()
                    .map(Some);
            }
            (Some(index), d) if d == NavigateDirection_NextSibling => {
                visible.into_iter().find(|i| *i > index)
            }
            (Some(index), d) if d == NavigateDirection_PreviousSibling => {
                visible.into_iter().rev().find(|i| *i < index)
            }
            _ => None,
        };
        index.map(|i| self.child(i)).transpose()
    }
}
impl Simple_Impl for Node_Impl {
    unsafe fn options(&self, out: *mut ProviderOptions) -> HRESULT {
        // UIA focus is virtual: never activate the HWND hosting these actions.
        unsafe {
            output(
                out,
                Ok(ProviderOptions_ServerSideProvider
                    | ProviderOptions_ProviderOwnsSetFocus
                    | ProviderOptions_UseComThreading),
            )
        }
    }
    unsafe fn pattern(&self, id: UIA_PATTERN_ID, out: *mut *mut c_void) -> HRESULT {
        let result = if self.index.is_some() && id == UIA_InvokePatternId {
            self.to_interface::<IInvokeProvider>().cast().map(Some)
        } else {
            Ok(None)
        };
        unsafe { interface_output(out, result) }
    }
    unsafe fn property(&self, id: UIA_PROPERTY_ID, out: *mut VARIANT) -> HRESULT {
        let result = (|| {
            self.surface.available()?;
            let snapshot = self.surface.snapshot();
            Ok(match id {
                p if p == UIA_NamePropertyId => {
                    VARIANT::from(self.index.map(|i| snapshot.label(i)).unwrap_or(
                        if snapshot.name.is_empty() {
                            "Koett"
                        } else {
                            &snapshot.name
                        },
                    ))
                }
                p if p == UIA_AutomationIdPropertyId => VARIANT::from(
                    self.index
                        .map(|i| format!("pill-action-{}", ACTIONS[i].id))
                        .unwrap_or_else(|| "koett-pill".into())
                        .as_str(),
                ),
                p if p == UIA_ControlTypePropertyId => VARIANT::from(if self.index.is_some() {
                    UIA_ButtonControlTypeId.0
                } else {
                    UIA_PaneControlTypeId.0
                }),
                p if p == UIA_LiveSettingPropertyId => {
                    VARIANT::from(if self.index.is_none() { 1_i32 } else { 0_i32 })
                }
                p if p == UIA_IsControlElementPropertyId || p == UIA_IsContentElementPropertyId => {
                    VARIANT::from(true)
                }
                p if p == UIA_IsKeyboardFocusablePropertyId
                    || p == UIA_HasKeyboardFocusPropertyId =>
                {
                    VARIANT::from(false)
                }
                p if p == UIA_IsEnabledPropertyId => {
                    VARIANT::from(self.index.is_none_or(|i| snapshot.visible(i)))
                }
                p if p == UIA_IsOffscreenPropertyId => VARIANT::from(
                    !unsafe { IsWindowVisible(self.surface.hwnd()) }.as_bool()
                        || self.index.is_some_and(|i| !snapshot.visible(i)),
                ),
                _ => VARIANT::default(),
            })
        })();
        unsafe { output(out, result) }
    }
    unsafe fn host(&self, out: *mut *mut c_void) -> HRESULT {
        let result = self.surface.available().and_then(|_| {
            if self.index.is_none() {
                unsafe { UiaHostProviderFromHwnd(self.surface.hwnd()) }
                    .and_then(|p| p.cast())
                    .map(Some)
            } else {
                Ok(None)
            }
        });
        unsafe { interface_output(out, result) }
    }
}
impl Fragment_Impl for Node_Impl {
    unsafe fn navigate(&self, direction: NavigateDirection, out: *mut *mut c_void) -> HRESULT {
        unsafe { interface_output(out, self.neighbor(direction)) }
    }
    unsafe fn runtime_id(&self, out: *mut *mut SAFEARRAY) -> HRESULT {
        if out.is_null() {
            return E_POINTER;
        }
        let result = (|| {
            self.surface.available()?;
            let Some(index) = self.index else {
                return Ok(ptr::null_mut());
            };
            let array = unsafe { SafeArrayCreateVector(VT_I4, 0, 2) };
            if array.is_null() {
                return Err(windows::core::Error::from_hresult(E_OUTOFMEMORY));
            }
            for (position, value) in [UiaAppendRuntimeId as i32, ACTIONS[index].id as i32]
                .into_iter()
                .enumerate()
            {
                if let Err(error) = unsafe {
                    SafeArrayPutElement(array, &(position as i32), (&value as *const i32).cast())
                } {
                    unsafe {
                        let _ = SafeArrayDestroy(array);
                    }
                    return Err(error);
                }
            }
            Ok(array)
        })();
        unsafe { output(out, result) }
    }
    unsafe fn bounds(&self, out: *mut UiaRect) -> HRESULT {
        unsafe { output(out, self.rectangle()) }
    }
    unsafe fn embedded_roots(&self, out: *mut *mut SAFEARRAY) -> HRESULT {
        unsafe { output(out, Ok(ptr::null_mut())) }
    }
    unsafe fn focus(&self) -> HRESULT {
        self.surface.available().into()
    }
    unsafe fn root(&self, out: *mut *mut c_void) -> HRESULT {
        unsafe {
            interface_output(
                out,
                self.root_provider()
                    .and_then(|p| p.cast::<IRawElementProviderFragmentRoot>()?.cast())
                    .map(Some),
            )
        }
    }
}
impl Root_Impl for Node_Impl {
    unsafe fn from_point(&self, x: f64, y: f64, out: *mut *mut c_void) -> HRESULT {
        let result = (|| {
            self.surface.available()?;
            let bounds = self.rectangle()?;
            if x < bounds.left
                || x >= bounds.left + bounds.width
                || y < bounds.top
                || y >= bounds.top + bounds.height
            {
                return Ok(None);
            }
            let snapshot = self.surface.snapshot();
            for index in 0..ACTIONS.len() {
                if !snapshot.visible(index) {
                    continue;
                }
                let node = Node {
                    surface: self.surface.clone(),
                    index: Some(index),
                    root: Some(self.root_provider()?),
                };
                let provider: Simple = node.into();
                let fragment: IRawElementProviderFragment = provider.cast()?;
                let r = unsafe { fragment.BoundingRectangle()? };
                if x >= r.left && x < r.left + r.width && y >= r.top && y < r.top + r.height {
                    return provider
                        .cast::<IRawElementProviderFragment>()?
                        .cast()
                        .map(Some);
                }
            }
            self.root_provider()?
                .cast::<IRawElementProviderFragment>()?
                .cast()
                .map(Some)
        })();
        unsafe { interface_output(out, result) }
    }
    unsafe fn focused(&self, out: *mut *mut c_void) -> HRESULT {
        unsafe { interface_output(out, Ok(None)) }
    }
}
impl IInvokeProvider_Impl for Node_Impl {
    fn Invoke(&self) -> Result<()> {
        self.surface.available()?;
        let Some(index) = self.index.filter(|i| self.surface.snapshot().visible(*i)) else {
            return Err(windows::core::Error::from_hresult(HRESULT(
                UIA_E_ELEMENTNOTENABLED as i32,
            )));
        };
        unsafe {
            PostMessageW(
                Some(self.surface.hwnd()),
                WM_COMMAND,
                WPARAM(ACTIONS[index].id),
                LPARAM(0),
            )
        }
    }
}
