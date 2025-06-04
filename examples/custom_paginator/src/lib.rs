use poise::{
    Command, FrameworkError,
    serenity_prelude::{ClientBuilder, FutureExt, GatewayIntents},
};

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::ApplicationContext<'a, Data, Error>;

pub mod view;

#[derive(Debug, Clone, Default)]
pub struct Data;

pub trait ApplyIf: Sized {
    fn apply_if<F, T>(self, condition: bool, f: F) -> Self
    where
        F: FnOnce(Self) -> T,
        T: Into<Self>;
}

impl<T> ApplyIf for T {
    fn apply_if<F, U>(self, condition: bool, f: F) -> Self
    where
        F: FnOnce(Self) -> U,
        U: Into<Self>,
    {
        if condition { f(self).into() } else { self }
    }
}

pub async fn run(commands: Vec<Command<Data, Error>>) -> Result<(), Error> {
    dotenv::dotenv().unwrap();
    tracing_subscriber::fmt()
        .pretty()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Add a .env file with the DISCORD_TOKEN variable set to your bot token
    let token = std::env::var("BOT_TOKEN").expect("missing DISCORD_TOKEN environment variable");
    let intents = GatewayIntents::privileged().difference(GatewayIntents::MESSAGE_CONTENT);

    let framework = poise::Framework::<Data, _>::builder()
        .options(poise::FrameworkOptions {
            commands,
            on_error: |err: FrameworkError<'_, Data, Error>| handle_error(err).boxed(),
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                Ok(Data)
            }
            .boxed()
        })
        .build();

    let mut client = ClientBuilder::new(token, intents)
        .framework(framework)
        .await?;

    client.start().await?;

    Ok(())
}

async fn handle_error(err: FrameworkError<'_, Data, Error>) {
    tracing::error!("Error: {:?}", err);
}
