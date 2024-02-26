use std::io::Bytes;

use base64::Engine;
use futures_util::StreamExt;
use gemini_client::{Error, GeminiClient, GeminiModel, HistoryItemImage, ModelInfo, StreamItem, UserMessage};
use teloxide::{
  dispatching::dialogue::{serializer::Json, ErasedStorage, SqliteStorage, Storage},
  net::Download,
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
  let msg_text = msg.text().map_or("", |v| v);
  let photo = msg.photo();
  if msg_text == "/reset" {
    dialogue.update(State::Start).await?;
    let result = bot
      .send_message(msg.chat.id, "----- Chat history is reset -----")
      .await;
    if result.is_err() {
      let err_str = format!("Error: {:?}", result.err().unwrap());
      bot.send_message(msg.chat.id, err_str).await?;
    }
    return Ok(());
  }
  if let State::History(history) = state {
    println!("history: {}", history);
    let m = bot.send_message(msg.chat.id, "I'm thinking...").await?;
    let mut client = GeminiClient::new(API_KEY, GeminiModel::Pro(ModelInfo::default()))
      .with_serialized_history(&history);
    let mut user_msg = UserMessage::new(msg_text);
    if photo.is_some() {
      let photo = photo.unwrap();
      if !photo.is_empty() {
        let photo = photo.get(photo.len() - 1).unwrap();
        let photo_file_id = photo.file.id.clone();
        bot.edit_message_text(msg.chat.id, m.id, format!("Start get image path... {}", photo_file_id)).await?;
        let photo_file_path = bot.get_file(photo_file_id).await?.path;
        bot.edit_message_text(msg.chat.id, m.id, format!("Downloading image... {}", photo_file_path)).await?;
        let stream = bot.download_file_stream(&photo_file_path);
        let s = stream.collect::<Vec<_>>().await;
        let mut bytes = vec![];
        for item in s {
          match item {
            Ok(b) => {
              bytes.append(&mut b.to_vec());
            },
            Err(e) => {
              let err_str = format!("Error: {:?}", e);
              bot.send_message(msg.chat.id, err_str).await?;
              return Ok(());
            }
          }
        }
        let img = image::guess_format(&bytes).unwrap();
        let img_mime_type = img.to_mime_type();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let b64_url = format!("data:{};base64,{}", img_mime_type, b64);
        user_msg = user_msg.with_image(HistoryItemImage::new(&b64_url));
      }
    }
    let rx_raw = client.send_message(user_msg).await;

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
