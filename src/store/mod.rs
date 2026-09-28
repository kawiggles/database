pub mod value;
pub mod bptree;
pub mod pager;
pub mod schema;
pub mod catalog;

use std::{
    path::Path,
};

use crate::{
    errors::{ DbResult, UserErr},
    store::{
        bptree::BpTree, 
        pager::{
            DataPage, Page, PageId, Pager,
            page::{PAGE_CAPACITY, SLOT_POINTER_SIZE}
        },
        value::Value,
        catalog::Catalog,
    }
};

// TODO: move this to somewhere more convenient
pub const RID_SIZE: usize = 10; // usize + u16 when serialized
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rid {
    pub page: PageId,
    pub slot: u16,
}

// Buffer pool for database, holds cache?
pub struct Store {
    pub catalog: Catalog,
    pub pager: Pager,
}

// Next major work happens here, plan is volcano iterator, cause splosions
impl Store {
    pub fn start(filepath: &str) -> DbResult<Self> {
        let path = Path::new(filepath);

        if path.exists() {
            let mut pager = Pager::open(filepath)?;
            let mut catalog = Catalog::open(&mut pager)?;
            
            Ok(Self { catalog, pager })
        } else {
            let mut pager = Pager::new(filepath)?;
            let mut catalog = Catalog::init(&mut pager)?;

            Ok(Self { catalog, pager })
        }
    }

    // don't even know if this sort of function will still be used.
    pub fn get(&self, key: &str) -> DbResult<Value> {
        todo!()
    }

    pub fn put(&mut self, key: &str, val: Value) -> DbResult<Value> {
        todo!()
    }

    pub fn del(&mut self, key: &str) -> DbResult<Value> {
        todo!()
    }

    pub fn exit(&mut self) -> DbResult<()> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
}
