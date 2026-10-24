use crate::{
    errors::{DbResult, UserErr, UserResult, StoreErr},
    store::{
        bptree::BpTree,
        pager::{DataPage, Pager, page::PageId},
        relation::{
            schema::{Column, Schema, Type},
            table::Table,
        },
    },
};

use std::{
    collections::HashMap,
};

// Hardcoded class schema
#[rustfmt::skip]
const CLASS_COLS: &[(&str, Type, bool)] = &[
    ("tid", Type::Uint, true),
    ("name", Type::Text, false),
    ("root_page", Type::Uint, false),
    ("active_data", Type::Uint, false),
];
const CLASS_TID: usize = 1;
const CLASS_ROOT: usize = 1;


// Hardcoded attributes schema
#[rustfmt::skip]
const ATTR_COLS: &[(&str, Type, bool)] = &[
    ("tid", Type::Uint, true),
    ("attnum", Type::Uint, true),
    ("name", Type::Text, false),
    ("ty", Type::Uint, false), // C-style enum mapping types to numbers
    ("is_key", Type::Bool, false),
    ("nullable", Type::Bool, false),
    ("is_dead", Type::Bool, false),
];
const ATTR_TID: usize = 2;
const ATTR_ROOT: usize = 3;

// equal to number of catalog tables + 1, so if you add another catalog table, update this
const FIRST_TID: usize = 3;

#[derive(Debug, PartialEq)]
pub struct Catalog {
    pub tables: HashMap<usize, Table>,
    pub names: HashMap<String, usize>,
    next_tid: usize,
}

impl Catalog {

    /// Create a new, empty catalog, seralize it and write to memory
    pub fn init(pager: &mut Pager) -> DbResult<Self> {
        let mut tables = HashMap::new();
        let mut names = HashMap::new();

        let schemas = vec![Schema::from_static(CLASS_COLS, CLASS_TID),
            Schema::from_static(ATTR_COLS, ATTR_TID)];

        let mut class = Table::new(CLASS_TID, "class_catalog", schemas[0].clone(), pager)?;
        let mut attr = Table::new(ATTR_TID, "attr_catalog", schemas[1].clone(), pager)?;

        class.insert((&class).into(), pager)?;
        class.insert((&attr).into(), pager)?;

        let mut next_tid: usize = 0;
        for schema in schemas {
            next_tid += 1;
            for (i, mut col) in schema.0.into_iter().enumerate() {
                col.tid = next_tid;
                col.attnum = i;
                attr.insert((&col).into(), pager)?;
            }
        }

        tables.insert(CLASS_TID, class);
        tables.insert(ATTR_TID, attr);
        names.insert("class_catalog".into(), CLASS_TID);
        names.insert("attr_catalog".into(), ATTR_TID);

        Ok(Self { tables, names, next_tid: FIRST_TID }) 
    }

    pub fn open(pager: &mut Pager) -> DbResult<Self> {
        // get schemas from scanning class and attr trees
        let class_tree = BpTree::open(PageId::new(CLASS_ROOT).unwrap());
        let class_rids = class_tree.scan_rids(pager)?;
        let class_schema = Schema::from_static(CLASS_COLS, CLASS_TID);

        let attr_tree = BpTree::open(PageId::new(ATTR_ROOT).unwrap());
        let attr_rids = attr_tree.scan_rids(pager)?;
        let attr_schema = Schema::from_static(ATTR_COLS, ATTR_TID);

        // Hashmap to store column schemas with their associated tables
        let mut schemas: HashMap<usize, Vec<Column>> = HashMap::new();
        for rid in attr_rids {
            let page = pager.read::<DataPage>(rid.page)?;
            let bytes = page.get(rid.slot)?;
            let col = Column::try_from(attr_schema.decode(&bytes)?)?;

            schemas.entry(col.tid)
                .or_default()
                .push(col);
        }

        // setup our return values
        let mut tables = HashMap::new();
        let mut names = HashMap::new();
        let mut next_tid = 1;

        // create and insert tables into catalog for each rid
        for rid in class_rids {
            let page = pager.read::<DataPage>(rid.page)?;
            let row = class_schema.decode(&page.get(rid.slot)?)?;
            let tid: usize = row.field(0)?;

            let schema = schemas.remove(&tid).ok_or(StoreErr::NoSchema(tid))?;
            let table = Table::from_row(&row, Schema(schema))?;

            names.insert(table.name.clone(), tid);
            tables.insert(tid, table);

            next_tid = tid + 1;
        }

        Ok(Self { tables, names, next_tid })
    }

    pub fn lookup_by_tid(&self, tid: usize) -> UserResult<&Table> {
        self.tables.get(&tid).ok_or(UserErr::NoTable)
    }

    pub fn lookup_by_name(&self, name: &str) -> UserResult<&Table> {
        let tid = self.names.get(name).ok_or(UserErr::NoTable)?;
        self.tables.get(&tid).ok_or(UserErr::NoTable)
    }

    pub fn create_table(&mut self, name: &str, schema: Schema, pager: &mut Pager) -> DbResult<usize> {
        schema.validate()?;

        let tid = self.next_tid;
        let new_table = Table::new(tid, name, schema.clone(), pager)?;
        
        let class = self.tables.get_mut(&CLASS_TID).ok_or(StoreErr::NoClass)?;
        // TODO: check that the name doesn't override an existing table
        class.insert((&new_table).into(), pager)?;

        let attr = self.tables.get_mut(&ATTR_TID).ok_or(StoreErr::NoAttr)?;

        for (i, mut col) in schema.0.into_iter().enumerate() {
            col.tid = tid;
            col.attnum = i;
            attr.insert((&col).into(), pager)?;
        }

        self.tables.insert(tid, new_table);
        self.names.insert(name.into(), tid);

        self.next_tid += 1;
        Ok(tid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::relation::Row;
    use tempfile::NamedTempFile;

    fn init_catalog() -> (Pager, Catalog, NamedTempFile) {
        let file = NamedTempFile::new().unwrap();
        let mut pager = Pager::new(file.path().to_str().unwrap()).unwrap(); // trust me bro
        let catalog = Catalog::init(&mut pager).unwrap();

        (pager, catalog, file)
    }

    fn open_catalog<>(file: NamedTempFile) -> (Pager, Catalog) {
        let mut pager = Pager::open(file.path().to_str().unwrap()).unwrap();
        let catalog = Catalog::open(&mut pager).unwrap();

        (pager, catalog)
    }

    #[test]
    fn check_roots() {
        let (_, catalog, _) = init_catalog();

        let class_root = catalog.tables.get(&CLASS_TID).unwrap().tree_root.get();
        println!("{}", class_root);
        assert_eq!(class_root, CLASS_ROOT);

        let attr_root = catalog.tables.get(&ATTR_TID).unwrap().tree_root.get();
        println!("{}", attr_root);
        assert_eq!(attr_root, ATTR_ROOT);
    }

    #[test]
    fn check_class_rows() {
        let (mut pager, catalog, _) = init_catalog();

        let class_table = catalog.tables.get(&CLASS_TID).unwrap();
        let class_key = class_table.schema.gen_key(&class_table.into()).unwrap();
        let class_tree = BpTree::open(class_table.tree_root);
        let class_rid = class_tree.get(&class_key, &mut pager).unwrap();
        let class_page = pager.read::<DataPage>(class_rid.page).unwrap();
        let class_bytes = class_page.get(class_rid.slot).unwrap();
        let class_row = class_table.schema.decode(&class_bytes).unwrap();

        assert_eq!(class_row, Row::from(class_table))
    }

    #[test]
    fn check_ownership_and_encoding() {
        let (mut pager, catalog, _) = init_catalog();

        let attr_table = catalog.tables.get(&ATTR_TID).unwrap();
        let tree = BpTree::open(attr_table.tree_root);

        let mut class_cols: Vec<Column> = Vec::new();
        let mut attr_cols: Vec<Column> = Vec::new();
        let rids = tree.scan_rids(&mut pager).unwrap();
        assert_eq!(rids.len(), CLASS_COLS.len() + ATTR_COLS.len());

        for (i, rid) in rids.iter().enumerate() {
            let row = attr_table.get_row(rid, &mut pager).unwrap();
            let col = Column::try_from(row).unwrap();

            if i < CLASS_COLS.len() {
                assert_eq!(col.tid, CLASS_TID);
            } else {
                assert_eq!(col.tid, ATTR_TID);
            }

            if col.tid == CLASS_TID {
                class_cols.push(col);
            } else if col.tid == ATTR_TID {
                attr_cols.push(col);
            } else {
                panic!("Non-catalog column found");
            }
        }

        assert_eq!(Schema(class_cols), Schema::from_static(CLASS_COLS, CLASS_TID));
        assert_eq!(Schema(attr_cols), Schema::from_static(ATTR_COLS, ATTR_TID));
    }

    #[test]
    fn init_then_open() {
        let (mut pager, i_catalog, file) = init_catalog();

        pager.close().unwrap();
        let (_, e_catalog) = open_catalog(file);

        assert_eq!(i_catalog, e_catalog);
    }

    #[test]
    fn create_and_close() -> DbResult<()> {
        let (mut pager, mut i_catalog, file) = init_catalog();

        let schema = Schema::from_static(&[
            ("key val", Type::Text, true),
            ("a bool", Type::Bool, false),
            ("a num", Type::Int, false),
        ], i_catalog.next_tid);

        i_catalog.create_table("test", schema, &mut pager)?;

        pager.close()?;
        let (_, e_catalog) = open_catalog(file);
        
        assert_eq!(i_catalog, e_catalog);

        Ok(())
    }
}
