//! Produces a history-trimmed corpus fixture without exposing live compaction to clients.
use hitslop_core::store::{Mode, Store};
use std::path::PathBuf;

fn main() {
    let path = PathBuf::from(std::env::args_os().nth(1).expect("document path"));
    let store = Store::open(&path, Mode::Document).expect("own fixture");
    let mut document = store.document().expect("read fixture");
    let job = store.job(&mut document, true).expect("checkpoint fixture").expect("forced checkpoint");
    store.write(&job).expect("save fixture");
    store.close().expect("release fixture");
}
