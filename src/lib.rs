use futures_util::TryStreamExt;
use serde_json::json;
use std::io::prelude::*;
use std::sync::mpsc::channel;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio_util::io::StreamReader;

use crate::utils::response::{read_response, ResponseItem};
mod utils;

pub struct RxReader {
  rx: std::sync::mpsc::Receiver<StreamItem>,
}

impl Read for RxReader {
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    let data = self.rx.recv().unwrap();
    match data {
      StreamItem::Data(data) => {
        let len = data.len();
        buf[..len].copy_from_slice(&data);
        Ok(len)
      }
      StreamItem::End => Ok(0),
      StreamItem::Error(err) => Err(err.into()),
    }
  }
}

enum StreamItem {
  Data(Vec<u8>),
  End,
  Error(Error),
}

pub fn r2r<S: AsyncRead + Unpin + Send + 'static>(mut reader: S) -> RxReader {
  let (tx, rx) = channel::<StreamItem>();

  tokio::spawn(async move {
    let mut buf = vec![0; 1024];
    loop {
      let n = reader.read(&mut buf).await.unwrap();
      if n == 0 {
        break;
      }

      tx.send(StreamItem::Data(buf[..n].to_vec())).unwrap();
    }
    tx.send(StreamItem::End).unwrap();
  });

  RxReader { rx }
}

#[derive(Debug)]
pub enum Error {
  FetchError(reqwest::Error),
  StdIOError(std::io::Error),
}

impl From<reqwest::Error> for Error {
  fn from(value: reqwest::Error) -> Self {
    Self::FetchError(value)
  }
}

impl From<std::io::Error> for Error {
  fn from(value: std::io::Error) -> Self {
    Self::StdIOError(value)
  }
}

impl From<Error> for std::io::Error {
  fn from(value: Error) -> Self {
    match value {
      Error::FetchError(err) => std::io::Error::new(std::io::ErrorKind::Other, err),
      Error::StdIOError(err) => err,
    }
  }
}

pub struct GeminiClient {
  api_key: String,
  history: ChatHistory,
}

#[derive(Debug, Clone)]
pub enum GeminiModel {
  Pro,
  ProVision,
}

impl From<GeminiModel> for String {
  fn from(value: GeminiModel) -> Self {
    match value {
      GeminiModel::Pro => String::from("gemini-pro"),
      GeminiModel::ProVision => String::from("gemini-pro-vision"),
    }
  }
}

pub struct ChatHistory {
  pub model: GeminiModel,
  items: Vec<HistoryItem>,
}

impl ChatHistory {
  pub fn format(&self) -> String {
    let contents: Vec<serde_json::Value> = self
      .items
      .iter()
      .map(|item| {
        let parts = if let Some(_) = item.image {
          json!({
            "text": item.text
          })
        } else {
          json!({
            "text": item.text
          })
        };

        let role: String = item.role.clone().into();

        json!({
          "parts": parts,
          "role": role
        })
      })
      .collect();

    let body = json!({
      "contents": contents,
    });

    body.to_string()
  }
}

#[derive(Clone)]
pub enum HistoryItemRole {
  User,
  Model,
}

impl From<HistoryItemRole> for String {
  fn from(value: HistoryItemRole) -> Self {
    match value {
      HistoryItemRole::Model => String::from("model"),
      HistoryItemRole::User => String::from("user"),
    }
  }
}

#[derive(Clone)]
pub struct HistoryItemImage {}

#[derive(Clone)]
pub struct HistoryItem {
  pub role: HistoryItemRole,
  pub text: String,
  pub image: Option<HistoryItemImage>,
}

impl HistoryItem {
  pub fn new(role: HistoryItemRole, text: &str) -> Self {
    Self {
      role,
      text: text.to_owned(),
      image: None,
    }
  }
  pub fn with_image(mut self, image: HistoryItemImage) -> Self {
    self.image = Some(image);
    self
  }
}

pub struct UserMessage {
  text: String,
  image: Option<HistoryItemImage>,
}

impl UserMessage {
  pub fn new(text: &str) -> Self {
    Self {
      text: text.to_owned(),
      image: None,
    }
  }
}

pub struct ChatResponse {}

impl GeminiClient {
  pub fn new(api_key: &str, model: GeminiModel) -> Self {
    GeminiClient {
      api_key: api_key.to_owned(),
      history: ChatHistory {
        items: vec![],
        model,
      },
    }
  }
  pub fn with_history(mut self, history: ChatHistory) -> Self {
    self.history = history;
    self
  }
  pub async fn send_message(&mut self, msg: UserMessage) -> Result<HistoryItem, Error> {
    let mut item = HistoryItem::new(HistoryItemRole::User, &msg.text);
    if let Some(image) = msg.image {
      item = item.with_image(image);
    }
    self.history.items.push(item);
    self.send_request().await
  }
  fn get_endpoint(&self, stream: bool) -> String {
    let model_name: String = self.history.model.clone().into();
    let api_key = &self.api_key;
    let method = if stream {
      "streamGenerateContent"
    } else {
      "generateContent"
    };
    format!(
      "https://generativelanguage.googleapis.com/v1beta/models/{}:{}?key={}",
      model_name, method, api_key
    )
  }
  async fn send_request(&self) -> Result<HistoryItem, Error> {
    let client = reqwest::Client::new();
    let end_point = self.get_endpoint(true);
    println!("end_point: {}", end_point);
    let body = self.history.format();
    let resp = client
      .post(&end_point)
      .header("content-type", "application/json")
      .body(body)
      .send()
      .await?;
    // let r = resp.text().await?;
    // println!("{}", r);

    let stream = resp.bytes_stream();
    let stream = stream.map_err(|err| {
      println!("err: {:?}", err);
      Error::FetchError(err)
    });
    let stream_reader = StreamReader::new(stream);
    let sync_reader = r2r(stream_reader);
    let mut rx = read_response(sync_reader);

    while let ResponseItem::Text(text) = rx.recv().await.unwrap() {
      println!("text: {:?}", text);
    }

    // let resp_json = resp.text().await?;
    // println!("{}", resp_json);
    let resp_msg = HistoryItem::new(HistoryItemRole::Model, "help me write some js code");
    Ok(resp_msg)
  }
}
