use crate::game::{Function, Object};
use crate::hook::sdk::{Canvas, WillowPlayerController};

use super::CACHED_FUNCTION_INDEXES;

use std::ffi::c_void;
use std::ptr;

use log::info;

mod yank;
use yank::Yank;

pub static mut CONTROLLER: *mut WillowPlayerController = ptr::null_mut();

pub unsafe fn process_event(
    this: *mut Object,
    method: *mut Function,
    parameters: *mut c_void,
    _return_value: *mut c_void,
) {
    let indexes = CACHED_FUNCTION_INDEXES.yank_ref();
    let method_index = (*method).index;

    if method_index == indexes.post_render {
        my_post_render(parameters.cast());
    } else if method_index == indexes.player_tick {
        my_player_tick(this.cast());
    } else if method_index == indexes.player_destroyed {
        my_player_destroyed();
    } else {
        // print_event(this, method);
    }
}

unsafe fn my_post_render(canvas: *mut *mut Canvas) {
    let canvas = *canvas;
    (*canvas).SetPos(200.0, 200.0, 0.0);
    (*canvas).DrawBox(200.0, 200.0);
}

unsafe fn my_player_tick(my_controller: *mut WillowPlayerController) {
    if CONTROLLER.is_null() {
        CONTROLLER = my_controller;
        info!("Set CONTROLLER.");
    }
}

unsafe fn my_player_destroyed() {
    CONTROLLER = ptr::null_mut();
    info!("Destroyed CONTROLLER.");
}

fn _print_event(object: &Object, method: &Function) {
    if let Some(object) = unsafe { object.full_name() } {
        if let Some(method) = unsafe { method.full_name() } {
            info!("{} called {}", object, method);
        }
    }
}
