use crate::{
    errors::DbResult,
    store::{
        bptree::BpTree,
        pager::{DataPage, Page, Pager, page::PageId},
        schema::{Schema, Type},
        value::Value,
    },
};

use std::{
    collections::HashMap,
};

// Hardcoded class schema
const CLASS_COLS: &[(&str, Type)] = &[
    ("tid", Type::Uint),
    ("name", Type::Text),
    ("root_page", Type::Uint),
    ("active_data", Type::Uint),
];
const CLASS_ROOT: usize = 1;
const CLASS_TID: usize = 1;

// Hardcoded attributes schema
const ATTR_COLS: &[(&str, Type)] = &[
    ("attnum", Type::Uint),
    ("tid", Type::Uint),
    ("name", Type::Text),
    ("ty", Type::Uint), // C-style enum mapping types to numbers
    ("is_key", Type::Bool),
    ("not_null", Type::Bool),
    ("is_dead", Type::Bool),
];
const ATTR_ROOT: usize = 2;
const ATTR_TID: usize = 2;

const FIRST_TID: usize = 3; // equal to number of catalog tables + 1

pub struct Catalog {
    pub tables: HashMap<usize, TableMeta>,
    next_tid: usize,
}

struct TableMeta {
    pub tid: usize,
    pub name: String,
    pub tree: BpTree,
    pub schema: Schema,
}

impl Catalog {
    fn init(pager: &mut Pager) -> DbResult<Self> {
        let class = Schema::from_static(CLASS_COLS);
        let attr = Schema::from_static(ATTR_COLS);

        let mut class_tree = BpTree::create(pager)?;
        let mut attr_tree = BpTree::create(pager)?;
        let mut class_page = DataPage::new(pager.alloc());
        let mut attr_page = DataPage::new(pager.alloc());
        
        let catalog_tables = [
            (CLASS_TID, "class_catalog", CLASS_ROOT, class_page.header().id.get(), CLASS_COLS),
            (ATTR_TID, "attr_catalog", ATTR_ROOT, attr_page.header().id.get(), ATTR_COLS),
        ];

        for (tid, name, root, active, cols) in catalog_tables {
            let row = class_row(tid, name, root, active);
            insert_row(&mut class_tree, &mut class_page, &class, &class_key(tid), row, pager)?;

            for (i, col) in cols.iter().enumerate() {
                let attnum = i + 1;
                let row = attr_row(col, attnum, tid);
                insert_row(&mut attr_tree, &mut attr_page, &attr, &attr_key(tid, attnum), row, pager)?;
            }
        }

        pager.write(class_page)?;
        pager.write(attr_page)?;
        pager.flush()?;

        let mut tables = HashMap::new();
        tables.insert(CLASS_TID, TableMeta {
            tid: CLASS_TID,
            name: "class_catalog".into(),
            tree: class_tree,
            schema: class,
        });
        tables.insert(ATTR_TID, TableMeta {
            tid: ATTR_TID,
            name: "attr_catalog".into(),
            tree: attr_tree,
            schema: attr,
        });
        
        Ok(Self { tables, next_tid: FIRST_TID }) 
    }

    fn open(pager: &mut Pager) -> DbResult<Self> {
        todo!()
    }
}

fn class_key(tid: usize) -> String {
    format!("{:020}", tid)
}

fn attr_key(tid: usize, attnum: usize) -> String {
    format!("{:020}{:020}", tid, attnum)
}

fn insert_row(
    tree: &mut BpTree,
    page: &mut DataPage,
    schema: &Schema,
    key: &str,
    row: Vec<Option<Value>>,
    pager: &mut Pager,
) -> DbResult<()> {
    let bytes = schema.encode(row)?;
    let rid = page.insert(&bytes)?; // TODO: bounds check insertion
    tree.insert(key, rid, pager)?;

    Ok(())
}

fn class_row(tid: usize, name: &str, root: usize, active: usize) -> Vec<Option<Value>> {
    vec![
        Some(Value::Uint(tid)),
        Some(Value::Text(name.into())),
        Some(Value::Uint(root)),
        Some(Value::Uint(active)),
    ]
}

fn attr_row(hardcode: &(&str, Type), attnum: usize, tid: usize) -> Vec<Option<Value>> {
    let mut row: Vec<Option<Value>> = Vec::with_capacity(7); // Number of columns is Attr
    row.push(Some(Value::Uint(attnum)));
    row.push(Some(Value::Uint(tid)));
    row.push(Some(Value::Text(hardcode.0.into())));
    row.push(Some(Value::Uint(hardcode.1 as usize)));

    // clumsy solution that will need to get replaced later
    if attnum == 1 {
        row.push(Some(Value::Bool(true)));
    } else { 
        row.push(Some(Value::Bool(false)));
    }

    row.push(Some(Value::Bool(true)));
    row.push(Some(Value::Bool(false)));

    row
}

#[cfg(test)]
mod tests {
    use crate::store::schema::Column;
    use super::*;
    use tempfile::NamedTempFile;

    fn init_catalog() -> (Pager, Catalog) {
        let file = NamedTempFile::new().unwrap();
        let mut pager = Pager::new(file.path().to_str().unwrap()).unwrap();
        let catalog = Catalog::init(&mut pager).unwrap();

        (pager, catalog)
    }

    #[test]
    fn check_roots() {
        let (_, catalog) = init_catalog();

        let class_root = catalog.tables.get(&CLASS_TID).unwrap().tree.root.unwrap().get();
        println!("{}", class_root);
        assert_eq!(class_root, CLASS_ROOT);

        let attr_root = catalog.tables.get(&ATTR_ROOT).unwrap().tree.root.unwrap().get();
        println!("{}", attr_root);
        assert_eq!(attr_root, ATTR_ROOT);
    }

    #[test]
    fn check_class_rows() {
        let (mut pager, catalog) = init_catalog();

        let class_table = catalog.tables.get(&CLASS_TID).unwrap();
        let class_rid = class_table.tree.get(&class_key(CLASS_TID), &mut pager).unwrap();
        let class_page = pager.read::<DataPage>(class_rid.page).unwrap();
        let class_bytes = class_page.get(class_rid.slot).unwrap();
        let class_row = class_table.schema.decode(&class_bytes).unwrap();

        assert_eq!(class_row[0], Some(Value::Uint(CLASS_TID))); 
        assert_eq!(class_row[1], Some(Value::Text("class_catalog".into())));
        assert_eq!(class_row[2], Some(Value::Uint(CLASS_ROOT)));
        assert_eq!(class_row[3], Some(Value::Uint(class_page.header().id.get())));
    }

    #[test]
    fn check_attr_ownership() {
        let (mut pager, catalog) = init_catalog();

        let attr_table = catalog.tables.get(&ATTR_TID).unwrap();
        
        for i in 1..=4 {
            let _ = attr_table.tree.get(&attr_key(CLASS_TID, i), &mut pager).unwrap();
        }

        for i in 1..=7 {
            let _ = attr_table.tree.get(&attr_key(ATTR_TID, i), &mut pager).unwrap();
        }
    }

    #[test]
    fn check_schema_encoding() {
        let (mut pager, catalog) = init_catalog();

        let attr_table = catalog.tables.get(&ATTR_TID).unwrap();
        let rids = attr_table.tree.scan_rids(&mut pager).unwrap();

        let mut cols = Vec::new();
        for rid in rids {
            let page = pager.read::<DataPage>(rid.page).unwrap();
            let bytes = page.get(rid.slot).unwrap();
            let row = attr_table.schema.decode(&bytes).unwrap();
            if row[1].clone().unwrap().as_uint().unwrap() == CLASS_TID { cols.push(Column::from_row(row)); }
        }

        assert_eq!(Schema(cols), Schema::from_static(CLASS_COLS))
    }

}
