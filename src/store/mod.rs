pub mod value;
pub mod bptree;
pub mod pager;
pub mod catalog;
pub mod relation;

use std::{
    path::Path,
};

use crate::{
    errors::{DbResult},
    store::{
        pager::{Pager},
        value::Value,
        catalog::Catalog,
    }
};

// Buffer pool for database, holds cache?
// Not here numbnutz
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
            let catalog = Catalog::open(&mut pager)?;
            
            Ok(Self { catalog, pager })
        } else {
            let mut pager = Pager::new(filepath)?;
            let catalog = Catalog::init(&mut pager)?;

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
        self.pager.close()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
}
