use crate::dump::helper;
use crate::game::{Array, ArrayProperty, ByteProperty, cast, Class, ClassProperty, FString, InterfaceProperty, NameIndex, ObjectProperty, Property, ScriptDelegate, ScriptInterface, StructProperty};

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
    #[error("helper error: {0}")]
    Helper(#[from] helper::Error),

    #[error("null inner array property for {0:?}")]
    NullArrayInner(*const ArrayProperty),

    #[error("null interface class for {0:?}")]
    NullInterfaceClass(*const InterfaceProperty),

    #[error("null meta class for {0:?}")]
    NullMetaClass(*const ClassProperty),

    // #[error("null map key property for {0:?}")]
    // NullMapKeyProperty(*const MapProperty),

    // #[error("null map value property for {0:?}")]
    // NullMapValueProperty(*const MapProperty),

    #[error("null property class for {0:?}")]
    NullPropertyClass(*const ObjectProperty),

    #[error("null property struct for {0:?}")]
    NullPropertyStruct(*const StructProperty),

    #[error("unknown property type for {0:?}")]
    UnknownProperty(*const Property),
}

pub unsafe fn find_static_classes() -> Result<(), Error> {
    ARRAY_PROPERTY = helper::find("Class Core.ArrayProperty")?;
    BOOL_PROPERTY = helper::find("Class Core.BoolProperty")?;
    BYTE_PROPERTY = helper::find("Class Core.ByteProperty")?;
    CLASS_PROPERTY = helper::find("Class Core.ClassProperty")?;
    DELEGATE_PROPERTY = helper::find("Class Core.DelegateProperty")?;
    FLOAT_PROPERTY = helper::find("Class Core.FloatProperty")?;
    INT_PROPERTY = helper::find("Class Core.IntProperty")?;
    INTERFACE_PROPERTY = helper::find("Class Core.InterfaceProperty")?;
    MAP_PROPERTY = helper::find("Class Core.MapProperty")?;
    NAME_PROPERTY = helper::find("Class Core.NameProperty")?;
    OBJECT_PROPERTY = helper::find("Class Core.ObjectProperty")?;
    STR_PROPERTY = helper::find("Class Core.StrProperty")?;
    STRUCT_PROPERTY = helper::find("Class Core.StructProperty")?;

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
                    let typ = helper::resolve_duplicate(property.enumeration.cast())?;
                    Self::new(size_of::<u8>(), typ.into())
                }
            } else if property.is(CLASS_PROPERTY) {
                let property: &ClassProperty = cast(property);

                if property.meta_class.is_null() {
                    return Err(Error::NullMetaClass(property));
                }

                let name = helper::get_name(property.meta_class.cast())?;
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
                info.comment = helper::get_name(property.class.cast())?.into();
                info
            } else if property.is(MAP_PROPERTY) {
                const MAP_SIZE_BYTES: u32 = 60;
                let typ = format!("[u8; {}]", MAP_SIZE_BYTES);
                let mut info = Self::new(MAP_SIZE_BYTES, typ.into());
                info.comment = "Map".into();
                info
            } else if property.is(NAME_PROPERTY) {
                simple!(NameIndex)
            } else if property.is(OBJECT_PROPERTY) {
                let property: &ObjectProperty = cast(property);
                
                if property.class.is_null() {
                    return Err(Error::NullPropertyClass(property));
                }

                let name = helper::get_name(property.class.cast())?;
                let typ = format!("Option<&'static {}>", name);

                Self::new(size_of::<usize>(), typ.into())
            } else if property.is(STR_PROPERTY) { 
                simple!(FString)
            } else if property.is(STRUCT_PROPERTY) {
                let property: &StructProperty = cast(property);
                
                if property.inner_struct.is_null() {
                    return Err(Error::NullPropertyStruct(property));
                }

                let typ = helper::resolve_duplicate(property.inner_struct.cast())?;
                Self::new(property.element_size, typ.into())
            } else {
                return Err(Error::UnknownProperty(property))
            }
        })
    }
}