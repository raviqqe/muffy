#![doc = include_str!("../README.md")]

extern crate alloc;

mod cache;
mod context;
mod document_output;
mod document_type;
mod element;
mod element_output;
mod error;
mod http_client;
mod metrics;
mod render;
mod response;
mod success;
mod timer;
mod validation;

use self::cache::{MemoryCache, SledCache};
pub use self::document_output::DocumentOutput;
pub use self::error::Error;
pub use self::metrics::Metrics;
pub use self::render::{RenderFormat, RenderOptions, render_document};
use self::timer::ClockTimer;
use self::{context::Context, validation::validate_link};
use alloc::sync::Arc;
use dirs::cache_dir;
use futures::{Stream, StreamExt};
use http_client::{CachedHttpClient, ReqwestHttpClient};
use rlimit::{Resource, getrlimit};
use std::env::temp_dir;
use tokio::{fs::create_dir_all, sync::mpsc::channel};
use tokio_stream::wrappers::ReceiverStream;

const DATABASE_NAME: &str = "muffy";
const RESPONSE_NAMESPACE: &str = "responses";

const INITIAL_REQUEST_CACHE_CAPACITY: usize = 1 << 16;
const JOB_CAPACITY: usize = 1 << 16;
const JOB_COMPLETION_BUFFER: usize = 1 << 8;

/// Validates websites recursively.
pub async fn validate(
    url: &str,
    cache: bool,
) -> Result<impl Stream<Item = Result<DocumentOutput, Error>>, Error> {
    let (sender, receiver) = channel(JOB_CAPACITY);
    let db = if cache {
        let directory = cache_dir().unwrap_or_else(temp_dir).join(DATABASE_NAME);
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
                Box::new(SledCache::new(db.open_tree(RESPONSE_NAMESPACE)?))
            } else {
                Box::new(MemoryCache::new(INITIAL_REQUEST_CACHE_CAPACITY))
            },
            (getrlimit(Resource::NOFILE)?.0 / 2) as _,
        ),
        sender,
        url.into(),
    ));

    validate_link(context, url.into(), None).await?;

    Ok(ReceiverStream::new(receiver)
        .map(Box::into_pin)
        .buffer_unordered(JOB_COMPLETION_BUFFER))
}
