#![doc = include_str!("../README.md")]

extern crate alloc;

mod cache;
mod config;
mod document_output;
mod document_type;
mod element;
mod element_output;
mod error;
mod html_parser;
mod http_client;
mod item_output;
mod metrics;
mod rate_limiter;
mod render;
mod request;
mod response;
mod timer;
mod web_validator;

pub use self::{
    cache::{Cache, FjallCache, MemoryCache, MokaCache, SledCache},
    config::*,
    document_output::DocumentOutput,
    error::Error,
    html_parser::HtmlParser,
    http_client::{BareHttpClient, HttpClient, ReqwestHttpClient},
    metrics::Metrics,
    render::{RenderFormat, RenderOptions, render_document},
    timer::ClockTimer,
    web_validator::WebValidator,
};
