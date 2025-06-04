use poise::serenity_prelude::ComponentInteraction;

#[derive(Debug, Clone)]
pub enum Event<E> {
    ToStart(ComponentInteraction),
    Next(ComponentInteraction),
    Previous(ComponentInteraction),
    ToEnd(ComponentInteraction),
    Jump(ComponentInteraction, usize),
    CancelledByTimeout,
    CancelledByUser(ComponentInteraction),
    Error(ComponentInteraction, E),
}
