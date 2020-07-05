use crate::game::{Function, Object};
use super::CACHED_FUNCTION_INDEXES;

use std::ffi::c_void;

use log::info;

mod yank;
use yank::Yank;

pub fn process_event(_this: &Object, method: &Function, _parameters: *mut c_void, _return_value: *mut c_void) {
    let indexes = unsafe { CACHED_FUNCTION_INDEXES.yank_ref() };
    
    if method.index == indexes.post_render {
        if let Some(full_name) = unsafe { method.full_name() } {
            info!("{}", full_name);
        }
    }
}
