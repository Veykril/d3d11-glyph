use std::num::NonZeroI32;
use std::ptr;

use winapi::shared::winerror::HRESULT;
use winapi::Interface;
use wio::com::ComPtr;

pub type HResult<T> = std::result::Result<T, NonZeroI32>;

pub fn hresult(code: HRESULT) -> HResult<()> {
    match NonZeroI32::new(code) {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

pub unsafe fn com_ptr_from_fn<T, F>(fun: F) -> HResult<ComPtr<T>>
where
    T: Interface,
    F: FnOnce(&mut *mut T) -> HRESULT,
{
    let mut ptr = ptr::null_mut();
    let res = fun(&mut ptr);
    hresult(res).map(|()| ComPtr::from_raw(ptr))
}

pub unsafe fn com_ref_cast<T, U>(com_ptr: &ComPtr<T>) -> &ComPtr<U>
where
    T: std::ops::Deref<Target = U>,
    U: Interface,
{
    &*(com_ptr as *const _ as *const _)
}
