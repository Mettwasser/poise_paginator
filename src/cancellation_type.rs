/// A type representing different states of cancellation for a paginator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CancellationType {
    /// The cancellation was triggered by a timeout.
    Timeout,

    /// The cancellation was triggered by the user pressing the cancel button.
    UserInput,

    /// The cancellation has not been triggered, the interaction is still ongoing.
    NotCancelled,
}
