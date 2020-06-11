use crate::GLOBAL_NAMES;

use std::ffi::{CStr, OsString, c_void};
use std::iter;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::os::raw::c_char;
use std::os::windows::ffi::OsStringExt;
use std::ptr;
use std::slice;

pub type Objects = Array<*mut Object>;
pub type Names = Array<*const Name>;

impl Objects {
    pub unsafe fn find(&self, full_name: &str) -> Option<*const Object> {
        self
            .iter()
            .find(|&o| (*o).full_name().map_or(false, |n| n == full_name))
            .map(|o| o as *const Object)
    }
}

#[repr(C)]
pub struct Array<T> {
    pub data: *mut T,
    pub count: u32,
    pub max: u32,
}

impl<T> Deref for Array<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe {
            slice::from_raw_parts(self.data, self.count as usize)
        }
    }
}

impl<T> Array<*const T> {
    pub fn iter(&self) -> impl Iterator<Item = *const T> + '_ {
        self.deref().iter().filter(|o| !o.is_null()).copied()
    }
}

impl<T> Array<*mut T> {
    pub fn iter(&self) -> impl Iterator<Item = *mut T> + '_ {
        self.deref().iter().filter(|o| !o.is_null()).copied()
    }
}

#[repr(C)]
pub struct Name {
    pub pad0: [u8; 0x10],
    pub text: c_char,
}

impl Name {
    pub unsafe fn text(&self) -> Option<&str> {
        CStr::from_ptr(&self.text as *const c_char).to_str().ok()
    }
}

#[repr(C)]
pub struct NameIndex {
    pub index: u32,
    pub number: u32,
}

impl NameIndex {
    pub unsafe fn name(&self) -> Option<&str> {
        let name = *(*GLOBAL_NAMES).get(self.index as usize)?;

        if name.is_null() {
            None
        } else {
            (*name).text()
        }
    }
}

#[repr(C)]
pub struct Object {
    pub vtable: usize,
    pub pad0: [u8; 0x1c],
    pub index: u32,
    pub pad1: [u8; 0x4],
    pub outer: *mut Object,
    pub name: NameIndex,
    pub class: *mut Class,
    pub archetype: *mut Object,
}

impl Object {
    pub unsafe fn full_name(&self) -> Option<String> {
        if self.class.is_null() {
            return None;
        }

        let outer_names: Option<Vec<_>> = self.iter_outer().map(|o| o.name()).collect();
        let mut outer_names = outer_names?;
        outer_names.reverse();
        let name = outer_names.join(".");

        let class = String::from((*self.class).field.object.name()?);

        Some(class + " " + &name)
    }

    pub unsafe fn iter_outer(&self) -> impl Iterator<Item = &Self> {
        iter::successors(Some(self), |current| current.outer.as_ref())
    }

    pub unsafe fn iter_class(&self) -> impl Iterator<Item = &Class> {
        iter::successors(self.class.as_ref(), |current| current.super_field
            .as_ref()
            .map(|field| mem::transmute::<&Field, &Class>(field)))
    }

    pub unsafe fn name(&self) -> Option<&str> {
        self.name.name()
    }

    pub unsafe fn is(&self, class: *const Class) -> bool {
        self.iter_class().any(|c| ptr::eq(c, class))
    }
}

#[repr(C)]
pub struct Field {
    pub object: Object,
    pub next: *mut Field,
}

impl Deref for Field {
    type Target = Object;

    fn deref(&self) -> &Self::Target {
        &self.object
    }
}

impl DerefMut for Field {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.object
    }
}

#[repr(C)]
pub struct Struct {
    pub field: Field,
    pub pad0: [u8; 8],
    pub super_field: *mut Field,
    pub children: *mut Field,
    pub property_size: u16,
    pub pad1: [u8; 0x2e],
}

impl Deref for Struct {
    type Target = Field;

    fn deref(&self) -> &Self::Target {
        &self.field
    }
}

impl DerefMut for Struct {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.field
    }
}

pub type FString = Array<u16>; // &[u16] -> OsString -> Cow<str>

impl FString {
    pub fn to_string(&self) -> OsString {
        OsString::from_wide(self)
    }
}

#[repr(C)]
pub struct Const {
    pub field: Field,
    pub value: FString,
}

impl Deref for Const {
    type Target = Field;

    fn deref(&self) -> &Self::Target {
        &self.field
    }
}

impl DerefMut for Const {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.field
    }
}

#[repr(C)]
pub struct Enum {
    pub field: Field,
    pub variants: Array<NameIndex>,
}

impl Enum {
    pub unsafe fn variants(&self) -> impl Iterator<Item = Option<&str>> {
        self.variants
            .iter()
            .map(|n| n.name())
    }
}

impl Deref for Enum {
    type Target = Field;

    fn deref(&self) -> &Self::Target {
        &self.field
    }
}

impl DerefMut for Enum {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.field
    }
}

#[repr(C)]
pub struct ScriptStruct {
    pub struct_base: Struct,
    pub pad0: [u8; 28],
}

impl Deref for ScriptStruct {
    type Target = Struct;

    fn deref(&self) -> &Self::Target {
        &self.struct_base
    }
}

impl DerefMut for ScriptStruct {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.struct_base
    }
}

#[repr(C)]
pub struct Function {
    pub struct_base: Struct,
    pub flags: u32,
    pub native: u16,
    pub rep_offset: u16,
    pub name_index: NameIndex,
    pub precedence: u8,
    pub num_params: u8,
    pub params_size: u16,
    pub return_value_offset: u16,
    pub pad0: [u8; 6],
    pub func: *mut c_void,
    pub pad1: [u8; 4],
}

impl Deref for Function {
    type Target = Struct;

    fn deref(&self) -> &Self::Target {
        &self.struct_base
    }
}

impl DerefMut for Function {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.struct_base
    }
}

#[repr(C)]
pub struct State {
    pub struct_base: Struct,
    pub pad0: [u8; 68],
}

impl Deref for State {
    type Target = Struct;

    fn deref(&self) -> &Self::Target {
        &self.struct_base
    }
}

impl DerefMut for State {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.struct_base
    }
}

#[repr(C)]
pub struct Class {
    pub struct_base: Struct,
    pub pad0: [u8; 268],
}

impl Deref for Class {
    type Target = Struct;

    fn deref(&self) -> &Self::Target {
        &self.struct_base
    }
}

impl DerefMut for Class {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.struct_base
    }
}

#[repr(C)]
pub struct Property {
    pub field: Field,
    pub array_dim: u32,
    pub element_size: u32,
    pub property_flags_0: u32,
    pub property_flags_1: u32,
    pub property_size: u16,
    pub pad0: [u8; 14],
    pub offset: u32,
    pub property_link_next: *mut Property,
    pub pad1: [u8; 12],
}

impl Deref for Property {
    type Target = Field;

    fn deref(&self) -> &Self::Target {
        &self.field
    }
}

impl DerefMut for Property {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.field
    }
}

#[repr(C)]
pub struct ByteProperty {
    pub property: Property,
    pub enumeration: *mut Enum,
}

impl Deref for ByteProperty {
    type Target = Property;

    fn deref(&self) -> &Self::Target {
        &self.property
    }
}

impl DerefMut for ByteProperty {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.property
    }
}

#[repr(C)]
pub struct BoolProperty {
    pub property: Property,
    pub bitmask: u32,
}

impl Deref for BoolProperty {
    type Target = Property;

    fn deref(&self) -> &Self::Target {
        &self.property
    }
}

impl DerefMut for BoolProperty {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.property
    }
}

#[repr(C)]
pub struct ObjectProperty {
    pub property: Property,
    pub property_class: *mut Class,
}

impl Deref for ObjectProperty {
    type Target = Property;

    fn deref(&self) -> &Self::Target {
        &self.property
    }
}

impl DerefMut for ObjectProperty {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.property
    }
}

#[repr(C)]
pub struct ClassProperty {
    pub object_property: ObjectProperty,
    pub meta_class: *mut Class,
}

impl Deref for ClassProperty {
    type Target = ObjectProperty;

    fn deref(&self) -> &Self::Target {
        &self.object_property
    }
}

impl DerefMut for ClassProperty {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.object_property
    }
}

#[repr(C)]
pub struct InterfaceProperty {
    pub property: Property,
    pub interface_class: *mut Class,
}

impl Deref for InterfaceProperty {
    type Target = Property;

    fn deref(&self) -> &Self::Target {
        &self.property
    }
}

impl DerefMut for InterfaceProperty {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.property
    }
}

#[repr(C)]
pub struct StructProperty {
    pub property: Property,
    pub inner_struct: *mut Struct,
}

impl Deref for StructProperty {
    type Target = Property;

    fn deref(&self) -> &Self::Target {
        &self.property
    }
}

impl DerefMut for StructProperty {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.property
    }
}

#[repr(C)]
pub struct ArrayProperty {
    pub property: Property,
    pub inner: *mut Property,
}

impl Deref for ArrayProperty {
    type Target = Property;

    fn deref(&self) -> &Self::Target {
        &self.property
    }
}

impl DerefMut for ArrayProperty {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.property
    }
}

#[repr(C)]
pub struct MapProperty {
    pub property: Property,
    pub key: *mut Property,
    pub value: *mut Property,
}

impl Deref for MapProperty {
    type Target = Property;

    fn deref(&self) -> &Self::Target {
        &self.property
    }
}

impl DerefMut for MapProperty {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.property
    }
}

#[repr(C)]
pub struct DelegateProperty {
    pub property: Property,
    pub function1: *mut Function,
    pub function2: *mut Function,
}

impl Deref for DelegateProperty {
    type Target = Property;

    fn deref(&self) -> &Self::Target {
        &self.property
    }
}

impl DerefMut for DelegateProperty {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.property
    }
}