use crate::{
    errors::{DbResult, StoreErr, StoreResult},
    store::{
        bptree::BpTree,
        pager::{DataPage, PageId, Pager,
            page::{PAGE_CAPACITY}},
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
    pub fn new(tid: usize, name: &str, schema: Schema, pager: &mut Pager) -> DbResult<Self> {
        let tree = BpTree::create(pager)?;
        let active_data = pager.alloc();
        let data = DataPage::new(active_data);
        pager.write(data)?;
        pager.flush()?;

        Ok(Self { tid, name: name.into(), tree, active_data, schema })
    }

    pub fn insert(&mut self, row: Vec<Option<Value>>, pager: &mut Pager) -> DbResult<()>{
        let key = self.schema.gen_row_keys(&row)?;
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

        if let Some(old) = self.tree.insert(&key, rid, pager)? {
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

    pub fn get_row(&self, rid: Rid, pager: &mut Pager) -> StoreResult<Vec<Option<Value>>> {
        let page = pager.read::<DataPage>(rid.page)?;
        let bytes = page.get(rid.slot)?;
        self.schema.decode(&bytes)
    }

    pub fn class_key(&self) -> String {
        format!("{:020}", self.tid)
    }
}

impl From<&Table> for Vec<Option<Value>> {
    fn from(table: &Table) -> Self {
        vec![
            Some(Value::Uint(table.tid)),
            Some(Value::Text(table.name.clone())),
            Some(Value::Uint(table.tree.root.get())),
            Some(Value::Uint(table.active_data.get())),
        ]
    }
}
