use crate::game::{cast, BoolProperty, Class, Const, Enum, Object, Property, Struct};
use crate::TimeIt;
use crate::{GLOBAL_NAMES, GLOBAL_OBJECTS};

use std::cmp::Ordering;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::iter;
use std::ptr;

use codegen::{Field, Scope, Struct as StructGen, Type};
use log::info;
use thiserror::Error;

mod bitfield;
use bitfield::{Bitfields, PostAddInstruction};

mod helper;

mod property_info;
use property_info::{PropertyInfo, BOOL_PROPERTY};

static mut CLASS: *const Class = ptr::null();
static mut CONSTANT: *const Class = ptr::null();
static mut ENUMERATION: *const Class = ptr::null();
static mut STRUCTURE: *const Class = ptr::null();
static mut FUNCTION: *const Class = ptr::null();

#[derive(Error, Debug)]
pub enum Error {
    #[error("enum {0:?} has an unknown or ill-formed variant")]
    BadVariant(*const Enum),

    #[error("helper error: {0}")]
    Helper(#[from] helper::Error),

    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("property info error: {0}")]
    PropertyInfo(#[from] property_info::Error),

    #[error("property size mismatch of {1} bytes for {0:?}; info = {2:?}")]
    PropertySizeMismatch(*const Property, u32, PropertyInfo),

    #[error("failed to convert OsString \"{0:?}\" to String")]
    StringConversion(OsString),
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
    let _time = TimeIt::new("find static classes");

    CLASS = helper::find("Class Core.Class")?;
    CONSTANT = helper::find("Class Core.Const")?;
    ENUMERATION = helper::find("Class Core.Enum")?;
    FUNCTION = helper::find("Class Core.Function")?;
    STRUCTURE = helper::find("Class Core.ScriptStruct")?;

    property_info::find_static_classes()?;

    Ok(())
}

fn add_crate_attributes(scope: &mut Scope) {
    scope.raw(
        "#![allow(dead_code)]\n\
               #![allow(non_camel_case_types)]\n\
               #![allow(non_snake_case)]",
    );
}

fn add_imports(scope: &mut Scope) {
    scope.raw("use crate::game::{Array, FString, is_bit_set, NameIndex, ScriptDelegate, ScriptInterface, set_bit};\n\
               use std::ops::{Deref, DerefMut};");
}

unsafe fn write_object(sdk: &mut Scope, object: *const Object) -> Result<(), Error> {
    if (*object).is(CONSTANT) {
        write_constant(sdk, object)?;
    } else if (*object).is(ENUMERATION) {
        write_enumeration(sdk, object)?;
    } else if (*object).is(STRUCTURE) {
        write_structure(sdk, object)?;
    } else if (*object).is(CLASS) {
        write_class(sdk, object)?;
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

    sdk.raw(&format!("// {} = {}", helper::get_name(object)?, value));
    Ok(())
}

unsafe fn write_enumeration(sdk: &mut Scope, object: *const Object) -> Result<(), Error> {
    let name = helper::resolve_duplicate(object)?;

    if name.starts_with("Default__") {
        return Ok(());
    }

    let enum_gen = sdk.new_enum(&name).repr("u8").vis("pub");

    let object: *const Enum = object.cast();

    let mut counts: HashMap<&str, usize> = HashMap::new();

    for variant in (*object).variants() {
        let variant = variant.ok_or(Error::BadVariant(object))?;

        let count = counts.entry(variant).and_modify(|c| *c += 1).or_default();

        if *count == 0 {
            enum_gen.new_variant(variant);
        } else {
            enum_gen.new_variant(&format!("{}_{}", variant, *count));
        }
    }

    Ok(())
}

unsafe fn write_structure(sdk: &mut Scope, object: *const Object) -> Result<(), Error> {
    let name = helper::resolve_duplicate(object)?;

    let structure: *const Struct = object.cast();

    let mut offset: u32 = 0;

    let super_class: *const Struct = (*structure).super_field.cast();

    let struct_gen = sdk.new_struct(&name).repr("C").vis("pub");

    let super_class = if super_class.is_null() || ptr::eq(super_class, structure) {
        None
    } else {
        offset = (*super_class).property_size.into();
        let super_name = helper::get_name(super_class.cast())?;
        emit_field(struct_gen, "base", super_name, 0, offset);
        Some(super_name)
    };

    let structure_size = (*structure).property_size.into();
    
    {
        let doc;
        let full_name = helper::get_full_name(object)?;
        
        if super_class.is_some() {
            let relative_size = structure_size - offset;
            doc = format!("{}, {:#x} ({:#x} - {:#x})", full_name, relative_size, structure_size, offset);
        } else {
            doc = format!("{}, {:#x}", full_name, structure_size);
        }

        struct_gen.doc(&doc);
    }

    let properties = get_properties(structure, offset);
    let bitfields = add_fields(struct_gen, &mut offset, properties)?;


    if offset < structure_size {
        add_padding(struct_gen, offset, structure_size - offset);
    }

    bitfields.emit(sdk, &name);

    if let Some(super_class) = super_class {
        add_deref_impls(sdk, &name, super_class);
    }

    Ok(())
}

unsafe fn get_properties(structure: *const Struct, offset: u32) -> Vec<&'static Property> {
    let properties = iter::successors(
        (*structure).children.cast::<Property>().as_ref(),
        |property| property.next.cast::<Property>().as_ref(),
    );

    let mut properties: Vec<&Property> = properties
        .filter(|p| p.element_size > 0)
        .filter(|p| p.offset >= offset)
        .filter(|p| !p.is(STRUCTURE) && !p.is(CONSTANT) & !p.is(ENUMERATION) && !p.is(FUNCTION))
        .collect();

    properties.sort_by(|p, q| {
        p.offset.cmp(&q.offset).then_with(|| {
            if p.is(BOOL_PROPERTY) && q.is(BOOL_PROPERTY) {
                let p: &BoolProperty = cast(p);
                let q: &BoolProperty = cast(q);
                p.bitmask.cmp(&q.bitmask)
            } else {
                Ordering::Equal
            }
        })
    });

    properties
}

unsafe fn add_fields(
    struct_gen: &mut StructGen,
    offset: &mut u32,
    properties: Vec<&Property>,
) -> Result<Bitfields, Error> {
    let mut bitfields = Bitfields::new();
    let mut counts: HashMap<&str, usize> = HashMap::with_capacity(properties.len());

    for property in properties {
        if *offset < property.offset {
            add_padding(struct_gen, *offset, property.offset - *offset);
        }

        let info = PropertyInfo::try_from(property)?;

        let total_property_size = property.element_size * property.array_dim;
        let size_mismatch = total_property_size - info.size * property.array_dim;

        if size_mismatch > 0 {
            return Err(Error::PropertySizeMismatch(property, size_mismatch, info));
        }

        let mut name = helper::get_name(property as &Object)?;

        if property.is(BOOL_PROPERTY) {
            let property: &BoolProperty = cast(property);

            if bitfields.add(property.offset, name) == PostAddInstruction::Skip {
                continue;
            }

            name = bitfield::FIELD;
        }

        let field_name = {
            let count = counts.entry(name).and_modify(|c| *c += 1).or_default();

            let name = scrub_reserved_name(name);

            if *count == 0 {
                format!("pub {}", name)
            } else {
                format!("pub {}_{}", name, *count)
            }
        };

        let mut field_type = if info.comment.is_empty() {
            info.field_type
        } else {
            format!("{} /* {} */", info.field_type, info.comment).into()
        };

        if property.array_dim > 1 {
            field_type = format!("[{}; {}]", field_type, property.array_dim).into();
        }

        emit_field(struct_gen, &field_name, field_type.as_ref(), property.offset, total_property_size);

        *offset = property.offset + total_property_size;
    }

    Ok(bitfields)
}

fn emit_field<T: Into<Type>>(struct_gen: &mut StructGen, name: &str, typ: T, offset: u32, length: u32) {
    let mut field = Field::new(name, typ);

    let comment = format!("\n// {:#x}({:#x})", offset, length);
    field.annotation([comment.as_ref()].into());
    
    struct_gen.push_field(field);
}

fn scrub_reserved_name(name: &str) -> &str {
    match name {
        "mod" => "r#mod",
        name => name,
    }
}

unsafe fn add_padding(struct_gen: &mut StructGen, offset: u32, size: u32) {
    let name = format!("pad_at_{:#x}", offset);
    let typ = format!("[u8; {:#x}]", size);
    emit_field(struct_gen, &name, typ, offset, size);
}

fn add_deref_impls(sdk: &mut Scope, derived_name: &str, base_name: &str) {
    sdk.new_impl(derived_name)
        .impl_trait("Deref")
        .associate_type("Target", base_name)
        .new_fn("deref")
        .arg_ref_self()
        .ret("&Self::Target")
        .line("&self.base");

    sdk.new_impl(derived_name)
        .impl_trait("DerefMut")
        .new_fn("deref_mut")
        .arg_mut_self()
        .ret("&mut Self::Target")
        .line("&mut self.base");
}

unsafe fn write_class(sdk: &mut Scope, object: *const Object) -> Result<(), Error> {
    write_structure(sdk, object)
}
