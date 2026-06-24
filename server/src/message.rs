use crate::store::PAGE_SIZE;

#[derive(Debug)]
pub enum Message {
    CopyData {
        mes_type: u8,
        len: usize,
        bytes: [u8; PAGE_SIZE - 9],
    },
}
