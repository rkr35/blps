#![warn(clippy::pedantic)]
#![allow(clippy::find_map)]

#[cfg(not(all(target_arch = "x86", target_os = "windows")))]
compile_error!("You must compile this crate as a 32-bit Windows .DLL.");

use std::io::{self, Read};
use std::ptr;

use log::{error, info};
use simplelog::{Config, LevelFilter, TermLogger, TerminalMode};
use thiserror::Error;
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

mod dump;

mod game;
use game::{Names, Objects};

mod macros;

mod module;
use module::Module;

mod timeit;
use timeit::TimeIt;

#[cfg(feature = "include_sdk")]
mod sdk;

pub static mut GLOBAL_NAMES: *const Names = ptr::null();
pub static mut GLOBAL_OBJECTS: *const Objects = ptr::null();

fn idle() {
    println!("Idling. Press enter to continue.");
    let mut sentinel = [0; 2];
    let _ = io::stdin().read_exact(&mut sentinel);
}

#[derive(Error, Debug)]
enum Error {
    #[error("dump error: {0}")]
    Dump(#[from] dump::Error),

    #[error("{0}")]
    Module(#[from] module::Error),

    #[error("cannot find global names")]
    NamesNotFound,

    #[error("cannot find global objects")]
    ObjectsNotFound,
}

unsafe fn find_globals() -> Result<(), Error> {
    let _time = TimeIt::new("find globals");

    let game = Module::from("BorderlandsPreSequel.exe")?;

    let names_pattern = [
        Some(0x66),
        Some(0x0F),
        Some(0xEF),
        Some(0xC0),
        Some(0x66),
        Some(0x0F),
        Some(0xD6),
        Some(0x05),
        None,
        None,
        None,
        None,
    ];

    let global_names = game
        .find_pattern(&names_pattern)
        .ok_or(Error::NamesNotFound)?;
    let global_names = (global_names + 8) as *const *const Names;
    let global_names = global_names.read_unaligned();
    GLOBAL_NAMES = global_names;
    info!("GLOBAL_NAMES = {:?}", GLOBAL_NAMES);

    let objects_pattern = [
        Some(0x8B),
        Some(0x0D),
        None,
        None,
        None,
        None,
        Some(0x8B),
        Some(0x34),
        Some(0xB9),
    ];

    let global_objects = game
        .find_pattern(&objects_pattern)
        .ok_or(Error::ObjectsNotFound)?;
    let global_objects = (global_objects + 2) as *const *const Objects;
    let global_objects = global_objects.read_unaligned();
    GLOBAL_OBJECTS = global_objects;
    info!("GLOBAL_OBJECTS = {:?}", GLOBAL_OBJECTS);

    Ok(())
}

unsafe fn run() -> Result<(), Error> {
    find_globals()?;
    // dump::names()?;
    // dump::objects()?;
    dump::sdk()?;
    Ok(())
}

unsafe extern "system" fn on_attach(dll: LPVOID) -> DWORD {
    AllocConsole();
    println!("Allocated console.");

    if let Err(e) = TermLogger::init(LevelFilter::Info, Config::default(), TerminalMode::Mixed) {
        eprintln!("Failed to initialize logger: {}", e);
    } else {
        info!("Initialized logger.");

        let _time = TimeIt::new("run()");

        if let Err(e) = run() {
            error!("{}", e);
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
        CreateThread(
            ptr::null_mut(),
            0,
            Some(on_attach),
            dll.cast(),
            0,
            ptr::null_mut(),
        );
    }

    TRUE
}
