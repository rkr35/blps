use crate::{GLOBAL_NAMES, GLOBAL_OBJECTS};
use crate::game::{BoolProperty, ByteProperty, Class, Const, Enum, Object, Property, Struct};
use crate::TimeIt;

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, ErrorKind, Write};
use std::iter;
use std::mem;
use std::path::{Path, PathBuf};
use std::ptr;

use codegen::{Impl, Scope};
use log::info;
use thiserror::Error;
use typed_builder::TypedBuilder;

static mut CONSTANT: *const Class = ptr::null();
static mut ENUMERATION: *const Class = ptr::null();
static mut STRUCTURE: *const Class = ptr::null();
static mut BYTE_PROPERTY: *const Class = ptr::null();
static mut BOOL_PROPERTY: *const Class = ptr::null();

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

    #[error("failed to get variants of the enum {0:?}")]
    Variants(*const Enum),

    #[error("unknown property type for {0:?}")]
    UnknownProperty(*const Property),
}

type Modules<'a> = HashMap<&'a str, Module>;
type Submodules<'a> = HashMap<&'a str, Submodule>;

struct StagingFile {
    pub file: BufWriter<File>,
    pub scope: Scope,
}

impl StagingFile {
    pub fn from(path: &mut PathBuf, file: &str) -> Result<Self, Error> {
        path.push(file);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map(BufWriter::new)?;
        path.pop();

        Ok(Self { file, scope: Scope::new() })
    }
}

impl Drop for StagingFile {
    fn drop(&mut self) {
        writeln!(&mut self.file, "{}", self.scope.to_string())
            .expect("flushing StagingFile");
    }
}

#[derive(Debug)]
struct Constant {
    name: &'static str,
    value: String,
}

impl Constant {
    pub unsafe fn from(object: *const Object) -> Result<Self, Error> {
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
    
        Ok(Self {
            name: name(object)?,
            value,
        })
    }
}

#[derive(Debug)]
struct Enumeration {
    name: &'static str,
    variants: Vec<&'static str>,
}

impl Enumeration {
    pub unsafe fn from(object: *const Object) -> Result<Self, Error> {
        let name = name(object)?;
        let object: *const Enum = object.cast();

        Ok(Self {
            name,
            variants: (*object).variants().ok_or(Error::Variants(object))?,
        })
    }
}

#[derive(Debug)]
enum MemberKind {
    Padding,
    Byte(*mut Enum),
    Bool,
    Struct(*const Struct),
    Unknown,
}

#[derive(Debug, TypedBuilder)]
struct Member {
    name: String,
    kind: MemberKind,
    offset: u32,
    size: u32,

    #[builder(default, setter(strip_option))]
    comment: Option<&'static str>,
}

struct PropertyInfo {
    kind: MemberKind,
    size: usize,
}

impl PropertyInfo {
    pub unsafe fn from(p: &Property) -> Result<Self, Error> {
        let (kind, size) = if p.is(BYTE_PROPERTY) {
            let p = mem::transmute::<&Property, &ByteProperty>(p);
            (MemberKind::Byte(p.enumeration), mem::size_of::<u8>())
        } else if p.is(BOOL_PROPERTY) {
            (MemberKind::Bool, mem::size_of::<u32>())
        } else {
            (MemberKind::Unknown, 0)
            // return Err(Error::UnknownProperty(p as *const Property));
        };

        Ok(Self { kind, size })
    }
}

#[derive(Debug)]
struct Structure {
    name: &'static str,
    super_class: *const Struct,
    size: usize,
    inherited_size: usize,
    members: Vec<Member>,
}

impl Structure {
    pub unsafe fn from(object: *const Object) -> Result<Self, Error> {
        static mut FUNCTION: *const Class = ptr::null();

        if FUNCTION.is_null() {
            FUNCTION = find_static_class("Class Core.Function")?;
        }

        let structure: *const Struct = object.cast();
        let size = usize::from((*structure).property_size);
        let super_class: *const Struct = (*structure).super_field.cast();

        let mut offset = 0;
        let mut inherited_size = 0;
        let mut members = vec![];

        if !super_class.is_null() && !std::ptr::eq(super_class, structure) {
            inherited_size = u32::from((*super_class).property_size);
            offset = inherited_size;
            members.push(Member::builder()
                .name("base".to_string())
                .kind(MemberKind::Struct(super_class))
                .offset(0)
                .size(inherited_size)
                .build());
        }

        let properties = iter::successors(
            (*structure).children.cast::<Property>().as_ref(),
            |property| property.next.cast::<Property>().as_ref()
        );

        let mut properties: Vec<&Property> = properties
            .filter(|p| !p.is(STRUCTURE) && !p.is(CONSTANT) & !p.is(ENUMERATION) && !p.is(FUNCTION))
            .collect();

        properties.sort_unstable_by(|p, q| 
            p.offset.cmp(&q.offset).then_with(||
                if p.is(BOOL_PROPERTY) && q.is(BOOL_PROPERTY) {
                    let p = mem::transmute::<&Property, &BoolProperty>(p);
                    let q = mem::transmute::<&Property, &BoolProperty>(q);
                    p.bitmask.cmp(&q.bitmask)
                } else {
                    Ordering::Equal
                }
            )
        );

        let mut previous_bitfield: Option<()> = None;

        for property in properties {
            if offset < property.offset {
                previous_bitfield = None;
                members.push(
                    Member::builder()
                        .name(format!("pad_at_{:#X}", offset))
                        .kind(MemberKind::Padding)
                        .offset(offset)
                        .size(property.offset - offset)
                        .comment("Missed offset. Likely alignment padding.")
                        .build()
                );
            }

            let PropertyInfo { kind, size } = PropertyInfo::from(property)?;
            members.push(Member::builder()
                .name(name(property as &Object)?.to_string())
                .kind(kind)
                .offset(offset)
                .size(size as u32)
                .build());
        }

        Ok(Self {
            name: name(object)?,
            super_class,
            size,
            inherited_size: inherited_size as usize,
            members,
        })
    }
}

#[derive(Debug)]
struct ModuleClass {
}

#[derive(Debug, Default)]
struct Submodule {
    constants: Vec<Constant>,
    enumerations: Vec<Enumeration>,
    structures: Vec<Structure>,
}

#[derive(Debug, Default)]
struct Module {
    classes: Vec<ModuleClass>,
    submodules: Submodules<'static>,
}

pub unsafe fn names() -> Result<(), Error> {
    const NAMES: &str = "names.txt";
    let _time = TimeIt::new("dump global names");

    let mut dump = File::create(NAMES).map(BufWriter::new)?;

    info!("Dumping global names {:?} to {}", GLOBAL_NAMES, NAMES);

    writeln!(&mut dump, "Global names is at {:?}", GLOBAL_NAMES)?;

    for (i, name) in (*GLOBAL_NAMES).iter().enumerate() {
        if let Some(text) = (*name).text() {
            writeln!(&mut dump, "[{}] {}", i, text)?;
        }
    }

    Ok(())
}

pub unsafe fn objects() -> Result<(), Error> {
    const OBJECTS: &str = "objects.txt";
    let _time = TimeIt::new("dump global objects");

    let mut dump = File::create(OBJECTS).map(BufWriter::new)?;

    info!("Dumping global objects {:?} to {}", GLOBAL_OBJECTS, OBJECTS);

    writeln!(&mut dump, "Global objects is at {:?}", GLOBAL_OBJECTS)?;

    for object in (*GLOBAL_OBJECTS).iter() {
        let address = object as usize;
        let object = &*object;
        
        if let Some(name) = object.full_name() {
            writeln!(&mut dump, "[{}] {} {:#x}", object.index, name, address)?;
        }
    }

    Ok(())
}

pub unsafe fn sdk() -> Result<(), Error> {
    let _time = TimeIt::new("sdk()");

    CONSTANT = find_static_class("Class Core.Const")?;
    ENUMERATION = find_static_class("Class Core.Enum")?;
    STRUCTURE = find_static_class("Class Core.ScriptStruct")?;
    BYTE_PROPERTY = find_static_class("Class Core.ByteProperty")?;
    BOOL_PROPERTY = find_static_class("Class Core.BoolProperty")?;

    let mut modules: Modules = Modules::new();

    for object in (*GLOBAL_OBJECTS).iter() {
        if (*object).is(CONSTANT) {
            get_submodule(&mut modules, object)?.constants.push(Constant::from(object)?);
        } else if (*object).is(ENUMERATION) {
            get_submodule(&mut modules, object)?.enumerations.push(Enumeration::from(object)?);
        } else if (*object).is(STRUCTURE) {
            get_submodule(&mut modules, object)?.structures.push(Structure::from(object)?);
        }
    }

    write_sdk(modules)?;

    Ok(())
}

unsafe fn find_static_class(class: &'static str) -> Result<*const Class, Error> {
    Ok((*GLOBAL_OBJECTS)
            .find(class)
            .map(|o| o.cast())
            .ok_or(Error::StaticClassNotFound(class))?)
}

unsafe fn get_submodule<'a>(modules: &'a mut Modules, object: *const Object) -> Result<&'a mut Submodule, Error> {
    let [module, submodule] = get_module_and_submodule(object)?;

    Ok(modules
        .entry(name(module)?)
        .or_default()
        .submodules
        .entry(name(submodule)?)
        .or_default())
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

unsafe fn name(object: *const Object) -> Result<&'static str, Error> {
    Ok((*object).name().ok_or(Error::NullName(object))?)
}

unsafe fn get_full_path(object: *const Object) -> Result<String, Error> {
    let [module, submodule] = get_module_and_submodule(object)?;
    let module = name(module)?;
    let submodule = name(submodule)?;
    let object = name(object)?;
    Ok(format!("sdk::{}::{}::{}", module, submodule, object))
}

fn write_sdk(modules: Modules) -> Result<(), Error> {
    const SDK_PATH: &str = r"C:\Users\Royce\Desktop\repos\blps\src\sdk";

    let _ = fs::remove_dir_all(SDK_PATH);

    let mut path = PathBuf::from(SDK_PATH);
    create_dir(&path)?;

    let mut module_file = StagingFile::from(&mut path, "mod.rs")?;

    for (module_name, module) in modules {
        path.push(module_name);
        create_dir(&path)?;
        
        write_submodules(&mut path, &module.submodules)?;
        
        let import = format!("pub mod {};", module_name);
        module_file.scope.raw(&import);

        path.pop();
    }

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

fn write_submodules(path: &mut PathBuf, submodules: &Submodules) -> Result<(), Error> {
    let mut module = StagingFile::from(path, "mod.rs")?;

    for (submodule_name, submodule) in submodules {
        let import = format!("pub use {};", submodule_name);
        module.scope.raw(&import);

        path.push(submodule_name);
        create_dir(&path)?;

        write_constants(path, &submodule.constants)?;
        write_enumerations(path, &submodule.enumerations)?;
        write_structures(path, &submodule.structures)?;

        path.pop();
    }

    Ok(())
}

fn write_constants(path: &mut PathBuf, constants: &[Constant]) -> Result<(), Error> {
    if constants.is_empty() {
        return Ok(());
    }

    path.push("constants.txt");
    let mut f = File::create(&path).map(BufWriter::new)?;
    path.pop();

    for constant in constants {
        writeln!(&mut f, "{} = {}", constant.name, constant.value)?;
    }

    Ok(())
}

fn write_enumerations(path: &mut PathBuf, enumerations: &[Enumeration]) -> Result<(), Error> {
    if enumerations.is_empty() {
        return Ok(());
    }

    let mut module = StagingFile::from(path, "mod.rs")?;
    let mut enum_file = StagingFile::from(path, "enums.rs")?;

    for Enumeration { name, variants } in enumerations {
        let e = enum_file.scope.new_enum(name).repr("u8");

        for variant in variants {
            e.new_variant(variant);
        }
    }

    let names: Vec<&str> = enumerations
        .iter()
        .map(|e| e.name)
        .collect();

    let import_names = format!("pub use enums::{{{}}};", names.join(", "));

    module.scope.raw(&import_names);

    Ok(())
}

fn write_structures(path: &mut PathBuf, structures: &[Structure]) -> Result<(), Error> {
    let mut module = StagingFile::from(path, "mod.rs")?;

    for s in structures {
        let import = format!(
            "mod {name};\n\
            pub use {name}::{name};",
            name = s.name
        );

        module.scope.raw(&import);

        let mut struct_file = StagingFile::from(path, &format!("{}.rs", s.name))?;

        struct_file.scope.import("crate", "sdk");

        if !s.super_class.is_null() {
            struct_file.scope.raw("use std::ops::{Deref, DerefMut};");
        }

        let struct_gen = struct_file
            .scope
            .new_struct(s.name)
            .vis("pub")
            .repr("C");

        for member in &s.members {
            let name = format!("pub {}", member.name);
            let ty: Cow<str> = match member.kind {
                MemberKind::Padding => format!("[u8; {}]", member.size).into(),
                MemberKind::Byte(enumeration) => if enumeration.is_null() {
                    "u8".into()
                } else {
                    unsafe { get_full_path(enumeration.cast())?.into() }
                }
                MemberKind::Bool => "u32".into(),
                MemberKind::Struct(structure) => unsafe { get_full_path(structure.cast())?.into() },
                MemberKind::Unknown => format!("UNK_{}", member.size).into(),
            };

            struct_gen.field(&name, ty.as_ref());
        }

        if !s.super_class.is_null() {
            let mut deref_impl = Impl::new(s.name);

            deref_impl
                .impl_trait("Deref")
                .associate_type("Target", unsafe { get_full_path(s.super_class.cast())? });

            deref_impl.new_fn("deref")
                .arg_ref_self()
                .ret("&Self::Target")
                .line("&self.base");

            struct_file.scope.push_impl(deref_impl);
        }
    }

    Ok(())
}