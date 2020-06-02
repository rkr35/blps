use crate::{GLOBAL_NAMES, GLOBAL_OBJECTS};
use crate::game::{Struct};

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::ptr;

use log::info;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("unable to find static class for \"{0}\"")]
    StaticClassNotFound(&'static str),
}

pub unsafe fn names() -> Result<(), Error> {
    const NAMES: &str = "names.txt";

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

pub unsafe fn objects() -> Result<(), Error> {
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

struct Const {
}

struct Enum {
}

struct Structure {
}

struct Class {
}

struct Submodule {
    consts: Vec<Const>,
    enums: Vec<Enum>,
    structs: Vec<Struct>,
}

struct Module {
    classes: Vec<Class>,
    submodules: HashMap<String, Submodule>,
}

pub unsafe fn sdk() -> Result<(), Error> {
    let mut modules: HashMap<String, Module> = HashMap::new();

    let constant: *const Struct = (*GLOBAL_OBJECTS)
        .find("Class Core.Const")
        .map(|o| o.cast())
        .ok_or(Error::StaticClassNotFound("Class Core.Const"))?;

    for &object in (*GLOBAL_OBJECTS).iter().filter(|o| !o.is_null()) {
        if (*object).is(constant) {
            
        }
    }

    Ok(())
}