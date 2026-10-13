use crate::{
    store::{
        pager::{PageId},
        bptree::BpTree,
        schema::Schema,
    },
};
pub const RID_SIZE: usize = 10; // usize + u16 when serialized
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rid {
    pub page: PageId,
    pub slot: u16,
}

#[derive(Debug, PartialEq)]
pub struct Table {
    pub tid: usize,
    pub name: String,
    pub tree: BpTree,
    pub active_data: PageId,
    pub schema: Schema,
}

impl Table {
    pub fn insert(&mut self, Vec<Value>) {
    }

    pub fn remove(&mut self) {
    }

    pub fn scan(&self) {
    }

    pub fn get_row(&self) {
    }
}
