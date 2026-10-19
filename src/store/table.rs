use crate::{
    errors::{DbResult},
    store::{
        pager::{PageId, Pager, DataPage},
        bptree::BpTree,
        schema::Schema,
        value::Value,
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
    pub fn insert(&mut self, row: Vec<Option<Value>>, pager: &mut Pager) -> DbResult<()>{
        let key = self.schema.gen_row_keys(&row)?;
        let bytes = self.schema.encode(row)?;

        let mut data = pager.read::<DataPage>(self.active_data)?;
        // TODO: check if no free space and allocate new datapage if necessary
        let rid = data.insert(&bytes, pager)?;
        pager.write(data)?;

        if let Some(old) = self.tree.insert(&key, rid, pager)? {
            let mut old_data = pager.read::<DataPage>(old.page)?;
            old_data.delete(old.slot)?;
            pager.write(old_data)?;
        }

        Ok(())
    }

    pub fn remove(&mut self) {
    }

    pub fn scan(&self) {
    }

    pub fn get_row(&self) {
    }
}
