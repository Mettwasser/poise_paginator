use {
    crate::{Error, cancellation_type::CancellationType, event::Event},
    poise::{
        CreateReply,
        serenity_prelude::{
            self, ChannelId, Color, ComponentInteraction, ComponentInteractionCollector,
            CreateEmbed, CreateInteractionResponse, CreateInteractionResponseFollowup,
            CreateInteractionResponseMessage, UserId, futures::StreamExt,
        },
    },
    std::{fmt::Display, sync::Arc, time::Duration},
    tokio::sync::mpsc::UnboundedSender,
};

pub use crate::view::View;

pub trait PaginationInfo {
    type PoiseData: Send + Sync + 'static;
    type PoiseError: Sized + Send + Sync + 'static + Display;

    type View: View<Self::PoiseData, Self::PoiseError> + Send + Sync + 'static;
}

/// A paginator function that allows users to navigate through a series of pages with a very fancy UI.
///
/// Note on the generator function:
///
/// It is called at the very beginning, when the paginator is created, and every time a button is pressed.
/// It's also called when the pagination is cancelled through user input (the cancel button) or due to a timeout, represented by the [CancellationType](crate::cancellation_type::CancellationType).
///
/// This function propagates the Context to the generator function, allowing you to access the context of the command.
/// It also allows you to pass some state to the generator function, which can be used to store additional information across pages. Note that this state is cloned for each page.
///
/// Due to how buttons are handled, the index cannot go out of bounds (that is below 0 or above the length of the pages).
///
/// # Arguments
/// * `ctx` - The context of the command.
/// * `generator` - A function that generates the embed for the current page.
/// * `length` - The total number of pages.
/// * `timeout` - The duration after which the pagination will be cancelled if no interaction occurs.
/// * `state` - A state that can be used to store additional information across pages, accessed through the generator.
pub async fn paginate<'a, P, Fut, S>(
    ctx: poise::ApplicationContext<'a, P::PoiseData, P::PoiseError>,
    generator: impl Fn(
        poise::ApplicationContext<'a, P::PoiseData, P::PoiseError>,
        usize,
        CancellationType,
        S,
    ) -> Fut,
    length: usize,
    timeout: Duration,
    state: S,
) -> Result<(), Error>
where
    P: PaginationInfo,
    S: Clone + Send + Sync + 'static,
    Fut: Future<Output = Result<CreateEmbed, Error>> + Send,
{
    let id = ctx.id();

    let mut current_idx: usize = 0;

    let ids = P::View::create_ids(ctx);

    let components = P::View::rerender_components(Arc::clone(&ids), current_idx, length, false);

    let first_embed = generator(
        ctx,
        current_idx,
        CancellationType::NotCancelled,
        state.clone(),
    )
    .await?;

    let msg = ctx
        .send(
            CreateReply::default()
                .embed(first_embed)
                .components(components),
        )
        .await?;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event<P::PoiseError>>();

    tokio::spawn(
        handle_button_presses::<P::PoiseData, P::PoiseError, P::View>(
            ctx.serenity_context().clone(),
            tx,
            id.to_string(),
            ctx.author().id,
            ctx.channel_id(),
            timeout,
            Arc::clone(&ids),
        ),
    );

    while let Some(event) = rx.recv().await {
        // This is a flag that is solely there for the "Jump to page" button.
        // It is a flag to indicate whether to use a followup response or not,
        // because the first one was used to create the modal.
        let mut interaction_already_responded = false;

        let interaction = match event {
            Event::ToStart(interaction) => {
                current_idx = 0;
                interaction
            }
            Event::Previous(interaction) => {
                current_idx = current_idx.saturating_sub(1);
                interaction
            }

            Event::Next(interaction) => {
                current_idx += 1;
                interaction
            }
            Event::ToEnd(interaction) => {
                current_idx = length - 1;
                interaction
            }

            Event::Jump(interaction, page) => {
                if page < length {
                    current_idx = page;
                    interaction_already_responded = true;
                    interaction
                } else {
                    send_error_embed(
                        ctx,
                        interaction,
                        format!("Page {} does not exist.", page + 1),
                    )
                    .await?;
                    continue;
                }
            }

            Event::CancelledByTimeout => {
                msg.edit(
                    ctx.into(),
                    CreateReply::default()
                        .embed(
                            generator(ctx, current_idx, CancellationType::Timeout, state.clone())
                                .await?,
                        )
                        .components(P::View::rerender_components(
                            Arc::clone(&ids),
                            current_idx,
                            length,
                            true,
                        )),
                )
                .await?;
                break;
            }

            Event::CancelledByUser(interaction) => {
                let reply = CreateInteractionResponseMessage::default()
                    .embed(
                        generator(ctx, current_idx, CancellationType::UserInput, state.clone())
                            .await?,
                    )
                    .components(P::View::rerender_components(
                        Arc::clone(&ids),
                        current_idx,
                        length,
                        true,
                    ));

                interaction
                    .create_response(ctx, CreateInteractionResponse::UpdateMessage(reply))
                    .await?;

                break;
            }

            Event::Error(interaction, e) => {
                send_error_embed(ctx, interaction, e).await?;
                continue;
            }
        };

        let embed = generator(
            ctx,
            current_idx,
            CancellationType::NotCancelled,
            state.clone(),
        )
        .await?;
        let components = P::View::rerender_components(Arc::clone(&ids), current_idx, length, false);

        match interaction_already_responded {
            true => {
                msg.edit(
                    ctx.into(),
                    CreateReply::default().embed(embed).components(components),
                )
                .await?;
            }
            false => {
                let reply = CreateInteractionResponseMessage::default()
                    .embed(embed)
                    .components(components);

                interaction
                    .create_response(ctx, CreateInteractionResponse::UpdateMessage(reply))
                    .await?;
            }
        };
    }

    Ok(())
}

async fn handle_button_presses<D, E, V: View<D, E>>(
    ctx: serenity_prelude::Context,
    tx: UnboundedSender<Event<E>>,
    id: String,
    author_id: UserId,
    channel_id: ChannelId,
    timeout: Duration,
    ids: Arc<[String]>,
) -> Result<(), Error> {
    let mut collector = ComponentInteractionCollector::new(&ctx)
        .author_id(author_id)
        .channel_id(channel_id)
        .timeout(timeout)
        .filter(move |interaction| interaction.data.custom_id.starts_with(&id))
        .stream();

    while let Some(press) = collector.next().await {
        V::on_button_press(ctx.clone(), press, tx.clone(), Arc::clone(&ids)).await?;
    }

    tx.send(Event::CancelledByTimeout).unwrap_or_default();
    Ok(())
}

async fn send_error_embed<D, E>(
    ctx: poise::ApplicationContext<'_, D, E>,
    interaction: ComponentInteraction,
    description: impl Display,
) -> Result<(), Error>
where
    D: Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    let embed = CreateEmbed::new()
        .title("Error")
        .description(description.to_string())
        .color(Color::RED);

    interaction
        .create_followup(
            ctx,
            CreateInteractionResponseFollowup::default()
                .embed(embed)
                .ephemeral(true),
        )
        .await?;

    Ok(())
}
