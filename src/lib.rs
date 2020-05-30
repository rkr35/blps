#![warn(clippy::pedantic)]

use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
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


mod game;
use game::{Objects, Names};

mod macros;

mod module;
use module::Module;

const NAMES: &str = "names.txt";

pub static mut GLOBAL_OBJECTS: *const Objects = ptr::null();
pub static mut GLOBAL_NAMES: *const Names = ptr::null();

fn idle() {
    println!("Idling. Press enter to continue.");
    let mut sentinel = [0; 2];
    let _ = io::stdin().read_exact(&mut sentinel);
}

#[derive(Error, Debug)]
enum Error {
    #[error("{0}")]
    Module(#[from] module::Error),

    #[error("cannot find global objects")]
    ObjectsNotFound,

    #[error("cannot find global names")]
    NamesNotFound,

    #[error("dump io error: {0}")]
    DumpIoError(#[from] io::Error),
}

unsafe fn find_globals() -> Result<(), Error> {
    let game = Module::from("BorderlandsPreSequel.exe")?;
    
    let pattern = [Some(0x8B), Some(0x0D), None, None, None, None, Some(0x8B), Some(0x34), Some(0xB9)];

    let global_objects = game.find_pattern(&pattern).ok_or(Error::ObjectsNotFound)?;
    let global_objects = (global_objects + 2) as *const *const Objects;
    let global_objects = global_objects.read_unaligned();
    GLOBAL_OBJECTS = global_objects;

    let pattern = [
        Some(0x66), Some(0x0F), Some(0xEF), Some(0xC0), Some(0x66), Some(0x0F), Some(0xD6), Some(0x05),
        None, None, None, None,
    ];

    let global_names = game.find_pattern(&pattern).ok_or(Error::NamesNotFound)?;
    let global_names = (global_names + 8) as *const *const Names;
    let global_names = global_names.read_unaligned();
    GLOBAL_NAMES = global_names;

    Ok(())
}

unsafe fn dump_objects() -> Result<(), Error> {
    const OBJECTS: &str = "objects.txt";

    let mut dump = File::create(OBJECTS).map(BufWriter::new)?;

    info!("Dumping to {}", OBJECTS);

    for &object in (*GLOBAL_OBJECTS).iter() {
        if object.is_null() {
            continue;
        }

        let address = object as usize;
        let object = &*object;
        
        if let Some(name) = object.full_name() {
            writeln!(&mut dump, "[{}] {} {:#x}", object.index, name, address)?;
        }
    }

    Ok(())
}

unsafe fn dump_names() -> Result<(), Error> {
    let mut dump = File::create(NAMES).map(BufWriter::new)?;

    info!("Dumping to {}", NAMES);

    for (i, &name) in (*GLOBAL_NAMES).iter().enumerate() {
        if name.is_null() {
            continue;
        }
        
        writeln!(&mut dump, "[{}] {}", i, (*name).text())?;
    }

    Ok(())
}

unsafe fn run() -> Result<(), Error> {
    find_globals()?;
    dump_objects()?;
    dump_names()?;
    Ok(())
}

unsafe extern "system" fn on_attach(dll: LPVOID) -> DWORD {
    AllocConsole();
    println!("Allocated console.");

    if let Err(e) = TermLogger::init(LevelFilter::Info, Config::default(), TerminalMode::Mixed) {
        eprintln!("Failed to initialize logger: {}", e);
    } else {
        info!("Initialized logger.");

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
        CreateThread(ptr::null_mut(), 0, Some(on_attach), dll.cast(), 0, ptr::null_mut());
    }

    TRUE
}