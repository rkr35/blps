use crate::GLOBAL_OBJECTS;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("unable to find \"{0}\"")]
    NotFound(&'static str),
}

pub struct CachedFunctionIndexes {
    pub post_render: u32,
    pub player_tick: u32,
    pub player_destroyed: u32,
}

impl CachedFunctionIndexes {
    pub unsafe fn new() -> Result<Self, Error> {
        Ok(Self {
            post_render: find("Function WillowGame.WillowGameViewportClient.PostRender")?,
            player_tick: find("Function WillowGame.WillowPlayerController.PlayerTick")?,
            player_destroyed: find("Function WillowGame.WillowPlayerController.Destroyed")?,
        })
    }
}

unsafe fn find(full_name: &'static str) -> Result<u32, Error> {
    (*GLOBAL_OBJECTS)
        .find(full_name)
        .map(|o| (*o).index)
        .ok_or(Error::NotFound(full_name))
}