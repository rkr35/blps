use crate::{GLOBAL_NAMES, GLOBAL_OBJECTS};
use crate::bitfield::{self, Bitfields, PostAddInstruction};
use crate::game::{Array, ArrayProperty, BoolProperty, ByteProperty, cast, Class, ClassProperty, Const, Enum, FString, InterfaceProperty, MapProperty, NameIndex, Object, ObjectProperty, Property, ScriptDelegate, ScriptInterface, Struct, StructProperty};
use crate::TimeIt;

use std::borrow::Cow;
use std::cmp::Ordering;
use std::convert::TryFrom;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::iter;
use std::mem;
use std::ptr;

use codegen::{Scope, Struct as StructGen};
use log::{info, warn};
use thiserror::Error;

static mut CONSTANT: *const Class = ptr::null();
static mut ENUMERATION: *const Class = ptr::null();
static mut STRUCTURE: *const Class = ptr::null();
static mut FUNCTION: *const Class = ptr::null();

static mut ARRAY_PROPERTY: *const Class = ptr::null();
static mut BOOL_PROPERTY: *const Class = ptr::null();
static mut BYTE_PROPERTY: *const Class = ptr::null();
static mut CLASS_PROPERTY: *const Class = ptr::null();
static mut DELEGATE_PROPERTY: *const Class = ptr::null();
static mut FLOAT_PROPERTY: *const Class = ptr::null();
static mut INT_PROPERTY: *const Class = ptr::null();
static mut INTERFACE_PROPERTY: *const Class = ptr::null();
static mut MAP_PROPERTY: *const Class = ptr::null();
static mut NAME_PROPERTY: *const Class = ptr::null();
static mut OBJECT_PROPERTY: *const Class = ptr::null();
static mut STR_PROPERTY: *const Class = ptr::null();
static mut STRUCT_PROPERTY: *const Class = ptr::null();

#[derive(Error, Debug)]
pub enum Error {
    #[error("enum {0:?} has an unknown or ill-formed variant")]
    BadVariant(*const Enum),

    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("null inner array property for {0:?}")]
    NullArrayInner(*const ArrayProperty),

    #[error("null interface class for {0:?}")]
    NullInterfaceClass(*const InterfaceProperty),

    #[error("null meta class for {0:?}")]
    NullMetaClass(*const ClassProperty),

    #[error("null map key property for {0:?}")]
    NullMapKeyProperty(*const MapProperty),

    #[error("null map value property for {0:?}")]
    NullMapValueProperty(*const MapProperty),

    #[error("null name for {0:?}")]
    NullName(*const Object),

    #[error("null property class for {0:?}")]
    NullPropertyClass(*const ObjectProperty),

    #[error("null property struct for {0:?}")]
    NullPropertyStruct(*const StructProperty),

    #[error("property size mismatch of {1} bytes for {0:?}; info = {2:?}")]
    PropertySizeMismatch(*const Property, u32, PropertyInfo),

    #[error("unable to find static class for \"{0}\"")]
    StaticClassNotFound(&'static str),

    #[error("failed to convert OsString \"{0:?}\" to String")]
    StringConversion(OsString),

    #[error("unknown property type for {0:?}")]
    UnknownProperty(*const Property),
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
    
    ARRAY_PROPERTY = find("Class Core.ArrayProperty")?;
    BOOL_PROPERTY = find("Class Core.BoolProperty")?;
    BYTE_PROPERTY = find("Class Core.ByteProperty")?;
    CLASS_PROPERTY = find("Class Core.ClassProperty")?;
    DELEGATE_PROPERTY = find("Class Core.DelegateProperty")?;
    FLOAT_PROPERTY = find("Class Core.FloatProperty")?;
    INT_PROPERTY = find("Class Core.IntProperty")?;
    INTERFACE_PROPERTY = find("Class Core.InterfaceProperty")?;
    MAP_PROPERTY = find("Class Core.MapProperty")?;
    NAME_PROPERTY = find("Class Core.NameProperty")?;
    OBJECT_PROPERTY = find("Class Core.ObjectProperty")?;
    STR_PROPERTY = find("Class Core.StrProperty")?;
    STRUCT_PROPERTY = find("Class Core.StructProperty")?;

    Ok(())
}


fn add_crate_attributes(scope: &mut Scope) {
    scope.raw("#![allow(dead_code)]\n\
               #![allow(non_camel_case_types)]\n\
               #![allow(non_snake_case)]");
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

    let mut counts: HashMap<&str, usize> = HashMap::new();

    for variant in (*object).variants() {
        let variant = variant.ok_or(Error::BadVariant(object))?;

        let count = counts
            .entry(variant)
            .and_modify(|c| *c += 1)
            .or_default();

        if *count == 0 {
            enum_gen.new_variant(variant);
        } else {
            enum_gen.new_variant(&format!("{}_{}", variant, *count));
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
    let mut offset: u32 = 0;
    let super_class: *const Struct = (*structure).super_field.cast();
    let struct_gen = sdk
        .new_struct(name)
        .repr("C")
        .vis("pub");

    let super_class = if super_class.is_null() || ptr::eq(super_class, structure) {
        None
    } else {
        offset = (*super_class).property_size.into();

        let super_name = get_name(super_class.cast())?;
        struct_gen.field("base", super_name);
        Some(super_name)
    };

    let bitfields = add_fields(struct_gen, &mut offset, get_properties(structure))?;

    let structure_size = (*structure).property_size.into();

    if offset < structure_size {
        add_padding(struct_gen, offset, structure_size - offset);
    }

    bitfields.emit(sdk, name);

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
        .filter(|p| p.element_size > 0)
        .filter(|p| !p.is(STRUCTURE) && !p.is(CONSTANT) & !p.is(ENUMERATION) && !p.is(FUNCTION))
        .collect();

    properties.sort_by(|p, q| 
        p.offset.cmp(&q.offset).then_with(||
            if p.is(BOOL_PROPERTY) && q.is(BOOL_PROPERTY) {
                let p: &BoolProperty = cast(p);
                let q: &BoolProperty = cast(q);
                p.bitmask.cmp(&q.bitmask)
            } else {
                Ordering::Equal
            }
        )
    );

    properties
}

unsafe fn add_fields(struct_gen: &mut StructGen, offset: &mut u32, properties: Vec<&Property>) -> Result<Bitfields, Error> {
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

        let mut name = get_name(property as &Object)?;

        if property.is(BOOL_PROPERTY) {
            let property: &BoolProperty = cast(property);

            
            if bitfields.add(property.offset, name) == PostAddInstruction::Skip {
                continue;
            }
            
            name = bitfield::FIELD;
        }

        let field_name = {
            let count = counts.entry(name).and_modify(|c| *c += 1).or_default();

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

        struct_gen.field(&field_name, field_type.as_ref());

        *offset = property.offset + total_property_size;
    }

    Ok(bitfields)
}

unsafe fn add_padding(struct_gen: &mut StructGen, offset: u32, size: u32) {
    let name = format!("pad_at_{:#x}", offset);
    let typ = format!("[u8; {:#x}]", size);
    struct_gen.field(&name, typ);
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

#[derive(Debug)]
pub struct PropertyInfo {
    size: u32,
    field_type: Cow<'static, str>,
    comment: Cow<'static, str>,
}

impl PropertyInfo {
    fn new(size: u32, field_type: Cow<'static, str>) -> Self {
        Self { 
            size,
            field_type,
            comment: "".into(),
        }
    }
}

impl TryFrom<&Property> for PropertyInfo {
    type Error = Error;

    fn try_from(property: &Property) -> Result<Self, Self::Error> {
        #[allow(clippy::cast_possible_truncation)]
        fn size_of<T>() -> u32 {
            mem::size_of::<T>() as u32
        }

        macro_rules! simple {
            ($typ:ty) => {
                Self::new(size_of::<$typ>(), stringify!($typ).into())
            }
        }

        Ok(unsafe {
            if property.is(ARRAY_PROPERTY) {
                let property: &ArrayProperty = cast(property);

                if let Some(inner) = property.inner.as_ref() {
                    let inner = PropertyInfo::try_from(inner)?;
                    let typ = format!("Array<{}>", inner.field_type);
                    let mut info = Self::new(size_of::<Array<usize>>(), typ.into());
                    info.comment = inner.comment;
                    info
                } else {
                    return Err(Error::NullArrayInner(property));
                }
            } else if property.is(BOOL_PROPERTY) {
                // not "bool" because bool properties are u32 bitfields.
                simple!(u32) 
            } else if property.is(BYTE_PROPERTY) {
                let property: &ByteProperty = cast(property);

                if property.enumeration.is_null() {
                    simple!(u8)
                } else {
                    let typ = get_name(property.enumeration.cast())?;
                    Self::new(size_of::<u8>(), typ.into())
                }
            } else if property.is(CLASS_PROPERTY) {
                let property: &ClassProperty = cast(property);

                if property.meta_class.is_null() {
                    return Err(Error::NullMetaClass(property));
                }

                let name = get_name(property.meta_class.cast())?;
                let typ = format!("Option<&'static {}>", name);

                Self::new(size_of::<usize>(), typ.into())
            } else if property.is(DELEGATE_PROPERTY) {
                simple!(ScriptDelegate)
            } else if property.is(FLOAT_PROPERTY) {
                simple!(f32)
            } else if property.is(INT_PROPERTY) {
                simple!(i32)
            } else if property.is(INTERFACE_PROPERTY) {
                let property: &InterfaceProperty = cast(property);

                if property.class.is_null() {
                    return Err(Error::NullInterfaceClass(property));
                }

                let mut info = simple!(ScriptInterface);
                info.comment = get_name(property.class.cast())?.into();
                info
            } else if property.is(MAP_PROPERTY) {
                let property: &MapProperty = cast(property);

                if let Some(key) = property.key.as_ref() {
                    if let Some(value) = property.value.as_ref() {
                        const MAP_SIZE_BYTES: u32 = 20;

                        let key = PropertyInfo::try_from(key)?;
                        let value = PropertyInfo::try_from(value)?;
                        
                        let typ = format!("[u8; {}]", MAP_SIZE_BYTES);

                        let mut info = Self::new(MAP_SIZE_BYTES, typ.into());
                        info.comment = format!("Map<{}, {}>", key.field_type, value.field_type).into();
                        info
                    } else {
                        return Err(Error::NullMapValueProperty(property));
                    }
                } else {
                    return Err(Error::NullMapKeyProperty(property));
                }

            } else if property.is(NAME_PROPERTY) {
                simple!(NameIndex)
            } else if property.is(OBJECT_PROPERTY) {
                let property: &ObjectProperty = cast(property);
                
                if property.class.is_null() {
                    return Err(Error::NullPropertyClass(property));
                }

                let name = get_name(property.class.cast())?;
                let typ = format!("Option<&'static {}>", name);

                Self::new(size_of::<usize>(), typ.into())
            } else if property.is(STRUCT_PROPERTY) {
                let property: &StructProperty = cast(property);
                
                if property.inner_struct.is_null() {
                    return Err(Error::NullPropertyStruct(property));
                }

                let typ = get_name(property.inner_struct.cast())?;
                Self::new(property.element_size, typ.into())
            } else if property.is(STR_PROPERTY) { 
                simple!(FString)
            } else {
                return Err(Error::UnknownProperty(property))
            }
        })
    }
}