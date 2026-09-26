use crate::{
    errors::StoreResult,
    store::{
        bptree::BpTree,
        pager::{Pager, DataPage, page::PageId},
        schema::{Schema, Type},
    },
};

use std::{
    collections::HashMap,
};

// Hardcoded class schema
const CLASS_COLS: &[(&str, Type)] = &[
    ("tid", Type::Uint),
    ("root_page", Type::Uint),
    ("active_data", Type::Uint),
];

// Hardcoded attributes schema
const ATTR_COLS: &[(&str, Type)] = &[
    ("tid", Type::Uint),
    ("attnum", Type::Uint),
    ("name", Type::Text),
    ("ty", Type::Uint), // C-style enum mapping types to numbers
    ("is_key", Type::Bool),
    ("not_null", Type::Bool),
    ("is_dead", Type::Bool),
];

const CLASS_ROOT: usize = 1;
const CLASS_TID: usize = 1;
const ATTRIBUTE_ROOT: usize = 2;
const ATTRIBUTE_TID: usize = 2;
const FIRST_TID: usize = 3;

pub struct Catalog {
    tables: HashMap<usize, BpTree>,
    next_oid: usize,
}

impl Catalog {
    fn new(pager: &mut Pager) -> StoreResult<Self> {
        let mut class_tree = BpTree::new(PageId::new(CLASS_ROOT));
        let class = Schema::from_static(CLASS_COLS);
        let mut class_page = DataPage::new(pager.alloc());
        
        let rid = class_page.insert();
        class_tree.insert();

        let mut attribute_tree = BpTree::new(PageId::new(ATTRIBUTE_ROOT));
        let attribute = Schema::from_static(ATTR_COLS);

        let mut tables = HashMap::new();
        tables.insert(CLASS_TID, class_tree);
        tables.insert(ATTRIBUTE_TID, attribute_tree);

        
        Ok(Self { tables: HashMap::new(), next_oid: FIRST_TID }) 
    }
}

struct TableMeta {
    pub tid: usize,
    pub name: String,
    pub tree: BpTree,
    pub schema: Schema,
}

/* 
** TODO: bootstrap the class and catalog tables using the scheme defined above
** There are two cases: one for each way the pager can be opened, which is new() and open()
**
** For new(), we need to allocate a new page, set that as the root page for a new B+ tree. We
** immediately add a new entry in the form of the attributes table, which 
*/
