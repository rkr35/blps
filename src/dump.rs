use crate::{GLOBAL_NAMES, GLOBAL_OBJECTS};
use crate::game::{Const, Object, Struct};

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, BufWriter, ErrorKind, Write};
use std::path::PathBuf;

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

    #[error("unable to create SDK folder: {0}")]
    UnableToCreateSdkFolder(io::Error),

    #[error("unable to create module or submodule folder: {0}")]
    UnableToCreateModuleFolder(io::Error),

    #[error("failed to convert OsString \"{0:?}\" to String")]
    StringConversion(OsString),
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
    value: String,
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

unsafe fn get_module_and_submodule(object: *const Object) -> Result<[*const Object; 2], Error> {
    let [module, submodule] = (*object)
        .iter_outer()
        .fold(
            [None, None],
            |[module, _], outer| [Some(outer), module]
        );

    let module = module.ok_or(Error::OutersIsFewerThanTwo(object))?;
    let submodule = submodule.ok_or(Error::OutersIsFewerThanTwo(object))?;

    Ok([module, submodule])
}

unsafe fn make_constant(object: *const Object) -> Result<Constant<'static>, Error> {
    let value = {
        // Cast so we can access fields of constant.
        let object: *const Const = object.cast();

        // Construct a printable string.
        let value: OsString = (*object).value.to_string();
        let mut value: String = value.into_string().map_err(Error::StringConversion)?;
        
        // The strings in memory are C strings, so they have null terminators that
        // Rust strings don't care for.
        // Get rid of that null-terminator so we don't see a funky '?' in the human-
        // readable output.
        if value.ends_with(char::from(0)) {
            value.pop();
        }

        value
    };

    Ok(Constant {
        name: (*object).name().ok_or(Error::NullName(object))?,
        value,
    })
}

unsafe fn process_constant(modules: &mut HashMap<&str, Module>, object: *const Object) -> Result<(), Error> {
    let [module, submodule] = get_module_and_submodule(object)?;

    let submodule = modules
        .entry((*module).name().ok_or(Error::NullName(module))?)
        .or_default()
        .submodules
        .entry((*submodule).name().ok_or(Error::NullName(submodule))?)
        .or_default();


    submodule.consts.push(make_constant(object)?);

    Ok(())
}

pub unsafe fn sdk() -> Result<(), Error> {
    let mut modules: HashMap<&str, Module> = HashMap::new();

    let constant: *const Struct = (*GLOBAL_OBJECTS)
        .find("Class Core.Const")
        .map(|o| o.cast())
        .ok_or(Error::StaticClassNotFound("Class Core.Const"))?;

    for &object in (*GLOBAL_OBJECTS).iter().filter(|o| !o.is_null()) {
        if (*object).is(constant) {
            process_constant(&mut modules, object)?;
        }
    }

    const SDK_PATH: &str = r"C:\Users\Royce\Desktop\repos\blps\src\sdk";
    let mut path = PathBuf::from(SDK_PATH);

    if let Err(e) = fs::create_dir(&path) {
        if e.kind() != ErrorKind::AlreadyExists {
            return Err(Error::UnableToCreateSdkFolder(e));
        }
    }

    for (module_name, module) in modules {
        path.push(module_name);

        if let Err(e) = fs::create_dir(&path) {
            if e.kind() != ErrorKind::AlreadyExists {
                return Err(Error::UnableToCreateModuleFolder(e));
            }
        }

        for (submodule_name, submodule) in module.submodules {
            path.push(submodule_name);

            if let Err(e) = fs::create_dir(&path) {
                if e.kind() != ErrorKind::AlreadyExists {
                    return Err(Error::UnableToCreateModuleFolder(e));
                }
            }

            const CONSTANTS: &str = "constants.txt";

            // Write out the constants in a file named "constants.txt"
            path.push(CONSTANTS);
            let mut constants = File::create(&path).map(BufWriter::new)?;
            path.pop();

            for constant in submodule.consts {
                writeln!(&mut constants, "{} = {}", constant.name, constant.value)?;
            }

            path.pop();
        }

        path.pop();
    }

    Ok(())
}