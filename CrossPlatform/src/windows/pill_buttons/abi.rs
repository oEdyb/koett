//! Raw UIA signatures preserve nullable COM outputs, which the generated
//! Result<Interface> traits cannot express. Every nullable output is initialized.
use std::ffi::c_void;
use windows::{
    Win32::{
        System::{Com::SAFEARRAY, Variant::VARIANT},
        UI::Accessibility::*,
    },
    core::{HRESULT, IUnknown, IUnknown_Vtbl, Interface, Result, interface},
};

#[interface("d6dd68d1-86fd-4332-8666-9abedea2d24c")]
pub unsafe trait Simple: IUnknown {
    fn options(&self, value: *mut ProviderOptions) -> HRESULT;
    fn pattern(&self, id: UIA_PATTERN_ID, value: *mut *mut c_void) -> HRESULT;
    fn property(&self, id: UIA_PROPERTY_ID, value: *mut VARIANT) -> HRESULT;
    fn host(&self, value: *mut *mut c_void) -> HRESULT;
}
#[interface("f7063da8-8359-439c-9297-bbc5299a7d87")]
pub unsafe trait Fragment: IUnknown {
    fn navigate(&self, direction: NavigateDirection, value: *mut *mut c_void) -> HRESULT;
    fn runtime_id(&self, value: *mut *mut SAFEARRAY) -> HRESULT;
    fn bounds(&self, value: *mut UiaRect) -> HRESULT;
    fn embedded_roots(&self, value: *mut *mut SAFEARRAY) -> HRESULT;
    fn focus(&self) -> HRESULT;
    fn root(&self, value: *mut *mut c_void) -> HRESULT;
}
#[interface("620ce2a5-ab8f-40a9-86cb-de3c75599b58")]
pub unsafe trait Root: IUnknown {
    fn from_point(&self, x: f64, y: f64, value: *mut *mut c_void) -> HRESULT;
    fn focused(&self, value: *mut *mut c_void) -> HRESULT;
}
pub unsafe fn output<T: Default>(out: *mut T, result: Result<T>) -> HRESULT {
    if out.is_null() {
        return HRESULT(0x80004003_u32 as i32);
    }
    unsafe {
        out.write(T::default());
    }
    match result {
        Ok(value) => {
            unsafe {
                out.write(value);
            }
            HRESULT(0)
        }
        Err(error) => error.into(),
    }
}
pub unsafe fn interface_output(out: *mut *mut c_void, result: Result<Option<IUnknown>>) -> HRESULT {
    if out.is_null() {
        return HRESULT(0x80004003_u32 as i32);
    }
    unsafe {
        output(
            out,
            result.map(|value| value.map_or(std::ptr::null_mut(), |v| v.into_raw())),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_interfaces_clear_an_uninitialized_output() {
        let mut value = std::ptr::dangling_mut::<c_void>();
        let result = unsafe { interface_output(&mut value, Ok(None)) };
        assert_eq!(result, HRESULT(0));
        assert!(value.is_null());
    }
    #[test]
    fn raw_interfaces_match_windows_contracts() {
        assert_eq!(Simple::IID, IRawElementProviderSimple::IID);
        assert_eq!(Fragment::IID, IRawElementProviderFragment::IID);
        assert_eq!(Root::IID, IRawElementProviderFragmentRoot::IID);
        assert_eq!(
            size_of::<Simple_Vtbl>(),
            size_of::<IRawElementProviderSimple_Vtbl>()
        );
        assert_eq!(
            size_of::<Fragment_Vtbl>(),
            size_of::<IRawElementProviderFragment_Vtbl>()
        );
        assert_eq!(
            size_of::<Root_Vtbl>(),
            size_of::<IRawElementProviderFragmentRoot_Vtbl>()
        );
    }
}
