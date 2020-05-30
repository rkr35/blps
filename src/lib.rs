#![warn(clippy::pedantic)]

use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
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


mod game;
use game::{Objects, Names};

mod macros;

mod module;
use module::Module;

const OBJECTS: &str = "objects.txt";
const NAMES: &str = "names.txt";

pub static mut GLOBAL_OBJECTS: *const Objects = ptr::null();
pub static mut GLOBAL_NAMES: *const Names = ptr::null();

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
                            info!("Dumping to {}", OBJECTS);

                            for &object in (*global_objects).iter() {
                                if object.is_null() {
                                    continue;
                                }
        
                                let address = object as usize;
                                let object = &*object;
                                
                                if let Some(name) = object.full_name() {
                                    let _ = writeln!(&mut objects_dump, "[{}] {} {:#x}", object.index, name, address);
                                }
                            }

                            if let Ok(mut names_dump) = File::create(NAMES).map(BufWriter::new) {
                                info!("Dumping to {}", NAMES);

                                for (i, &name) in (*global_names).iter().enumerate() {
                                    if name.is_null() {
                                        continue;
                                    }
                                    
                                    let _ = writeln!(&mut names_dump, "[{}] {}", i, (*name).text());
                                }
                            } else {
                                error!("Unable to create {}", NAMES);
                            }
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