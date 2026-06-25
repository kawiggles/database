use crate::store::PAGE_SIZE;

use std::io::Read;

#[derive(Debug)]
pub enum Message {
    CopyData {
        len: usize,
        bytes: [u8; PAGE_SIZE - 9],
    },
}

impl Message {
    pub fn decode<T: Read>(stream: &mut T) -> Self {
    }
}
