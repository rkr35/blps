use crate::{GLOBAL_NAMES, GLOBAL_OBJECTS};
use crate::game::{Const, Object, Struct};

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufWriter, Write};

use log::info;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("unable to find static class for \"{0}\"")]
    StaticClassNotFound(&'static str),

    #[error("the path length for {0:?} is fewer than two")]
    OutersIsFewerThanTwo(*const Object),

    #[error("null name for {0:?}")]
    NullName(*const Object),
}

pub unsafe fn names() -> Result<(), Error> {
    const NAMES: &str = "names.txt";

    let mut dump = File::create(NAMES).map(BufWriter::new)?;

    info!("Dumping to {}", NAMES);

    for (i, &name) in (*GLOBAL_NAMES).iter().enumerate() {
        if name.is_null() {
            continue;
        }
        
        if let Some(text) = (*name).text() {
            writeln!(&mut dump, "[{}] {}", i, text)?;
        }
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

#[derive(Debug)]
struct Constant<'n> {
    name: &'n str,
    value: OsString,
}

#[derive(Debug)]
struct Enumeration {
}

#[derive(Debug)]
struct Structure {
}

#[derive(Debug)]
struct Class {
}

#[derive(Debug, Default)]
struct Submodule<'n> {
    consts: Vec<Constant<'n>>,
    enums: Vec<Enumeration>,
    structs: Vec<Structure>,
}

#[derive(Debug, Default)]
struct Module<'sm, 'n> {
    classes: Vec<Class>,
    submodules: HashMap<&'sm str, Submodule<'n>>,
}

pub unsafe fn sdk() -> Result<(), Error> {
    let mut modules: HashMap<&str, Module> = HashMap::new();

    let constant: *const Struct = (*GLOBAL_OBJECTS)
        .find("Class Core.Const")
        .map(|o| o.cast())
        .ok_or(Error::StaticClassNotFound("Class Core.Const"))?;

    for &object in (*GLOBAL_OBJECTS).iter().filter(|o| !o.is_null()) {
        if (*object).is(constant) {
            let [module, submodule] = (*object)
                .iter_outer()
                .fold(
                    [None, None],
                    |[module, _], outer| [Some(outer), module]
                );

            let module = module.ok_or(Error::OutersIsFewerThanTwo(object))?;
            let submodule = submodule.ok_or(Error::OutersIsFewerThanTwo(object))?;

            let submodule = modules
                .entry(module.name().ok_or(Error::NullName(module))?)
                .or_default()
                .submodules
                .entry(submodule.name().ok_or(Error::NullName(submodule))?)
                .or_default();

            submodule.consts.push(Constant {
                name: (*object).name().ok_or(Error::NullName(object))?,
                value: (*object.cast::<Const>()).value.to_string(),
            });
        }
    }

    Ok(())
}