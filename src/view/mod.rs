pub(crate) mod default_view;

use {
    crate::{Error, event::Event},
    poise::{
        ApplicationContext,
        serenity_prelude::{self, ComponentInteraction, CreateActionRow},
    },
    std::sync::Arc,
    tokio::sync::mpsc::UnboundedSender,
};

pub trait View<D, E>: Send + Sync + 'static {
    fn create_ids(ctx: ApplicationContext<'_, D, E>) -> Arc<[String]>;

    fn rerender_components(
        ids: Arc<[String]>,
        current_idx: usize,
        length: usize,
        disable_all: bool,
    ) -> Vec<CreateActionRow>;

    fn on_button_press(
        ctx: serenity_prelude::Context,
        press: ComponentInteraction,
        tx: UnboundedSender<Event<E>>,
        ids: Arc<[String]>,
    ) -> impl Future<Output = Result<(), Error>> + Send + 'static;
}
