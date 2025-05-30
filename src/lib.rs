pub mod cancellation_type;
mod event;
pub mod paginator;

type Error = Box<dyn std::error::Error + Send + Sync>;
