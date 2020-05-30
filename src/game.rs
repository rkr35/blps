use crate::GLOBAL_NAMES;

use std::borrow::Cow;
use std::ffi::CStr;
use std::iter;
use std::ops::Deref;
use std::os::raw::c_char;
use std::slice;

pub type Objects = Array<*mut Object>;
pub type Names = Array<*const Name>;

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

#[repr(C)]
pub struct Name {
    pad0: [u8; 0x10],
    text: c_char,
}

impl Name {
    pub unsafe fn text(&self) -> Cow<str> {
        CStr::from_ptr(&self.text as *const c_char).to_string_lossy()
    }
}

#[repr(C)]
pub struct NameIndex {
    index: u32,
    number: u32,
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
    pad2: [u8; 0x4],
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

    pub unsafe fn name(&self) -> Option<Cow<str>> {
        let name = *(*GLOBAL_NAMES).get(self.name.index as usize)?;

        if name.is_null() {
            None
        } else {
            Some((*name).text())
        }
    }
}

#[repr(C)]
pub struct Field {
    object: Object,
    pub next: *mut Field,
}

#[repr(C)]
pub struct Struct {
    field: Field,
    pad0: [u8; 8],
    pub super_field: *mut Struct,
    pub children: *mut Field,
    pub property_size: u16,
    pad1: [u8; 0x3a],
}