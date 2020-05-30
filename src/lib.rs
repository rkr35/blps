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

#[repr(C)]
pub struct Array<T> {
    data: *mut T,
    count: u32,
    max: u32,
}

#[repr(C)]
pub struct Name {
    index: u32,
    number: u32,
}

#[repr(C)]
pub struct Object {
    vtable: usize,
    pad0: [u8; 0x1c],
    index: u32,
    pad1: [u8; 0x4],
    pub outer: *mut Object,
    name: Name,
    class: *mut Struct,
    pad2: [u8; 0x4],
}

#[repr(C)]
pub struct Field {
    object: Object,
    pub next: *mut Field,
}

#[repr(C)]
pub struct Struct {
    field: Field,
    pad0: [u8; 8],
    pub super_field: *mut Struct,
    pub children: *mut Field,
    pub property_size: u16,
    pad1: [u8; 0x3a],
}

pub type GlobalObjects = Array<Object>;
// pub type GlobalNames = Array<Name>;

unsafe extern "system" fn on_attach(dll: LPVOID) -> DWORD {
    msg_box!("Press OK to free library.", "on_attach()");

    // Find global objects.
    // Find global names.

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