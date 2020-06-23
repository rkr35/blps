use crate::wide_format;

use std::mem::{self, MaybeUninit};

use thiserror::Error;
use winapi::shared::minwindef::HMODULE;
use winapi::um::{
    libloaderapi::GetModuleHandleW,
    processthreadsapi::GetCurrentProcess,
    psapi::{GetModuleInformation, MODULEINFO},
};

#[derive(Error, Debug)]
pub enum ErrorKind {
    #[error("failed to get a handle to the module")]
    NullModule,

    #[error("failed to query module information")]
    GetModuleInformation,
}

#[derive(Error, Debug)]
#[error("\"{module}\" error: {kind}")]
pub struct Error {
    module: String,
    kind: ErrorKind,
}

impl Error {
    fn new(module: &str, kind: ErrorKind) -> Error {
        Error {
            module: String::from(module),
            kind,
        }
    }
}

#[derive(Debug)]
pub struct Module {
    module: HMODULE,
    pub name: String,
    pub base: usize,
    pub size: usize,
    pub end: usize,
}

impl Module {
    /// Construct a module from its PE name, e.g., "notepad.exe".
    pub fn from(name: &str) -> Result<Module, Error> {
        let (module, info) = unsafe {
            let module = Module::get_handle(name)?;

            let info = Module::get_info(module)
                .ok_or_else(|| Error::new(name, ErrorKind::GetModuleInformation))?;

            (module, info)
        };

        let base = info.lpBaseOfDll as usize;
        let size = info.SizeOfImage as usize;

        let module = Module {
            module,
            name: String::from(name),
            base,
            size,
            end: base + size,
        };

        Ok(module)
    }

    fn get_handle(name: &str) -> Result<HMODULE, Error> {
        let handle = unsafe {
            let wide_name = wide_format!("{}", name);
            GetModuleHandleW(wide_name.as_ptr())
        };

        if handle.is_null() {
            Err(Error::new(name, ErrorKind::NullModule))
        } else {
            Ok(handle)
        }
    }

    unsafe fn get_info(handle: HMODULE) -> Option<MODULEINFO> {
        let mut info = MaybeUninit::<MODULEINFO>::uninit();

        #[allow(clippy::cast_possible_truncation)]
        let size = mem::size_of::<MODULEINFO>() as u32;

        if GetModuleInformation(GetCurrentProcess(), handle, info.as_mut_ptr(), size) == 0 {
            None
        } else {
            Some(info.assume_init())
        }
    }

    pub fn _find_bytes(&self, find_me: &[u8]) -> Option<*const u8> {
        let memory = unsafe {
            let base = self.base as *const u8;
            std::slice::from_raw_parts(base, self.size)
        };

        memory
            .windows(find_me.len())
            .find(|window| *window == find_me)
            .map(|window| window.as_ptr())
    }

    pub fn _find_string(&self, string: &str) -> Option<*const u8> {
        self._find_bytes(string.as_bytes())
    }

    /// Find the first address in this module that matches `pattern`.
    ///
    /// Each byte in `pattern` can be `Some(u8)` or `None`, where the former
    /// looks for exactly the specified byte, and the latter is a wildcard byte
    /// that matches any byte.
    pub fn find_pattern(&self, pattern: &[Option<u8>]) -> Option<usize> {
        let memory = unsafe {
            let base = self.base as *const u8;
            std::slice::from_raw_parts(base, self.size)
        };

        memory
            .windows(pattern.len())
            .find(|window| {
                pattern
                    .iter()
                    .zip(window.iter())
                    .all(|(pattern_byte, module_byte)| {
                        pattern_byte.map_or(true, |p| p == *module_byte)
                    })
            })
            .map(|window| window.as_ptr() as usize)
    }
}
