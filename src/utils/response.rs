use std::{io::Read, thread};
use struson::reader::simple::{SimpleJsonReader, ValueReader};
use tokio::sync::mpsc::channel;
use crate::{Error, RxReader, StreamItem};
use super::reader::duplicate_reader;

pub fn read_response(reader: RxReader) -> tokio::sync::mpsc::Receiver<StreamItem<String>> {
  let (tx, rx) = channel::<StreamItem<String>>(10);

  thread::spawn(move || {
    let (r1, mut r2) = duplicate_reader(reader);
    let json_reader = SimpleJsonReader::new(r1);
    let mut has_content = false;

    let success = json_reader.read_array_items(|item| {
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
                          tx.blocking_send(StreamItem::Data(v))?;
                          has_content = true;
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
    });

    if success.is_ok() && has_content {
      tx.blocking_send(StreamItem::End).unwrap();
    } else {
      let mut buf = String::with_capacity(1024);
      r2.read_to_string(&mut buf).unwrap();
      tx.blocking_send(StreamItem::Error(Error::Unknown(buf))).unwrap();
    }
  });
  rx
}
