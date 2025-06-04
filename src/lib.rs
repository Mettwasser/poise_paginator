mod cancellation_type;
pub mod custom_paginator;
mod event;
mod paginator;
mod view;

type Error = Box<dyn std::error::Error + Send + Sync>;

pub use cancellation_type::CancellationType;
pub use paginator::paginate;
