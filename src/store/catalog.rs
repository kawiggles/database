use crate::{
    errors::{DbResult},
    store::{
        pager::{Pager, page::PageId},
        schema::{Type, Column, Schema},
    },
};

// Hardcoded class schema
const CLASS_COLS: &[(&str, Type)] = &[("root_page", Type::Uint)];

// Hardcoded attributes schema
const ATTR_COLS: &[(&str, Type)] = &[
    ("table_name", Type::Text),
    ("attnum", Type::Uint),
    ("name", Type::Text),
    ("ty", Type::Uint), // C-style enum mapping types to numbers
    ("is_key", Type::Bool),
    ("not_null", Type::Bool),
];

// TODO: make tables a hash map from the table name to the TableMeta
pub struct Catalog;

impl Catalog {
    pub fn get_table(pager: &mut Pager, name: &str) -> DbResult<TableMeta> {
        todo!()
    }

    pub fn create_table(pager: &mut Pager, name: &str, cols: &[Column]) -> DbResult<()> {
        todo!()
    }

    pub fn drop_table(pager: &mut Pager, name: &str) -> DbResult<()> {
        todo!()
    }
}

struct TableMeta {
    pub name: String,
    pub root: PageId,
    pub schema: Schema,
}

/* 
** TODO: bootstrap the class and catalog tables using the scheme defined above
** There are two cases: one for each way the pager can be opened, which is new() and open()
**
** For new(), we need to allocate a new page, set that as the root page for a new B+ tree. We
** immediately add a new entry in the form of the attributes table, which 
*/
