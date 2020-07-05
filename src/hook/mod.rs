use crate::game;
use crate::PROCESS_EVENT;

use std::ffi::c_void;
use std::mem;

use detours_sys::{
    DetourAttach, DetourDetach, DetourTransactionBegin, DetourTransactionCommit,
    DetourUpdateThread, LONG as DetourErrorCode,
};
use log::error;
use thiserror::Error;
use winapi::um::processthreadsapi::GetCurrentThread;

mod cached_function_indexes;
use cached_function_indexes::CachedFunctionIndexes;

mod bitfield;
mod sdk;

mod user;

pub static mut CACHED_FUNCTION_INDEXES: Option<CachedFunctionIndexes> = None;

#[derive(Error, Debug)]
pub enum Error {
    #[error("cached function indexes error: {0}")]
    CFI(#[from] cached_function_indexes::Error),

    #[error("detour error: {0} returned {1}")]
    Detour(&'static str, DetourErrorCode),
}

/// A helper macro to call Detour functions and wrap any error codes into a
/// variant of the top-level `Error` enum.
macro_rules! det {
    ($call:expr) => {{
        const NO_ERROR: DetourErrorCode = 0;

        let error_code = $call;

        if error_code == NO_ERROR {
            Ok(())
        } else {
            Err(Error::Detour(stringify!($call), error_code))
        }
    }};
}

pub struct Hook;

impl Hook {
    pub unsafe fn new() -> Result<Hook, Error> {
        CACHED_FUNCTION_INDEXES = Some(CachedFunctionIndexes::new()?);
        hook_process_event()?;
        Ok(Hook)
    }
}

impl Drop for Hook {
    fn drop(&mut self) {
        unsafe {
            if let Err(e) = unhook_process_event() {
                error!("{}", e);
            }
        }
    }
}

unsafe fn hook_process_event() -> Result<(), Error> {
    det!(DetourTransactionBegin())?;
    det!(DetourUpdateThread(GetCurrentThread()))?;
    det!(DetourAttach(&mut PROCESS_EVENT, my_process_event as *mut _))?;
    det!(DetourTransactionCommit())?;
    Ok(())
}

unsafe fn unhook_process_event() -> Result<(), Error> {
    det!(DetourTransactionBegin())?;
    det!(DetourUpdateThread(GetCurrentThread()))?;
    det!(DetourDetach(&mut PROCESS_EVENT, my_process_event as *mut _))?;
    det!(DetourTransactionCommit())?;
    Ok(())
}

unsafe extern "fastcall" fn my_process_event(
    this: &game::Object,
    edx: usize,
    function: &game::Function,
    parameters: *mut c_void,
    return_value: *mut c_void,
) {
    type ProcessEvent = unsafe extern "fastcall" fn(
        this: &game::Object,
        _edx: usize,
        function: &game::Function,
        parameters: *mut c_void,
        return_value: *mut c_void,
    );

    let original = mem::transmute::<*mut c_void, ProcessEvent>(PROCESS_EVENT);
    user::process_event(this, function, parameters, return_value);
    original(this, edx, function, parameters, return_value);
}
