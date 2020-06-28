#![warn(clippy::pedantic)]
#![allow(clippy::find_map)]

#[cfg(not(all(target_arch = "x86", target_os = "windows")))]
compile_error!("You must compile this crate as a 32-bit Windows .DLL.");

use std::ffi::c_void;
use std::io::{self, Read};
use std::ptr;

use log::{error, info, warn};
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

mod hook;

mod macros;

mod module;
use module::Module;

mod timeit;
use timeit::TimeIt;

pub static mut GLOBAL_NAMES: *const Names = ptr::null();
pub static mut GLOBAL_OBJECTS: *const Objects = ptr::null();
pub static mut PROCESS_EVENT: *mut c_void = ptr::null_mut();

fn idle() {
    println!("Idling. Press enter to continue.");
    let mut sentinel = [0; 2];
    let _ = io::stdin().read_exact(&mut sentinel);
}

#[derive(Error, Debug)]
enum Error {
    #[error("dump error: {0}")]
    Dump(#[from] dump::Error),

    #[error("hook error: {0}")]
    Hook(#[from] hook::Error),

    #[error("{0}")]
    Module(#[from] module::Error),

    #[error("cannot find global names")]
    NamesNotFound,

    #[error("cannot find global objects")]
    ObjectsNotFound,

    #[error("cannot find ProcessEvent")]
    ProcessEventNotFound,
}

unsafe fn find_global_names(game: &Module) -> Result<*const Names, Error> {
    const PATTERN: [Option<u8>; 12] = [
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
        .find_pattern(&PATTERN)
        .ok_or(Error::NamesNotFound)?;

    let global_names = (global_names + 8) as *const *const Names;

    Ok(global_names.read_unaligned())
}

unsafe fn find_global_objects(game: &Module) -> Result<*const Objects, Error> {
    const PATTERN: [Option<u8>; 9] = [
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
        .find_pattern(&PATTERN)
        .ok_or(Error::ObjectsNotFound)?;

    let global_objects = (global_objects + 2) as *const *const Objects;
    
    Ok(global_objects.read_unaligned())
}

unsafe fn find_process_event(game: &Module) -> Result<*mut c_void, Error> {
    const PATTERN: [Option<u8>; 15] = [Some(0x50), Some(0x51), Some(0x52), Some(0x8B), Some(0xCE), Some(0xE8), None, None, None, None, Some(0x5E), Some(0x5D), Some(0xC2), Some(0x0C), Some(0x00)];

    // 1. Find the first address A that matches the above pattern.
    let a = game.find_pattern(&PATTERN).ok_or(Error::ProcessEventNotFound)?;

    // 2. Offset A by six bytes to get the address of the CALL immediate. Call that address B.
    let b = a + 6;

    // 3. Do an unaligned* usize pointer read operation on B to get the call immediate. Call that immediate I.
    let i = (b as *const usize).read_unaligned();

    // 4. Offset B by four bytes to get the address of the instruction following the CALL instruction. Call that address C.
    let c = b + 4;

    // 5. The address of ProcessEvent is C + I, where '+' is a wrapping add.
    Ok(c.wrapping_add(i) as *mut _)
}

unsafe fn find_globals() -> Result<(), Error> {
    let _time = TimeIt::new("find globals");

    let game = Module::from("BorderlandsPreSequel.exe")?;

    GLOBAL_NAMES = find_global_names(&game)?;
    info!("GLOBAL_NAMES = {:?}", GLOBAL_NAMES);

    GLOBAL_OBJECTS = find_global_objects(&game)?;
    info!("GLOBAL_OBJECTS = {:?}", GLOBAL_OBJECTS);

    PROCESS_EVENT = find_process_event(&game)?;
    info!("PROCESS_EVENT = {:?}", PROCESS_EVENT);

    Ok(())
}

unsafe fn run() -> Result<(), Error> {
    find_globals()?;
    // dump::names()?;
    // dump::objects()?;
    dump::sdk()?;

    {
        let _hook = hook::Hook::new()?;
        idle();
    }

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
