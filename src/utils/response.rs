use std::{io::Read, thread};
use struson::reader::simple::{SimpleJsonReader, ValueReader};
use tokio::sync::mpsc::channel;

pub enum ResponseItem {
  Text(String),
  End,
}

pub fn read_response(
  reader: impl Read + Send + 'static,
) -> tokio::sync::mpsc::Receiver<ResponseItem> {
  let (tx, rx) = channel::<ResponseItem>(10);

  thread::spawn(move || {
    let json_reader = SimpleJsonReader::new(reader);

    json_reader.read_array_items(|item| {
      item.read_object_owned_names(|key, val| {
        if key == "candidates" {
          val.read_array_items(|item| {
            item.read_object_owned_names(|key, val| {
              if key == "content" {
                val.read_object_owned_names(|key, val| {
                  if key == "parts" {
                    val.read_array_items(|item| {
                      item.read_object_owned_names(|key, val| {
                        if key == "text" {
                          let v = val.read_string()?;
                          tx.blocking_send(ResponseItem::Text(v))?;
                        }
                        Ok(())
                      })?;
                      Ok(())
                    })?;
                  }
                  Ok(())
                })?;
              }
              Ok(())
            })?;
            Ok(())
          })?;
        }
        Ok(())
      })?;
      Ok(())
    }).ok();

    tx.blocking_send(ResponseItem::End).unwrap();
  });
  rx
}
