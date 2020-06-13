use crate::{GLOBAL_NAMES, GLOBAL_OBJECTS};
use crate::game::{BoolProperty, ByteProperty, Class, Const, Enum, Object, Property, Struct};
use crate::TimeIt;

use std::borrow::Cow;
use std::cmp::Ordering;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::iter;
use std::mem;
use std::ptr;

use codegen::{Impl, Scope};
use log::{info, warn};
use thiserror::Error;
use typed_builder::TypedBuilder;

static mut CONSTANT: *const Class = ptr::null();
static mut ENUMERATION: *const Class = ptr::null();
static mut STRUCTURE: *const Class = ptr::null();
static mut FUNCTION: *const Class = ptr::null();
static mut BYTE_PROPERTY: *const Class = ptr::null();
static mut BOOL_PROPERTY: *const Class = ptr::null();

#[derive(Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("unable to find static class for \"{0}\"")]
    StaticClassNotFound(&'static str),

    #[error("null name for {0:?}")]
    NullName(*const Object),

    #[error("failed to convert OsString \"{0:?}\" to String")]
    StringConversion(OsString),

    #[error("unknown property type for {0:?}")]
    UnknownProperty(*const Property),

    #[error("enum {0:?} has an unknown or ill-formed variant")]
    BadVariant(*const Enum),
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

pub unsafe fn _names() -> Result<(), Error> {
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

pub unsafe fn _objects() -> Result<(), Error> {
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
    const SDK_PATH: &str = r"C:\Users\Royce\Desktop\repos\blps\src\sdk.rs";
    
    let _time = TimeIt::new("sdk()");

    find_static_classes()?;

    let mut sdk = File::create(SDK_PATH).map(BufWriter::new)?;
    let mut scope = Scope::new();

    add_crate_attributes(&mut scope);

    for object in (*GLOBAL_OBJECTS).iter() {
        write_object(&mut scope, object)?;
    }

    writeln!(&mut sdk, "{}", scope.to_string())?;

    Ok(())
}

fn add_crate_attributes(scope: &mut Scope) {
    scope.raw("#![allow(dead_code)]\n\
               #![allow(non_camel_case_types)]");
}

unsafe fn find_static_classes() -> Result<(), Error> {
    let _time = TimeIt::new("find static classes");

    CONSTANT = find_static_class("Class Core.Const")?;
    ENUMERATION = find_static_class("Class Core.Enum")?;
    STRUCTURE = find_static_class("Class Core.ScriptStruct")?;
    FUNCTION = find_static_class("Class Core.Function")?;
    BYTE_PROPERTY = find_static_class("Class Core.ByteProperty")?;
    BOOL_PROPERTY = find_static_class("Class Core.BoolProperty")?;

    Ok(())
}

unsafe fn find_static_class(class: &'static str) -> Result<*const Class, Error> {
    Ok((*GLOBAL_OBJECTS)
            .find(class)
            .map(|o| o.cast())
            .ok_or(Error::StaticClassNotFound(class))?)
}

unsafe fn write_object(sdk: &mut Scope, object: *const Object) -> Result<(), Error> {
    if (*object).is(CONSTANT) {
        write_constant(sdk, object)?;
    } else if (*object).is(ENUMERATION) {
        write_enumeration(sdk, object)?;
    }
    //  else if (*object).is(STRUCTURE) {
    //     write_structure(&mut sdk, object)?;
    // }
    Ok(())
}

unsafe fn write_constant(sdk: &mut Scope, object: *const Object) -> Result<(), Error> {
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

    sdk.raw(&format!("// {} = {}", name(object)?, value));
    Ok(())
}

fn is_enum_duplicate(name: &str) -> bool {
    const DUPLICATES: [&str; 2] = ["ECompareObjectOutputLinkIds", "EFlightMode"];
    DUPLICATES.iter().any(|dup| name == *dup)
}

unsafe fn write_enumeration(sdk: &mut Scope, object: *const Object) -> Result<(), Error> {
    let name = name(object)?;

    if name.starts_with("Default__") {
        return Ok(());
    }

    if is_enum_duplicate(name) {
        warn!("Ignoring {} because multiple enums have this name.", name);
        return Ok(());
    }

    let enum_gen = sdk.new_enum(name).repr("u8").vis("pub");

    let object: *const Enum = object.cast();

    let mut previous: Option<&str> = None;
    let mut count = 0;

    for variant in (*object).variants() {
        let variant = variant.ok_or(Error::BadVariant(object))?;

        if let Some(prev) = previous {
            if prev == variant {
                enum_gen.new_variant(&format!("{}_{}", prev, count));
                count += 1;
            } else {
                if count > 0 {
                    enum_gen.new_variant(&format!("{}_{}", prev, count));
                } else {
                    enum_gen.new_variant(prev);
                }
                previous = Some(variant);
                count = 0;
            }
        } else {
            previous = Some(variant);
            count = 0;
        }
    }

    if let Some(previous) = previous {
        if count > 0 {
            enum_gen.new_variant(&format!("{}_{}", previous, count));
        } else {
            enum_gen.new_variant(previous);
        }
        }

    Ok(())
    }

    Ok(())
}

unsafe fn name(object: *const Object) -> Result<&'static str, Error> {
    Ok((*object).name().ok_or(Error::NullName(object))?)
}

// fn write_structures(path: &mut PathBuf, structures: &[Structure]) -> Result<(), Error> {
//     let mut module = StagingFile::from(path, "mod.rs")?;

//     for s in structures {
//         let import = format!(
//             "mod {name};\n\
//             pub use {name}::{name};",
//             name = s.name
//         );

//         module.scope.raw(&import);

//         let mut struct_file = StagingFile::from(path, &format!("{}.rs", s.name))?;

//         struct_file.scope.import("crate", "sdk");

//         if !s.super_class.is_null() {
//             struct_file.scope.raw("use std::ops::{Deref, DerefMut};");
//         }

//         let struct_gen = struct_file
//             .scope
//             .new_struct(s.name)
//             .vis("pub")
//             .repr("C");

//         for member in &s.members {
//             let ty: Cow<str> = match member.kind {
//                 MemberKind::Padding => format!("[u8; {}]", member.size).into(),
//                 MemberKind::Byte(enumeration) => if enumeration.is_null() {
//                     "u8".into()
//                 } else {
//                     unsafe { name(enumeration.cast())?.into() }
//                 }
//                 MemberKind::Bool => "u32".into(),
//                 MemberKind::Struct(structure) => unsafe { name(structure.cast())?.into() },
//                 MemberKind::Unknown => format!("UNK_{}", member.size).into(),
//             };

//             let name = format!("pub {}", member.name);
//             struct_gen.field(&name, ty.as_ref());
//         }

//         if !s.super_class.is_null() {
//             let mut deref_impl = Impl::new(s.name);

//             deref_impl
//                 .impl_trait("Deref")
//                 .associate_type("Target", unsafe { name(s.super_class.cast())? });
                
//             deref_impl.new_fn("deref")
//                 .arg_ref_self()
//                 .ret("&Self::Target")
//                 .line("&self.base");

//             struct_file.scope.push_impl(deref_impl);

//             let mut deref_impl = Impl::new(s.name);

//             deref_impl
//                 .impl_trait("DerefMut");
                
//             deref_impl.new_fn("deref_mut")
//                 .arg_mut_self()
//                 .ret("&mut Self::Target")
//                 .line("&mut self.base");

//             struct_file.scope.push_impl(deref_impl);
//         }
//     }

//     Ok(())
// }