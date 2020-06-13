use crate::{GLOBAL_NAMES, GLOBAL_OBJECTS};
use crate::game::{BoolProperty, ByteProperty, Class, Const, Enum, Object, Property, Struct};
use crate::TimeIt;

use std::cmp::Ordering;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::iter;
use std::mem;
use std::ptr;

use codegen::{Impl, Scope, Struct as StructGen};
use log::{info, warn};
use thiserror::Error;

static mut CONSTANT: *const Class = ptr::null();
static mut ENUMERATION: *const Class = ptr::null();
static mut STRUCTURE: *const Class = ptr::null();
static mut FUNCTION: *const Class = ptr::null();
static mut BYTE_PROPERTY: *const Class = ptr::null();
static mut BOOL_PROPERTY: *const Class = ptr::null();

#[derive(Error, Debug)]
pub enum Error {
    #[error("enum {0:?} has an unknown or ill-formed variant")]
    BadVariant(*const Enum),

    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("null name for {0:?}")]
    NullName(*const Object),

    #[error("unable to find static class for \"{0}\"")]
    StaticClassNotFound(&'static str),

    #[error("failed to convert OsString \"{0:?}\" to String")]
    StringConversion(OsString),

    // #[error("unknown property type for {0:?}")]
    // UnknownProperty(*const Property),
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

    let mut sdk = File::create(SDK_PATH)?;
    let mut scope = Scope::new();

    add_crate_attributes(&mut scope);
    add_imports(&mut scope);

    for object in (*GLOBAL_OBJECTS).iter() {
        write_object(&mut scope, object)?;
    }

    writeln!(&mut sdk, "{}", scope.to_string())?;

    Ok(())
}

unsafe fn find_static_classes() -> Result<(), Error> {
    unsafe fn find(class: &'static str) -> Result<*const Class, Error> {
        Ok((*GLOBAL_OBJECTS)
                .find(class)
                .map(|o| o.cast())
                .ok_or(Error::StaticClassNotFound(class))?)
    }
    
    let _time = TimeIt::new("find static classes");

    CONSTANT = find("Class Core.Const")?;
    ENUMERATION = find("Class Core.Enum")?;
    STRUCTURE = find("Class Core.ScriptStruct")?;
    FUNCTION = find("Class Core.Function")?;
    BYTE_PROPERTY = find("Class Core.ByteProperty")?;
    BOOL_PROPERTY = find("Class Core.BoolProperty")?;

    Ok(())
}


fn add_crate_attributes(scope: &mut Scope) {
    scope.raw("#![allow(dead_code)]\n\
               #![allow(non_camel_case_types)]");
}

fn add_imports(scope: &mut Scope) {
    scope.raw("use std::ops::{Deref, DerefMut};");
}

unsafe fn write_object(sdk: &mut Scope, object: *const Object) -> Result<(), Error> {
    if (*object).is(CONSTANT) {
        write_constant(sdk, object)?;
    } else if (*object).is(ENUMERATION) {
        write_enumeration(sdk, object)?;
    } else if (*object).is(STRUCTURE) {
        write_structure(sdk, object)?;
    }
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

    sdk.raw(&format!("// {} = {}", get_name(object)?, value));
    Ok(())
}

unsafe fn get_name(object: *const Object) -> Result<&'static str, Error> {
    Ok((*object).name().ok_or(Error::NullName(object))?)
}

unsafe fn write_enumeration(sdk: &mut Scope, object: *const Object) -> Result<(), Error> {
    let name = get_name(object)?;

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

fn is_enum_duplicate(name: &str) -> bool {
    const DUPLICATES: [&str; 2] = ["ECompareObjectOutputLinkIds", "EFlightMode"];
    DUPLICATES.iter().any(|dup| name == *dup)
}

unsafe fn write_structure(sdk: &mut Scope, object: *const Object) -> Result<(), Error> {
    let name = get_name(object)?;

    if is_struct_duplicate(name) {
        warn!("Ignoring {} because multiple structs have this name.", name);
        return Ok(());
    }

    let structure: *const Struct = object.cast();
    let mut offset = 0;
    let super_class: *const Struct = (*structure).super_field.cast();
    let mut struct_gen = sdk
        .new_struct(name)
        .repr("C")
        .vis("pub");

    let super_class = if super_class.is_null() || ptr::eq(super_class, structure) {
        None
    } else {
        offset = (*super_class).property_size;

        let super_name = get_name(super_class.cast())?;
        struct_gen.field("base", super_name);
        Some(super_name)
    };

    add_fields(struct_gen, get_properties(structure))?;

    if let Some(super_class) = super_class {
        add_deref_impls(sdk, name, super_class);
    }
    
    Ok(())
}

fn is_struct_duplicate(name: &str) -> bool {
    const DUPLICATES: [&str; 3] = ["CheckpointRecord", "TerrainWeightedMaterial", "ProjectileBehaviorSequenceStateData"];
    DUPLICATES.iter().any(|dup| name == *dup)
}

unsafe fn get_properties(structure: *const Struct) -> Vec<&'static Property> {
    let properties = iter::successors(
        (*structure).children.cast::<Property>().as_ref(),
        |property| property.next.cast::<Property>().as_ref()
    );

    let mut properties: Vec<&Property> = properties
        .filter(|p| !p.is(STRUCTURE) && !p.is(CONSTANT) & !p.is(ENUMERATION) && !p.is(FUNCTION))
        .collect();

    properties.sort_by(|p, q| 
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

    properties
}

unsafe fn add_fields(struct_gen: &mut StructGen, properties: Vec<&Property>) -> Result<(), Error> {
    Ok(())
}

fn add_deref_impls(sdk: &mut Scope, derived_name: &str, base_name: &str) {
    sdk
        .new_impl(derived_name)
        .impl_trait("Deref")
        .associate_type("Target", base_name)
        .new_fn("deref")
        .arg_ref_self()
        .ret("&Self::Target")
        .line("&self.base");

    sdk
        .new_impl(derived_name)
        .impl_trait("DerefMut")
        .new_fn("deref_mut")
        .arg_mut_self()
        .ret("&mut Self::Target")
        .line("&mut self.base");
}