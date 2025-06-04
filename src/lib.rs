mod cancellation_type;
mod custom_paginator;
mod event;
mod paginator;
mod view;

type Error = Box<dyn std::error::Error + Send + Sync>;

pub use cancellation_type::CancellationType;
pub use custom_paginator::{PaginationInfo, paginate as custom_paginate};
pub use event::Event;
pub use paginator::paginate;
pub use view::{View, default_view::DefaultView};
