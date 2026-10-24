use crate::{
    errors::{DbResult, StoreResult},
    store::{
        bptree::BpTree,
        pager::{DataPage, PageId, Pager,
            page::{PAGE_CAPACITY}},
        value::Value,
    },
};

use super::{
    Row, Rid,
    schema::Schema,
};

#[derive(Debug, PartialEq)]
pub struct Table {
    pub tid: usize,
    pub name: String,
    pub tree_root: PageId,
    pub active_data: PageId,
    pub schema: Schema,
}

impl Table {
    pub fn new(tid: usize, name: &str, schema: Schema, pager: &mut Pager) -> DbResult<Self> {
        let tree = BpTree::create(pager)?;
        let active_data = pager.alloc();
        let data = DataPage::new(active_data);
        pager.write(data)?;
        pager.flush()?;

        Ok(Self { tid, name: name.into(), tree_root: tree.root, active_data, schema })
    }

    pub fn from_row(row: &Row, schema: Schema) -> DbResult<Self> {
        Ok(Self {
            tid: row.field(0)?,
            name: row.field(1)?,
            tree_root: row.field(2)?,
            active_data: row.field(3)?,
            schema
        })
    }

    pub fn insert(&mut self, row: Row, pager: &mut Pager) -> DbResult<()>{
        let key = self.schema.gen_key(&row)?;
        let bytes = self.schema.encode(row)?;

        let mut data = match pager.read_header(self.active_data)?.free_space() {
            Some(space) => {
                if bytes.len() > space as usize {
                    let new_id = pager.alloc();
                    self.active_data = new_id;

                    DataPage::new(new_id)
                } else {
                    pager.read::<DataPage>(self.active_data)?
                }
            },
            None => {
                let new_id = pager.alloc();
                self.active_data = new_id;
                DataPage::new(new_id)
            }
        };

        let rid = if bytes.len() > PAGE_CAPACITY as usize {
            data.insert_overflow(&bytes, pager)?
        } else {
            data.insert(&bytes)?
        };

        let mut tree = BpTree::open(self.tree_root);
        if let Some(old) = tree.insert(&key, rid, pager)? {
            let mut old_data = pager.read::<DataPage>(old.page)?;
            old_data.delete(old.slot)?;
            pager.write(old_data)?;
        }

        pager.write(data)?;
        pager.flush()?;

        Ok(())
    }

    pub fn remove(&mut self) {
        todo!()
    }

    pub fn scan(&self) {
        todo!()
    }

    pub fn get_row(&self, rid: &Rid, pager: &mut Pager) -> StoreResult<Row> {
        let page = pager.read::<DataPage>(rid.page)?;
        let bytes = page.get(rid.slot)?;
        self.schema.decode(&bytes)
    }
}

impl From<&Table> for Row {
    fn from(table: &Table) -> Self {
        Self(vec![
            Some(Value::Uint(table.tid)),
            Some(Value::Text(table.name.clone())),
            Some(Value::Uint(table.tree_root.get())),
            Some(Value::Uint(table.active_data.get())),
        ])
    }
}
