use gemini_client::{Error, GeminiClient, GeminiModel, ModelInfo, StreamItem, UserMessage};
use teloxide::{
  dispatching::dialogue::{serializer::Json, ErasedStorage, SqliteStorage, Storage},
  prelude::*,
  utils::command::BotCommands,
};

type MyDialogue = Dialogue<State, ErasedStorage<State>>;
type MyStorage = std::sync::Arc<ErasedStorage<State>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

const API_KEY: &str = "AIzaSyA0h7cfIsH8ltSmVR01mUNXgxeTBv2gp9k";

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub enum State {
  #[default]
  Start,
  GotNumber(i32),
  History(String),
}

/// These commands are supported:
#[derive(Clone, BotCommands)]
#[command(rename_rule = "lowercase")]
pub enum Command {
  /// Get your number.
  Get,
  /// Reset your number.
  Reset,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let bot = Bot::new("6919639454:AAGjVEuGF19C8NBqkLpK9I7WX5puXcsaYEQ");

  let storage: MyStorage = SqliteStorage::open("db.sqlite", Json)
    .await
    .unwrap()
    .erase();

  let handler = Update::filter_message()
    .enter_dialogue::<Message, ErasedStorage<State>, State>()
    .branch(dptree::case![State::Start].endpoint(init))
    .branch(dptree::case![State::History(c)].endpoint(chat));

  Dispatcher::builder(bot, handler)
    .dependencies(dptree::deps![storage])
    .enable_ctrlc_handler()
    .build()
    .dispatch()
    .await;

  Ok(())
}

async fn init(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
  println!("init");
  let client = GeminiClient::new(API_KEY, GeminiModel::Pro(ModelInfo::default()));
  dialogue
    .update(State::History(client.serialize_history()))
    .await?;
  chat(bot, dialogue, msg).await?;
  Ok(())
}

async fn chat(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
  let state = dialogue.get().await.unwrap().unwrap();
  let msg_text = msg.text().unwrap();
  if msg_text == "/reset" {
    dialogue.update(State::Start).await?;
    bot.send_message(msg.chat.id, "----- Chat history is reset -----").await?;
    return Ok(());
  }
  if let State::History(history) = state {
    println!("history: {}", history);
    let m = bot.send_message(msg.chat.id, "I'm thinking...").await?;
    let mut client = GeminiClient::new(API_KEY, GeminiModel::Pro(ModelInfo::default())).with_serialized_history(&history);
    let rx_raw = client
      .send_message(UserMessage::new(msg_text))
      .await;

    if rx_raw.is_err() {
      let err = rx_raw.err().unwrap();
      let err_str = format!("Error: {:?}", err);
      bot.send_message(msg.chat.id, err_str).await?;
      return Ok(());
    }
    let mut rx = rx_raw.unwrap();

    dialogue
      .update(State::History(client.serialize_history()))
      .await?;
    let mut resp_text = String::new();
    while let Some(item) = rx.recv().await {
      match item {
        StreamItem::Data(text) => {
          resp_text.push_str(&text);
          bot.edit_message_text(msg.chat.id, m.id, &resp_text).await?;
        }
        StreamItem::Error(err) => {
          let err_str = format!("Error: {:?}", err);
          bot.send_message(msg.chat.id, err_str).await?;
        }
        _ => {}
      }
    }
    client.push_model_message(&resp_text);
    dialogue
      .update(State::History(client.serialize_history()))
      .await?;
  }

  Ok(())
}
