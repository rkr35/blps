use crate::game::{cast, BoolProperty, Class, Const, Enum, Function, Object, Property, Struct};
use crate::TimeIt;
use crate::{GLOBAL_NAMES, GLOBAL_OBJECTS};

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::ptr;

use codegen::{Block, Field, Impl, Scope, Struct as StructGen, Type};
use heck::CamelCase;
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

    #[error("unable to get the outer class for constant {0:?}")]
    ConstOuter(*const Object),

    #[error("helper error: {0}")]
    Helper(#[from] helper::Error),

    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("property info error: {0}")]
    PropertyInfo(#[from] property_info::Error),

    #[error("property size mismatch of {1} bytes for {0:?}; info = {2:?}")]
    PropertySizeMismatch(*const Property, i64, PropertyInfo),

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
    const SDK_PATH: &str = r"C:\Users\Royce\Desktop\repos\blps\src\hook\sdk.rs";

    let _time = TimeIt::new("sdk()");

    find_static_classes()?;

    let mut scope = Scope::new();

    add_crate_attributes(&mut scope);
    add_imports(&mut scope);

    for object in (*GLOBAL_OBJECTS).iter() {
        write_object(&mut scope, object)?;
    }

    fs::write(SDK_PATH, scope.to_string())?;

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
        "#![allow(bindings_with_variant_name)]\n\
         #![allow(clippy::doc_markdown)]\n\
         #![allow(dead_code)]\n\
         #![allow(non_camel_case_types)]\n\
         #![allow(non_snake_case)]",
    );
}

fn add_imports(scope: &mut Scope) {
    scope.raw(
        "use crate::GLOBAL_OBJECTS;\n\
         use crate::game::{self, Array, FString, NameIndex, ScriptDelegate, ScriptInterface};\n\
         use crate::hook::bitfield::{is_bit_set, set_bit};\n\
         use std::mem::MaybeUninit;\n\
         use std::ops::{Deref, DerefMut};",
    );
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

    let outer = (*object)
        .iter_outer()
        .nth(1)
        .ok_or(Error::ConstOuter(object))?;

    let outer = helper::get_name(outer)?;

    sdk.raw(&format!(
        "// {}_{} = {}",
        outer,
        helper::get_name(object)?,
        value
    ));
    Ok(())
}

unsafe fn write_enumeration(sdk: &mut Scope, object: *const Object) -> Result<(), Error> {
    impl Enum {
        pub unsafe fn variants(&self) -> impl Iterator<Item = Option<&str>> {
            self.variants.iter().map(|n| n.name())
        }
    }

    let object: *const Enum = object.cast();

    let mut variant_name_counts: HashMap<&str, u8> = HashMap::new();
    let mut common_prefix: Option<Vec<&str>> = None;

    let variants: Result<Vec<Cow<str>>, Error> = (*object)
        .variants()
        .map(|variant| {
            let variant = variant.ok_or(Error::BadVariant(object))?;

            if let Some(common_prefix) = common_prefix.as_mut() {
                // Shrink the common prefix to the number of components still matching.
                let num_components_matching = common_prefix
                    .iter()
                    .zip(variant.split('_'))
                    .take_while(|(cp, s)| *cp == s)
                    .count();

                common_prefix.truncate(num_components_matching);
            } else {
                // All of the first variant will be the common prefix.
                common_prefix = Some(variant.split('_').collect());
            }

            Ok(get_unique_name(&mut variant_name_counts, variant))
        })
        .collect();

    let variants = variants?;

    let common_prefix_len = if let Some(common_prefix) = common_prefix {
        // Get the total number of bytes that we need to skip the common
        // prefix for each variant name.
        let num_underscores = common_prefix.len();
        let len: usize = common_prefix.iter().map(|component| component.len()).sum();

        num_underscores + len
    } else {
        // If we haven't initialized the common prefix, then there are no
        // variants in the enum. We don't generate empty enums.
        return Ok(());
    };

    let name = helper::resolve_duplicate(object.cast())?;

    let enum_gen = sdk.new_enum(&name).repr("u8").vis("pub");

    for variant in variants {
        // Use the unstripped prefix form of the variant if the stripped form
        // is an invalid Rust identifier.
        let variant = variant
            .get(common_prefix_len..)
            .filter(|stripped| {
                let begins_with_number = stripped.as_bytes()[0].is_ascii_digit();
                let is_self = *stripped == "Self";

                !begins_with_number && !is_self
            })
            .map_or(variant.as_ref(), |stripped| {
                // Special case: Trim "Enum name + Max" to "Max".
                if stripped.starts_with(name.as_ref()) && stripped.ends_with("MAX") {
                    &stripped[name.len()..]
                } else {
                    stripped
                }
            })
            .to_camel_case();

        enum_gen.new_variant(&variant);
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
            doc = format!(
                "{}, {:#x} ({:#x} - {:#x})",
                full_name, relative_size, structure_size, offset
            );
        } else {
            doc = format!("{}, {:#x}", full_name, structure_size);
        }

        struct_gen.doc(&doc);
    }

    let properties = get_fields(structure, offset);
    let bitfields = add_fields(struct_gen, &mut offset, properties)?;

    if offset < structure_size {
        add_padding(struct_gen, offset, structure_size - offset);
    }

    bitfields.emit(sdk, &name);

    if let Some(super_class) = super_class {
        add_deref_impls(sdk, &name, super_class);
    } else if name == "Object" {
        add_object_deref_impl(sdk);
    }

    Ok(())
}

unsafe fn get_fields(structure: *const Struct, offset: u32) -> Vec<&'static Property> {
    let mut properties: Vec<&Property> = (*structure)
        .iter_children()
        .filter(|p| p.element_size > 0)
        .filter(|p| p.offset >= offset)
        .filter(|p| !p.is(STRUCTURE) && !p.is(CONSTANT) & !p.is(ENUMERATION) && !p.is(FUNCTION))
        .collect();

    properties.sort_by(|p, q| property_compare(p, q));

    properties
}

unsafe fn property_compare(p: &Property, q: &Property) -> Ordering {
    p.offset.cmp(&q.offset).then_with(|| {
        if p.is(BOOL_PROPERTY) && q.is(BOOL_PROPERTY) {
            let p: &BoolProperty = cast(p);
            let q: &BoolProperty = cast(q);
            p.bitmask.cmp(&q.bitmask)
        } else {
            Ordering::Equal
        }
    })
}

unsafe fn add_fields(
    struct_gen: &mut StructGen,
    offset: &mut u32,
    properties: Vec<&Property>,
) -> Result<Bitfields, Error> {
    let mut bitfields = Bitfields::new();

    let mut field_name_counts: HashMap<&str, u8> = HashMap::with_capacity(properties.len());

    for property in properties {
        if *offset < property.offset {
            add_padding(struct_gen, *offset, property.offset - *offset);
        }

        let info = PropertyInfo::try_from(property)?;

        let total_property_size = property.element_size * property.array_dim;
        
        let size_mismatch = i64::from(total_property_size) - i64::from(info.size * property.array_dim);

        if size_mismatch != 0 {
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

        let field_name = format!(
            "pub {}",
            get_unique_name(
                &mut field_name_counts,
                scrub_reserved_name(name)
            )
        );

        let mut field_type = info.into_typed_comment();

        if property.array_dim > 1 {
            field_type = format!("[{}; {}]", field_type, property.array_dim).into();
        }

        emit_field(
            struct_gen,
            &field_name,
            field_type.as_ref(),
            property.offset,
            total_property_size,
        );

        *offset = property.offset + total_property_size;
    }

    Ok(bitfields)
}

fn emit_field<T: Into<Type>>(
    struct_gen: &mut StructGen,
    name: &str,
    typ: T,
    offset: u32,
    length: u32,
) {
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

fn add_padding(struct_gen: &mut StructGen, offset: u32, size: u32) {
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

/// Add a `Deref` and `DerefMut` for `&[mut] sdk::Object` (generated) -> 
/// `&[mut] game::Object` (handwritten with helpful impls)
fn add_object_deref_impl(sdk: &mut Scope) {
    sdk.new_impl("Object")
        .impl_trait("Deref")
        .associate_type("Target", "game::Object")
        .new_fn("deref")
        .arg_ref_self()
        .ret("&Self::Target")
        .line("unsafe { &*(self as *const Self as *const Self::Target) }");

    sdk.new_impl("Object")
        .impl_trait("DerefMut")
        .new_fn("deref_mut")
        .arg_mut_self()
        .ret("&mut Self::Target")
        .line("unsafe { &mut *(self as *mut Self as *mut Self::Target) }");
}

unsafe fn write_class(sdk: &mut Scope, object: *const Object) -> Result<(), Error> {
    write_structure(sdk, object)?;
    add_methods(sdk, object.cast())?;
    Ok(())
}

unsafe fn add_methods(sdk: &mut Scope, class: *const Struct) -> Result<(), Error> {
    let name = helper::resolve_duplicate(class.cast())?;
    let impl_gen = sdk.new_impl(&name);

    let mut method_name_counts: HashMap<&str, u8> = HashMap::new();

    for method in get_methods(class) {
        add_method(impl_gen, &mut method_name_counts, method)?;
    }

    Ok(())
}

unsafe fn get_methods(class: *const Struct) -> impl Iterator<Item = &'static Function> {
    (*class)
        .iter_children()
        .filter(|p| p.is(FUNCTION))
        .map(|p| cast::<Function>(p))
}

#[derive(PartialEq, Eq)]
enum ParameterKind {
    Input,
    Output,
}

struct Parameter<'a> {
    property: &'a Property,
    kind: ParameterKind,
    name: Cow<'a, str>,
    typ: Cow<'a, str>,
}

#[derive(Default)]
struct Parameters<'a>(Vec<Parameter<'a>>);

impl<'a> TryFrom<&'a Function> for Parameters<'a> {
    type Error = Error;

    fn try_from(method: &Function) -> Result<Parameters, Self::Error> { unsafe {
        let parameters = method
            .iter_children()
            .filter(|p| p.element_size > 0);

        let mut ret = Parameters::default();

        let mut parameter_name_counts = HashMap::new();

        for parameter in parameters {
            let kind = if parameter.is_out_param() || parameter.is_return_param() {
                ParameterKind::Output
            } else if parameter.is_param() {
                ParameterKind::Input
            } else {
                continue;
            };

            let name = helper::get_name(parameter as &Object)?;
            let name = scrub_reserved_name(name);
            let name = get_unique_name(&mut parameter_name_counts, name);
            let mut typ = PropertyInfo::try_from(parameter)?.into_typed_comment();

            if typ == "u32" {
                // Special case: Apparently `BoolProperty` is "u32" in
                // structure/class definitions, but "bool" when in method
                // parameters.
                typ = "bool".into();
            }

            ret.0.push(Parameter { property: parameter, kind, name, typ });
        }

        ret.0.sort_by(|p, q| property_compare(p.property, q.property));

        Ok(ret)
    }}
}

unsafe fn add_method(impl_gen: &mut Impl, method_name_counts: &mut HashMap<&str, u8>, method: &Function) -> Result<(), Error> {
    let name = get_unique_name(
        method_name_counts,
        helper::get_name(method as &Object)?
    );

    let method_gen = impl_gen
        .new_fn(&name)
        .vis("pub unsafe")
        .line("static mut FUNCTION: Option<*mut game::Function> = None;\n")
        .arg_mut_self();

    let parameters = Parameters::try_from(method)?;
    
    let mut structure = Block::new("#[repr(C)]\nstruct Parameters");
    let mut structure_init = Block::new("let mut p = Parameters");
    let mut return_tuple = vec![];

    // TODO: ARRAY PARAMETERS: Parameters `p` such that `p.property.array_dim > 1`
    for parameter in parameters.0 {
        match parameter.kind {
            ParameterKind::Input => {
                method_gen.arg(&parameter.name, parameter.typ.as_ref());
                structure.line(format!("{}: {},", parameter.name, parameter.typ));
                structure_init.line(format!("{},", parameter.name));
            }

            ParameterKind::Output => {
                structure.line(format!("{}: MaybeUninit<{}>,", parameter.name, parameter.typ));
                structure_init.line(format!("{}: MaybeUninit::uninit(),", parameter.name));
                return_tuple.push((format!("p.{}.assume_init()", parameter.name), parameter.typ));
            }
        }
    }

    let mut if_block = Block::new("if let Some(function) = FUNCTION");
    
    structure.after("\n");
    if_block.push_block(structure);

    structure_init.after(";\n");
    if_block.push_block(structure_init);
    
    if_block.line("let old_flags = (*function).flags;");

    if method.is_native() {
        if_block.line("(*function).flags |= 0x400;");
    }

    if_block.line("self.process_event(function, &mut p as *mut Parameters as *mut _);");

    if_block.line("(*function).flags = old_flags;");

    if return_tuple.len() == 1 {
        let (ident, typ) = &return_tuple[0];
        if_block.line(format!("\nSome({})", ident));
        method_gen.ret(format!("Option<{}>", typ));
    } else if return_tuple.len() > 1 {
        let mut idents = String::from("\nSome((");
        let mut ret = String::from("Option<(");

        for (ident, typ) in &return_tuple {
            idents.push_str(ident);
            idents.push_str(", ");

            ret.push_str(typ);
            ret.push_str(", ");
        }

        // Remove trailing ", "
        idents.pop();
        idents.pop();

        ret.pop();
        ret.pop();

        idents.push_str("))");
        ret.push_str(")>");

        if_block.line(idents);
        method_gen.ret(ret);
    }

    if_block.after(" else");

    let mut else_block = Block::new("");
    
    else_block.line(format!(
        "FUNCTION = (*GLOBAL_OBJECTS).find_mut(\"{}\").map(|o| o.cast());",
        helper::get_full_name(method as &Object)?
    ));

    if !return_tuple.is_empty() {
        else_block.line("None");
    }
    
    method_gen.push_block(if_block);
    method_gen.push_block(else_block);

    Ok(())
}

fn get_unique_name<'a>(name_counts: &mut HashMap<&'a str, u8>, name: &'a str) -> Cow<'a, str> {
    let count = *name_counts
        .entry(name)
        .and_modify(|c| *c += 1)
        .or_default();

    if count == 0 {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(format!("{}_{}", name, count))
    }
}
