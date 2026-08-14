#![doc = include_str!("../README.md")]

extern crate alloc;

mod cache;
mod config;
mod css;
mod document_output;
mod document_parser;
mod document_type;
mod element;
mod element_output;
mod error;
mod http_client;
mod item_output;
mod metrics;
mod rate_limiter;
mod render;
mod request;
mod response;
mod robot_list;
mod sitemap;
mod timer;
mod web_validator;

pub use self::{
    cache::{FjallCache, GlobalCache, LocalCache, MemoryCache, MokaCache, SledCache},
    config::*,
    document_output::DocumentOutput,
    document_parser::DocumentParser,
    error::{Error, ItemError},
    http_client::{BareHttpClient, HttpClient, ReqwestHttpClient},
    metrics::Metrics,
    rate_limiter::RateLimiter,
    render::{RenderFormat, RenderOptions, render_document},
    timer::ClockTimer,
    web_validator::WebValidator,
};
