#![warn(clippy::pedantic)]

use std::io::{self, Read};
use std::os::raw::c_char;
use std::ptr;

use log::{error, info};
use simplelog::{Config, LevelFilter, TermLogger, TerminalMode};
use winapi::{
    shared::minwindef::{BOOL, DWORD, HINSTANCE, LPVOID, TRUE},
    um::{
        consoleapi::AllocConsole,
        libloaderapi::{DisableThreadLibraryCalls, FreeLibraryAndExitThread},
        processthreadsapi::CreateThread,
        synchapi::Sleep,
        wincon::FreeConsole,
        winnt::DLL_PROCESS_ATTACH,
    },
};

mod macros;
mod module;
use module::Module;

#[repr(C)]
pub struct Array<T> {
    data: *mut T,
    count: u32,
    max: u32,
}

#[repr(C)]
pub struct Name {
    pad0: [u8; 0x10],
    text: c_char,
}

#[repr(C)]
pub struct NameIndex {
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
    name: NameIndex,
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

pub type Objects = Array<*mut Object>;
pub type Names = Array<*const Name>;

pub const GLOBAL_OBJECTS: [Option<u8>; 9] = [
    Some(0x8B), Some(0x0D),
    None, None, None, None, 
    Some(0x8B), Some(0x34), Some(0xB9),
];

pub const GLOBAL_NAMES: [Option<u8>; 12] = [
    Some(0x66), Some(0x0F), Some(0xEF), Some(0xC0), Some(0x66), Some(0x0F), Some(0xD6), Some(0x05),
    None, None, None, None,
];

fn idle() {
    println!("Idling. Press enter to continue.");
    let mut sentinel = [0; 2];
    let _ = io::stdin().read_exact(&mut sentinel);
}

unsafe extern "system" fn on_attach(dll: LPVOID) -> DWORD {
    AllocConsole();
    println!("Allocated console.");

    if let Err(e) = TermLogger::init(LevelFilter::Info, Config::default(), TerminalMode::Mixed) {
        eprintln!("Failed to initialize logger: {}", e);
    } else {
        info!("Initialized logger.");

        match Module::from("BorderlandsPreSequel.exe") {
            Ok(game) => {
                info!("{:#x?}", game);
                
                // Find global objects.
                if let Some(global_objects) = game.find_pattern(&GLOBAL_OBJECTS) {
                    let global_objects = (global_objects + 2) as *const *const Objects;
                    let global_objects = global_objects.read_unaligned();
                    let global_objects = &*global_objects;

                    info!("global_objects = {:?}, {}, {}", global_objects.data, global_objects.count, global_objects.max);

                    if let Some(global_names) = game.find_pattern(&GLOBAL_NAMES) {
                        // Find global names.
                        let global_names = (global_names + 8) as *const *const Names;
                        let global_names = global_names.read_unaligned();
                        let global_names = &*global_names;
    
                        info!("global_names = {:?}, {}, {}", global_names.data, global_names.count, global_names.max);

                    } else {
                        error!("Unable to find global names.");
                    }
                } else {
                    error!("Unable to find global objects.");
                }
            }

            Err(e) => eprintln!("{}", e)
        }
    }

    idle();
    println!("Sleeping 1 second before detaching.");
    Sleep(1000);

    FreeConsole();
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