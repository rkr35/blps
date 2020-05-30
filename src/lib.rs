#![warn(clippy::pedantic)]

use std::ptr;

use wchar::wch_c as w;
use winapi::{
    shared::minwindef::{BOOL, DWORD, HINSTANCE, LPVOID, TRUE},
    um::{
        libloaderapi::{DisableThreadLibraryCalls, FreeLibraryAndExitThread},
        processthreadsapi::CreateThread,
        winnt::DLL_PROCESS_ATTACH,
        winuser::{MessageBoxW, MB_OK},
    },
};

macro_rules! msg_box {
    ($text:literal, $caption:literal) => {
        let text = w!($text);
        let caption = w!($caption);
        MessageBoxW(ptr::null_mut(), text.as_ptr(), caption.as_ptr(), MB_OK)
    }
}

unsafe extern "system" fn on_attach(dll: LPVOID) -> DWORD {
    msg_box!("Press OK to free library.", "on_attach()");

    FreeLibraryAndExitThread(dll.cast(), 0);

    0
}

#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "system" fn DllMain(dll: HINSTANCE, reason: DWORD, _: LPVOID) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        DisableThreadLibraryCalls(dll);
        CreateThread(ptr::null_mut(), 0, Some(on_attach), dll.cast(), 0, ptr::null_mut());
    }

    TRUE
}