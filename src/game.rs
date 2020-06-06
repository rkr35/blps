use crate::GLOBAL_NAMES;

use std::ffi::{CStr, OsString, c_void};
use std::iter;
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
    data: *mut T,
    count: u32,
    max: u32,
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
    pad0: [u8; 0x10],
    text: c_char,
}

impl Name {
    pub unsafe fn text(&self) -> Option<&str> {
        CStr::from_ptr(&self.text as *const c_char).to_str().ok()
    }
}

#[repr(C)]
pub struct NameIndex {
    index: u32,
    number: u32,
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
    vtable: usize,
    pad0: [u8; 0x1c],
    pub index: u32,
    pad1: [u8; 0x4],
    pub outer: *mut Object,
    name: NameIndex,
    class: *mut Struct,
    archetype: *mut Object,
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

    pub unsafe fn iter_class(&self) -> impl Iterator<Item = &Struct> {
        iter::successors(self.class.as_ref(), |current| current.super_field.as_ref())
    }

    pub unsafe fn name(&self) -> Option<&str> {
        self.name.name()
    }

    pub unsafe fn is(&self, class: *const Struct) -> bool {
        self.iter_class().any(|c| ptr::eq(c, class))
    }
}

#[repr(C)]
pub struct Field {
    object: Object,
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
    field: Field,
    pad0: [u8; 8],
    pub super_field: *mut Field,
    pub children: *mut Field,
    pub property_size: u16,
    pad1: [u8; 0x2e],
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
    field: Field,
    variants: Array<NameIndex>,
}

impl Enum {
    pub unsafe fn variants(&self) -> Option<Vec<&str>> {
        self.variants
            .iter()
            .map(|n| n.name())
            .collect()
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
    struct_base: Struct,
    pad0: [u8; 28],
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
    struct_base: Struct,
    flags: u32,
    native: u16,
    rep_offset: u16,
    name_index: NameIndex,
    precedence: u8,
    num_params: u8,
    params_size: u16,
    return_value_offset: u16,
    pad0: [u8; 6],
    func: Option<&'static c_void>,
    pad1: [u8; 4],
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
    struct_base: Struct,
    pad0: [u8; 68],
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
    struct_base: Struct,
    pad0: [u8; 268],
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
    array_dim: u32,
    element_size: u32,
    property_flags: u64,
    property_size: u16,
    pad0: [u8; 14],
    offset: u32,
    property_link_next: Option<&'static Property>,
    pad1: [u8; 12],
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
