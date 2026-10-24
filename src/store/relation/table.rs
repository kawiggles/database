use std::collections::HashSet;

use crate::{
    errors::{DbResult, StoreResult},
    store::{
        bptree::BpTree,
        pager::{Page, DataPage, PageId, Pager, data::MAX_FREE_SPACE},
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

    /// Generates a new table from a predefined table id, name, and schema
    pub fn new(tid: usize, name: &str, schema: Schema, pager: &mut Pager) -> DbResult<Self> {
        let tree = BpTree::create(pager)?;
        let active_data = pager.alloc();
        let data = DataPage::new(active_data);
        pager.write(data)?;
        pager.flush()?;

        Ok(Self { tid, name: name.into(), tree_root: tree.root, active_data, schema })
    }

    /// Generates a table from a stored catalog row, used when opening catalog
    pub fn from_row(row: &Row, schema: Schema) -> DbResult<Self> {
        Ok(Self {
            tid: row.field(0)?,
            name: row.field(1)?,
            tree_root: row.field(2)?,
            active_data: row.field(3)?,
            schema
        })
    }

    /// Inserts a Row into a datapage, and then inserts the associated Rid into the Table b+ tree
    /// Caller must check if active_data is different and update the catalog accordingly
    pub fn insert(&mut self, row: Row, pager: &mut Pager) -> DbResult<()> {
        let key = self.schema.gen_key(&row)?;
        let bytes = self.schema.encode(row)?;

        let mut data = pager.read::<DataPage>(self.active_data)?;

        let rid = if bytes.len() > MAX_FREE_SPACE {
            let new = DataPage::new(pager.alloc());
            self.active_data = new.header().id;

            let rid = data.insert_overflow(&bytes, pager)?;
            pager.write(data)?;
            pager.write(new)?;

            rid
        } else {
            if let Some(space) = data.free_space() && space as usize > bytes.len() {
                let rid = data.insert(&bytes)?;
                pager.write(data)?;
                rid
            } else {
                let mut new = DataPage::new(pager.alloc());
                self.active_data = new.header().id;

                let rid = new.insert(&bytes)?;
                pager.write(new)?;
                rid
            }
        };
        pager.flush()?;

        let mut tree = BpTree::open(self.tree_root);
        if let Some(old) = tree.insert(&key, rid, pager)? {
            let mut old_data = pager.read::<DataPage>(old.page)?;
            old_data.delete(old.slot)?;
            pager.write(old_data)?;
        }
        pager.flush()?;

        Ok(())
    }

    /// Clears a row (identified by key) from the table's BpTree and tombstones the slot in its 
    /// associated datapage
    pub fn remove(&mut self, key: String, pager: &mut Pager) -> DbResult<()> {
        let mut tree = BpTree::open(self.tree_root);
        let rid = tree.remove(&key, pager)?;
        let mut data = pager.read::<DataPage>(rid.page)?;
        data.delete(rid.slot)?;
        pager.write(data)?;
        pager.flush()?;

        Ok(())
    }

    /// Returns every Row in a table
    pub fn scan(&self, pager: &mut Pager) -> DbResult<Vec<Row>>{
        let tree = BpTree::open(self.tree_root);
        let rids = tree.scan_rids(pager)?;

        let mut rows: Vec<Row> = Vec::new();
        let mut pages: HashSet<PageId> = HashSet::new();

        for rid in rids {
            if pages.contains(&rid.page) { continue; }

            let data = pager.read::<DataPage>(rid.page)?;
            let data_rows: Vec<Row> = data.data.into_iter()
                .flatten()
                .filter_map(|bytes| self.schema.decode(&bytes).ok())
                .collect();
            rows.extend_from_slice(&data_rows);

            pages.insert(rid.page);
        }

        Ok(rows)
    }

    /// Returns a single Row from a table, identified by Rid
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
