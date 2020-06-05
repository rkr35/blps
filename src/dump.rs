use crate::{GLOBAL_NAMES, GLOBAL_OBJECTS};
use crate::game::{Const, Object, Struct};

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};

use log::info;
use thiserror::Error;

type Modules<'a> = HashMap<&'a str, Module>;
type Submodules<'a> = HashMap<&'a str, Submodule>;

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

    #[error("unable to create directory {0:?}")]
    UnableToCreateDir(PathBuf),

    #[error("failed to convert OsString \"{0:?}\" to String")]
    StringConversion(OsString),
}

pub unsafe fn names() -> Result<(), Error> {
    const NAMES: &str = "names.txt";

    let mut dump = File::create(NAMES).map(BufWriter::new)?;

    info!("Dumping to {}", NAMES);

    for (i, name) in (*GLOBAL_NAMES).iter().enumerate() {
        
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

    for object in (*GLOBAL_OBJECTS).iter() {
        let address = object as usize;
        let object = &*object;
        
        if let Some(name) = object.full_name() {
            writeln!(&mut dump, "[{}] {} {:#x}", object.index, name, address)?;
        }
    }

    Ok(())
}

#[derive(Debug)]
struct Constant {
    name: &'static str,
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
struct Submodule {
    constants: Vec<Constant>,
    enumerations: Vec<Enumeration>,
    structures: Vec<Structure>,
}

#[derive(Debug, Default)]
struct Module {
    classes: Vec<Class>,
    submodules: Submodules<'static>,
}

unsafe fn name(object: *const Object) -> Result<&'static str, Error> {
    Ok((*object).name().ok_or(Error::NullName(object))?)
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

unsafe fn make_constant(object: *const Object) -> Result<Constant, Error> {
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
        name: name(object)?,
        value,
    })
}

unsafe fn process_constant(modules: &mut Modules, object: *const Object) -> Result<(), Error> {
    let [module, submodule] = get_module_and_submodule(object)?;

    let submodule = modules
        .entry(name(module)?)
        .or_default()
        .submodules
        .entry(name(submodule)?)
        .or_default();


    submodule.constants.push(make_constant(object)?);

    Ok(())
}

fn create_dir<P: AsRef<Path>>(path: P) -> Result<(), Error> {
    if let Err(e) = fs::create_dir(&path) {
        if e.kind() != ErrorKind::AlreadyExists {
            return Err(Error::UnableToCreateDir(path.as_ref().to_path_buf()));
        }
    }
    
    Ok(())
}

unsafe fn write_constants(path: &mut PathBuf, constants: &Vec<Constant>) -> Result<(), Error> {
    const CONSTANTS: &str = "constants.txt";
    
    path.push(CONSTANTS);
    let mut f = File::create(&path).map(BufWriter::new)?;
    path.pop();

    for constant in constants {
        writeln!(&mut f, "{} = {}", constant.name, constant.value)?;
    }

    Ok(())
}

unsafe fn write_submodules(path: &mut PathBuf, submodules: &Submodules) -> Result<(), Error> {
    for (submodule_name, submodule) in submodules {
        path.push(submodule_name);
        create_dir(&path)?;

        write_constants(path, &submodule.constants)?;
        
        path.pop();
    }

    Ok(())
}

unsafe fn write_sdk(modules: Modules) -> Result<(), Error> {
    const SDK_PATH: &str = r"C:\Users\Royce\Desktop\repos\blps\src\sdk";

    let mut path = PathBuf::from(SDK_PATH);
    create_dir(&path)?;

    for (module_name, module) in modules {
        path.push(module_name);
        create_dir(&path)?;

        write_submodules(&mut path, &module.submodules)?;

        path.pop();
    }

    Ok(())
}

pub unsafe fn sdk() -> Result<(), Error> {
    let mut modules: Modules = Modules::new();

    let constant: *const Struct = (*GLOBAL_OBJECTS)
        .find("Class Core.Const")
        .map(|o| o.cast())
        .ok_or(Error::StaticClassNotFound("Class Core.Const"))?;

    for object in (*GLOBAL_OBJECTS).iter() {
        if (*object).is(constant) {
            process_constant(&mut modules, object)?;
        }
    }

    write_sdk(modules)?;

    Ok(())
}