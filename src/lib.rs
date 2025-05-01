//! The static website validator.

extern crate alloc;

mod cache;
mod context;
mod document_type;
mod element;
mod error;
mod http_client;
mod metrics;
mod render;
mod response;
mod success;
mod timer;
mod validation;

use self::cache::{MemoryCache, SledCache};
pub use self::error::Error;
use self::metrics::Metrics;
use self::timer::ClockTimer;
use self::{context::Context, validation::validate_link};
use alloc::sync::Arc;
use dirs::cache_dir;
use http_client::{CachedHttpClient, ReqwestHttpClient};
use rlimit::{Resource, getrlimit};
use std::env::temp_dir;
use tabled::{
    Table,
    settings::{Color, Style, themes::Colorization},
};
use tokio::{fs::create_dir_all, sync::mpsc::channel};

const INITIAL_REQUEST_CACHE_CAPACITY: usize = 1 << 16;
const JOB_CAPACITY: usize = 1 << 16;

/// Runs validation.
pub async fn validate(url: &str, cache: bool) -> Result<(), Error> {
    let (sender, mut receiver) = channel(JOB_CAPACITY);
    let db = if cache {
        let directory = cache_dir().unwrap_or_else(temp_dir).join("muffy");
        create_dir_all(&directory).await?;
        Some(sled::open(directory)?)
    } else {
        None
    };
    let context = Arc::new(Context::new(
        CachedHttpClient::new(
            ReqwestHttpClient::new(),
            ClockTimer::new(),
            if let Some(db) = &db {
                Box::new(SledCache::new(db.open_tree("responses")?))
            } else {
                Box::new(MemoryCache::new(INITIAL_REQUEST_CACHE_CAPACITY))
            },
            (getrlimit(Resource::NOFILE)?.0 / 2) as _,
        ),
        sender,
        url.into(),
    ));

    validate_link(context, url.into(), None).await?;
}
