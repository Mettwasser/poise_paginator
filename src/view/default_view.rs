use {
    super::View,
    crate::{Error, event::Event},
    poise::{
        ApplicationContext,
        serenity_prelude::{
            self, ButtonStyle, ComponentInteraction, CreateActionRow, CreateButton,
            CreateInteractionResponse, CreateQuickModal, ReactionType,
        },
    },
    std::{sync::Arc, time::Duration},
    tokio::sync::mpsc::UnboundedSender,
};

pub struct DefaultView;

impl<D> View<D, Error> for DefaultView
where
    D: 'static,
{
    fn create_ids(ctx: ApplicationContext<'_, D, Error>) -> Arc<[String]> {
        [
            format!("{}_fast_rewind", ctx.id()),
            format!("{}_rewind", ctx.id()),
            format!("{}_counter", ctx.id()),
            format!("{}_forward", ctx.id()),
            format!("{}_fast_forward", ctx.id()),
            format!("{}_jump_to", ctx.id()),
            format!("{}_cancel", ctx.id()),
        ]
        .into()
    }

    fn rerender_components(
        ids: std::sync::Arc<[String]>,
        current_idx: usize,
        length: usize,
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

    async fn on_button_press(
        ctx: serenity_prelude::Context,
        press: ComponentInteraction,
        tx: UnboundedSender<Event<Error>>,
        ids: Arc<[String]>,
    ) -> Result<(), Error> {
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

        Ok(())
    }
}
