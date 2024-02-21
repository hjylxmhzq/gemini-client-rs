use std::{io::Read, thread};

use crossbeam::channel::unbounded;

use crate::{RxReader, StreamItem};

pub fn duplicate_reader<S: Read + Unpin + Send + 'static>(mut reader: S) -> (RxReader, RxReader) {
  let (tx, rx) = unbounded::<StreamItem>();
  let (tx2, rx2) = unbounded::<StreamItem>();
  let reader1 = RxReader::new(rx);
  let reader2 = RxReader::new(rx2);
  thread::spawn(move || {
    let mut buf = vec![0; 1024];
    loop {
      let n = reader.read(&mut buf).unwrap();
      if n == 0 {
        break;
      }
      tx.send(StreamItem::Data(buf[..n].to_vec())).unwrap();
      tx2.send(StreamItem::Data(buf[..n].to_vec())).unwrap();
    }
    tx.send(StreamItem::End).unwrap();
    tx2.send(StreamItem::End).unwrap();
  });
  (reader1, reader2)
}