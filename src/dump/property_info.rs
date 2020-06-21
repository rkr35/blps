use crate::GLOBAL_OBJECTS;
use crate::game::{Array, ArrayProperty, ByteProperty, cast, Class, ClassProperty, FString, InterfaceProperty, MapProperty, NameIndex, Object, ObjectProperty, Property, ScriptDelegate, ScriptInterface, StructProperty};

use std::borrow::Cow;
use std::convert::TryFrom;
use std::mem;
use std::ptr;

use thiserror::Error;

static mut ARRAY_PROPERTY: *const Class = ptr::null();
pub static mut BOOL_PROPERTY: *const Class = ptr::null();
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

    #[error("unable to find static class for \"{0}\"")]
    StaticClassNotFound(&'static str),

    #[error("unknown property type for {0:?}")]
    UnknownProperty(*const Property),
}

unsafe fn get_name(object: *const Object) -> Result<&'static str, Error> {
    Ok((*object).name().ok_or(Error::NullName(object))?)
}

pub unsafe fn find_static_classes() -> Result<(), Error> {
    unsafe fn find(class: &'static str) -> Result<*const Class, Error> {
        Ok((*GLOBAL_OBJECTS)
                .find(class)
                .map(|o| o.cast())
                .ok_or(Error::StaticClassNotFound(class))?)
    }

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

#[derive(Debug)]
pub struct PropertyInfo {
    pub size: u32,
    pub field_type: Cow<'static, str>,
    pub comment: Cow<'static, str>,
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
            } else if property.is(STR_PROPERTY) { 
                simple!(FString)
            } else if property.is(STRUCT_PROPERTY) {
                let property: &StructProperty = cast(property);
                
                if property.inner_struct.is_null() {
                    return Err(Error::NullPropertyStruct(property));
                }

                let typ = get_name(property.inner_struct.cast())?;
                Self::new(property.element_size, typ.into())
            } else {
                return Err(Error::UnknownProperty(property))
            }
        })
    }
}