use {
    crate::{Data, Error},
    poise::{
        ApplicationContext,
        serenity_prelude::{
            self, ButtonStyle, ComponentInteraction, CreateActionRow, CreateButton, ReactionType,
        },
    },
    poise_paginator::{Event, View},
    std::sync::Arc,
    tokio::sync::mpsc::UnboundedSender,
};

pub struct SimpleView;

impl View<Data, Error> for SimpleView {
    fn create_ids(ctx: ApplicationContext<'_, Data, Error>) -> Arc<[String]> {
        [
            format!("{}_rewind", ctx.id()),
            format!("{}_counter", ctx.id()),
            format!("{}_forward", ctx.id()),
        ]
        .into()
    }

    fn rerender_components(
        ids: Arc<[String]>,
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

        vec![CreateActionRow::Buttons(vec![
            CreateButton::new(&ids[0])
                .emoji(ReactionType::Unicode("◀️".to_owned()))
                .style(ButtonStyle::Secondary)
                .disabled(left_disabled),
            CreateButton::new(&ids[1])
                .label(format!("{} / {}", current_idx + 1, length))
                .disabled(true),
            CreateButton::new(&ids[2])
                .emoji(ReactionType::Unicode("▶️".to_owned()))
                .style(ButtonStyle::Secondary)
                .disabled(right_disabled),
        ])]
    }

    async fn on_button_press(
        _ctx: serenity_prelude::Context,
        press: ComponentInteraction,
        tx: UnboundedSender<Event<Error>>,
        ids: Arc<[String]>,
    ) -> Result<(), Error> {
        match press.data.custom_id.as_str() {
            id if id == ids[0] => {
                // Rewind
                tx.send(Event::Previous(press)).unwrap_or_default();
            }
            id if id == ids[2] => {
                // Forward
                tx.send(Event::Next(press)).unwrap_or_default();
            }

            _ => unreachable!("Unexpected button ID: {}", press.data.custom_id),
        }

        Ok(())
    }
}
