mod cancellation_type;
mod event;
mod paginator;

type Error = Box<dyn std::error::Error + Send + Sync>;

pub use cancellation_type::CancellationType;
pub use paginator::paginate;
