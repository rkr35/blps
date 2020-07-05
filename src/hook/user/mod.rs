use crate::game::{Function, Object};

use std::ffi::c_void;

use log::info;

pub fn process_event(this: &Object, method: &Function, _parameters: *mut c_void, _return_value: *mut c_void) {
}
