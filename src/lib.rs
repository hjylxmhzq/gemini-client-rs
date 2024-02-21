use crossbeam::channel::unbounded;
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::prelude::*;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio_util::io::StreamReader;

use crate::utils::response::read_response;
pub mod utils;

pub struct RxReader {
  rx: crossbeam::channel::Receiver<StreamItem>,
  rx_out: crossbeam::channel::Receiver<StreamItem>,
  tx_out: crossbeam::channel::Sender<StreamItem>,
  buf: Vec<u8>,
}

impl RxReader {
  pub fn new(rx: crossbeam::channel::Receiver<StreamItem>) -> Self {
    let (tx_out, rx_out) = unbounded::<StreamItem>();
    Self {
      rx,
      tx_out,
      rx_out,
      buf: vec![],
    }
  }
  pub fn clone(&self) -> Self {
    Self::new(self.rx_out.clone())
  }
}

impl Read for RxReader {
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    let data = self.rx.recv();
    if data.is_err() {
      println!("RecvError: {:?}", data.err().unwrap());
      return Ok(0);
    }
    let data = data.unwrap();
    match data {
      StreamItem::Data(mut data) => {
        self.buf.append(&mut data);
        let self_buf_len = self.buf.len();
        let buf_len = buf.len();
        let min_len = std::cmp::min(self_buf_len, buf_len);
        let copy_data: Vec<u8> = self.buf.drain(0..min_len).collect();
        buf[..min_len].copy_from_slice(copy_data.as_slice());
        self.tx_out.send(StreamItem::Data(copy_data)).unwrap();
        Ok(min_len)
      }
      StreamItem::End => Ok(0),
      StreamItem::Error(err) => Err(err.into()),
    }
  }
}

pub enum StreamItem<T = Vec<u8>> {
  Data(T),
  End,
  Error(Error),
}

pub fn r2r<S: AsyncRead + Unpin + Send + 'static>(mut reader: S) -> RxReader {
  let (tx, rx) = unbounded::<StreamItem>();

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

  RxReader::new(rx)
}

#[derive(Debug)]
pub enum Error {
  FetchError(reqwest::Error),
  StdIOError(std::io::Error),
  Unknown(String),
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
      Error::Unknown(err) => std::io::Error::new(std::io::ErrorKind::Other, err),
    }
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeminiClient {
  api_key: String,
  history: ChatHistory,
  max_message_length: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum GeminiModel {
  Pro(ModelInfo),
  ProVision(ModelInfo),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelInfo {
  max_input_tokens: usize,
}

impl Default for ModelInfo {
  fn default() -> Self {
    Self {
      max_input_tokens: 12000,
    }
  }
}

impl GeminiModel {
  pub fn max_input_tokens(&self) -> usize {
    match self {
      GeminiModel::Pro(info) => info.max_input_tokens,
      GeminiModel::ProVision(info) => info.max_input_tokens,
    }
  }
}

impl From<GeminiModel> for String {
  fn from(value: GeminiModel) -> Self {
    match value {
      GeminiModel::Pro(_) => String::from("gemini-pro"),
      GeminiModel::ProVision(_) => String::from("gemini-pro-vision"),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatHistory {
  pub model: GeminiModel,
  items: Vec<HistoryItem>,
}

impl ChatHistory {
  pub fn new(model: GeminiModel) -> Self {
    Self {
      model,
      items: vec![],
    }
  }

  pub fn format(&self, max_len: usize) -> String {
    let skip = self.items.len().saturating_sub(max_len);
    let contents: Vec<serde_json::Value> = self
      .items
      .iter()
      .skip(skip)
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryItemImage {}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct CountTokensResponse {
  totalTokens: usize,
}

impl GeminiClient {
  pub fn new(api_key: &str, model: GeminiModel) -> Self {
    GeminiClient {
      api_key: api_key.to_owned(),
      history: ChatHistory {
        items: vec![],
        model,
      },
      max_message_length: 200,
    }
  }

  pub fn serialize_history(&self) -> String {
    serde_json::to_string(&self.history).unwrap()
  }

  pub fn with_serialized_history(mut self, data: &str) -> Self {
    let history = serde_json::from_str::<ChatHistory>(data).map_or_else(
      |_err| ChatHistory::new(GeminiModel::Pro(ModelInfo::default())),
      |v| v,
    );
    self.history = history;
    self
  }

  pub fn push_model_message(&mut self, text: &str) {
    let item = HistoryItem::new(HistoryItemRole::Model, text);
    self.history.items.push(item);
  }

  pub fn with_history(mut self, history: ChatHistory) -> Self {
    self.history = history;
    self
  }

  pub async fn send_message(
    &mut self,
    msg: UserMessage,
  ) -> Result<tokio::sync::mpsc::Receiver<StreamItem<String>>, Error> {
    let mut item = HistoryItem::new(HistoryItemRole::User, &msg.text);
    if let Some(image) = msg.image {
      item = item.with_image(image);
    }
    self.history.items.push(item);
    let rx = self.send_request().await?;
    Ok(rx)
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
  fn get_count_tokens_endpoint(&self) -> String {
    let model_name: String = self.history.model.clone().into();
    let api_key = &self.api_key;
    format!(
      "https://generativelanguage.googleapis.com/v1beta/models/{}:countTokens?key={}",
      model_name, api_key
    )
  }

  pub async fn count_tokens(&self, max_msg_len: usize) -> Result<usize, Error> {
    let client = reqwest::Client::new();
    let end_point = self.get_count_tokens_endpoint();
    let body = self.history.format(max_msg_len);
    let resp = client
      .post(&end_point)
      .header("content-type", "application/json")
      .body(body)
      .send()
      .await?;
    let r: CountTokensResponse = resp.json().await.unwrap();
    Ok(r.totalTokens)
  }

  async fn send_request(&self) -> Result<tokio::sync::mpsc::Receiver<StreamItem<String>>, Error> {
    let client = reqwest::Client::new();
    let end_point = self.get_endpoint(true);
    println!("end_point: {}", end_point);

    let max_input_tokens = self.history.model.max_input_tokens();

    let mut msg_len = self.max_message_length;
    loop {
      if msg_len < 1 {
        return Err(Error::Unknown(
          "Message length is too short, this may caused by too much content in message".to_owned(),
        ));
      }
      let tokens = self.count_tokens(msg_len).await?;
      if tokens < max_input_tokens {
        break;
      }
      msg_len = msg_len.saturating_sub(10);
    }

    let body = self.history.format(msg_len);
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
    let rx = read_response(sync_reader);
    Ok(rx)
  }
}
