use {
    crate::{Error, cancellation_type::CancellationType, event::Event},
    poise::{
        CreateReply,
        serenity_prelude::{
            self, ButtonStyle, ChannelId, Color, ComponentInteraction,
            ComponentInteractionCollector, CreateActionRow, CreateButton, CreateEmbed,
            CreateInteractionResponse, CreateInteractionResponseFollowup,
            CreateInteractionResponseMessage, CreateQuickModal, ReactionType, UserId,
            futures::StreamExt,
        },
    },
    std::{fmt::Display, sync::Arc, time::Duration},
    tokio::sync::mpsc::UnboundedSender,
};

fn get_paginate_components(
    current_idx: usize,
    length: usize,
    ids: Arc<[String]>,
    disable_all: bool,
) -> Vec<CreateActionRow> {
    let (left_disabled, right_disabled) = match (disable_all, current_idx, length) {
        (true, ..) => (true, true),
        (false, 0, _) => (true, false),
        (false, idx, len) if idx == len - 1 => (false, true),
        (false, ..) => (false, false),
    };

    vec![
        CreateActionRow::Buttons(vec![
            CreateButton::new(&ids[0])
                .emoji(ReactionType::Unicode("⏪".to_owned()))
                .style(ButtonStyle::Success)
                .disabled(left_disabled),
            CreateButton::new(&ids[1])
                .emoji(ReactionType::Unicode("◀️".to_owned()))
                .style(ButtonStyle::Secondary)
                .disabled(left_disabled),
            CreateButton::new(&ids[2])
                .label(format!("{} / {}", current_idx + 1, length))
                .disabled(true),
            CreateButton::new(&ids[3])
                .emoji(ReactionType::Unicode("▶️".to_owned()))
                .style(ButtonStyle::Secondary)
                .disabled(right_disabled),
            CreateButton::new(&ids[4])
                .emoji(ReactionType::Unicode("⏩".to_owned()))
                .style(ButtonStyle::Success)
                .disabled(right_disabled),
        ]),
        CreateActionRow::Buttons(vec![
            CreateButton::new(&ids[5])
                .style(ButtonStyle::Primary)
                .label("Jump to page")
                .disabled(disable_all),
            CreateButton::new(&ids[6])
                .style(ButtonStyle::Danger)
                .label("Cancel")
                .disabled(disable_all),
        ]),
    ]
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
pub async fn paginate<'a, Gen, Fut, S, D, E>(
    ctx: poise::ApplicationContext<'a, D, E>,
    generator: Gen,
    length: usize,
    timeout: Duration,
    state: S,
) -> Result<(), Error>
where
    S: Clone + Send + Sync + 'static,
    Fut: Future<Output = Result<CreateEmbed, Error>> + Send,
    Gen: Fn(poise::ApplicationContext<'a, D, E>, usize, CancellationType, S) -> Fut,
    D: Send + Sync + 'static + Sized,
    E: Send + Sync + 'static + Sized,
{
    let id = ctx.id();

    let mut current_idx: usize = 0;

    let ids: Arc<[String]> = [
        format!("{}_fast_rewind", ctx.id()),
        format!("{}_rewind", ctx.id()),
        format!("{}_counter", ctx.id()),
        format!("{}_forward", ctx.id()),
        format!("{}_fast_forward", ctx.id()),
        format!("{}_jump_to", ctx.id()),
        format!("{}_cancel", ctx.id()),
    ]
    .into();

    let components = get_paginate_components(current_idx, length, Arc::clone(&ids), false);

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

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event<Error>>();

    tokio::spawn(handle_button_presses(
        ctx.serenity_context().clone(),
        tx,
        id.to_string(),
        ctx.author().id,
        ctx.channel_id(),
        timeout,
        Arc::clone(&ids),
    ));

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
                        .components(get_paginate_components(
                            current_idx,
                            length,
                            Arc::clone(&ids),
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
                    .components(get_paginate_components(
                        current_idx,
                        length,
                        Arc::clone(&ids),
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
        let components = get_paginate_components(current_idx, length, Arc::clone(&ids), false);

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

async fn handle_button_presses(
    ctx: serenity_prelude::Context,
    tx: UnboundedSender<Event<Error>>,
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
        handle_button_press(ctx.clone(), press, tx.clone(), Arc::clone(&ids)).await;
    }

    tx.send(Event::CancelledByTimeout).unwrap_or_default();
    Ok(())
}

async fn handle_button_press(
    ctx: serenity_prelude::Context,
    press: ComponentInteraction,
    tx: UnboundedSender<Event<Error>>,
    ids: Arc<[String]>,
) {
    match press.data.custom_id.as_str() {
        id if id == ids[0] => {
            // Fast rewind
            tx.send(Event::ToStart(press)).unwrap_or_default();
        }
        id if id == ids[1] => {
            // Rewind
            tx.send(Event::Previous(press)).unwrap_or_default();
        }
        id if id == ids[3] => {
            // Forward
            tx.send(Event::Next(press)).unwrap_or_default();
        }
        id if id == ids[4] => {
            // Fast forward
            tx.send(Event::ToEnd(press)).unwrap_or_default();
        }
        id if id == ids[5] => {
            // Jump to page

            let modal = CreateQuickModal::new("Jump to Page")
                .timeout(Duration::from_secs(30))
                .short_field("Page Number");

            let tx = tx.clone();

            tokio::spawn(async move {
                let response = press.quick_modal(&ctx, modal).await;

                let event = match response {
                    Ok(Some(response)) => {
                        let number = response.inputs[0].parse::<usize>();

                        let event = match number {
                            Ok(num) => Event::Jump(press, num - 1),
                            Err(e) => Event::Error(press, Error::from(e)),
                        };

                        response
                            .interaction
                            .create_response(ctx, CreateInteractionResponse::Acknowledge)
                            .await
                            .ok();

                        event
                    }
                    Ok(None) => return,
                    Err(e) => Event::Error(press, e.into()),
                };

                tx.send(event).unwrap_or_default();
            });
        }

        id if id == ids[6] => {
            // Cancel
            tx.send(Event::CancelledByUser(press)).unwrap_or_default();
        }

        _ => unreachable!("Unexpected button ID: {}", press.data.custom_id),
    }
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
