use crate::GLOBAL_OBJECTS;
use crate::game::{Class, Object};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("cannot find module and submodule for {0:?}")]
    ModuleSubmodule(*const Object),

    #[error("null name for {0:?}")]
    NullName(*const Object),

    #[error("unable to find static class for \"{0}\"")]
    StaticClassNotFound(&'static str),
}

pub unsafe fn resolve_duplicate(object: *const Object, name: &str) -> Result<Option<String>, Error> {
    const DUPLICATES: [&str; 5] = [
        "ECompareObjectOutputLinkIds",
        "EFlightMode",
        "CheckpointRecord",
        "TerrainWeightedMaterial",
        "ProjectileBehaviorSequenceStateData"
    ];

    if DUPLICATES.iter().any(|dup| name == *dup) {
        let mut module = None;
        let mut submodule = None;

        for outer in (*object).iter_outer() {
            submodule = module;
            module = Some(outer);
        }

        let module = get_name(module.ok_or(Error::ModuleSubmodule(object))?)?;
        let submodule = get_name(submodule.ok_or(Error::ModuleSubmodule(object))?)?;

        Ok(Some(format!("{}_{}_{}", module, submodule, name)))
    } else {
        Ok(None)
    }
}

pub unsafe fn get_name(object: *const Object) -> Result<&'static str, Error> {
    Ok((*object).name().ok_or(Error::NullName(object))?)
}

pub unsafe fn find(class: &'static str) -> Result<*const Class, Error> {
    Ok((*GLOBAL_OBJECTS)
            .find(class)
            .map(|o| o.cast())
            .ok_or(Error::StaticClassNotFound(class))?)
}