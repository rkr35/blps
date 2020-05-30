#![warn(clippy::pedantic)]

use std::borrow::Cow;
use std::ffi::CStr;
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::iter;
use std::ops::Deref;
use std::os::raw::c_char;
use std::ptr;
use std::slice;

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

pub type Objects = Array<*mut Object>;
pub type Names = Array<*const Name>;

const OBJECTS: &str = "objects.txt";
const NAMES: &str = "names.txt";

pub static mut GLOBAL_OBJECTS: *const Objects = ptr::null();
pub static mut GLOBAL_NAMES: *const Names = ptr::null();

#[repr(C)]
pub struct Array<T> {
    data: *mut T,
    count: u32,
    max: u32,
}

impl<T> Deref for Array<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe {
            slice::from_raw_parts(self.data, self.count as usize)
        }
    }
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
    pub index: u32,
    pad1: [u8; 0x4],
    pub outer: *mut Object,
    name: NameIndex,
    class: *mut Struct,
    pad2: [u8; 0x4],
}

impl Object {
    pub unsafe fn full_name(&self) -> Option<String> {
        if self.class.is_null() {
            return None;
        }

        let outer_names: Option<Vec<_>> = self.iter_outer().map(|o| o.name()).collect();
        let mut outer_names = outer_names?;
        outer_names.reverse();
        let name = outer_names.join(".");

        let class = String::from((*self.class).field.object.name()?);

        Some(class + " " + &name)
    }

    pub unsafe fn iter_outer(&self) -> impl Iterator<Item = &Self> {
        iter::successors(Some(self), |current| current.outer.as_ref())
    }

    pub unsafe fn name(&self) -> Option<Cow<str>> {
        let name = *(*GLOBAL_NAMES).get(self.name.index as usize)?;

        if name.is_null() {
            return None;
        }

        let name = CStr::from_ptr(&(*name).text as *const c_char);
        Some(name.to_string_lossy())
    }
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
                
                let pattern = [Some(0x8B), Some(0x0D), None, None, None, None, Some(0x8B), Some(0x34), Some(0xB9)];

                if let Some(global_objects) = game.find_pattern(&pattern) {
                    let global_objects = (global_objects + 2) as *const *const Objects;
                    let global_objects = global_objects.read_unaligned();
                    GLOBAL_OBJECTS = global_objects;

                    let pattern = [
                        Some(0x66), Some(0x0F), Some(0xEF), Some(0xC0), Some(0x66), Some(0x0F), Some(0xD6), Some(0x05),
                        None, None, None, None,
                    ];

                    if let Some(global_names) = game.find_pattern(&pattern) {
                        let global_names = (global_names + 8) as *const *const Names;
                        let global_names = global_names.read_unaligned();
                        GLOBAL_NAMES = global_names;

                        if let Ok(mut objects_dump) = File::create(OBJECTS).map(BufWriter::new) {
                            let global_objects = &*global_objects;

                            info!("Dumping to {}", OBJECTS);
                            for &object in global_objects.iter() {
                                if object.is_null() {
                                    continue;
                                }
        
                                let address = object as usize;
                                let object = &*object;
                                
                                if let Some(name) = object.full_name() {
                                    let _ = writeln!(&mut objects_dump, "[{}] {} {:#x}", object.index, name, address);
                                }
                            }

                            info!("Dumping to {}", NAMES);

                        } else {
                            error!("Unable to create {}", OBJECTS);
                        }
                    } else {
                        error!("Unable to find global names.");
                    }
                } else {
                    error!("Unable to find global objects.");
                }
            }

            Err(e) => error!("{}", e)
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