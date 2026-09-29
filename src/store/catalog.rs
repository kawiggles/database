use crate::{
    errors::{DbResult, StoreErr, StoreResult},
    store::{
        bptree::BpTree,
        pager::{DataPage, Pager, page::PageId},
        schema::{Column, Schema, Type},
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
const CLASS_TID: usize = 1;

// Hardcoded attributes schema
const ATTR_COLS: &[(&str, Type)] = &[
    ("attnum", Type::Uint),
    ("tid", Type::Uint),
    ("name", Type::Text),
    ("ty", Type::Uint), // C-style enum mapping types to numbers
    ("is_key", Type::Bool),
    ("nullable", Type::Bool),
    ("is_dead", Type::Bool),
];
const ATTR_TID: usize = 2;

const FIRST_TID: usize = 3; // equal to number of catalog tables + 1

// TODO: store catalog roots in the database header
pub struct Catalog {
    pub tables: HashMap<usize, TableMeta>,
    next_tid: usize,
}

pub struct TableMeta {
    pub tid: usize,
    pub name: String,
    pub tree: BpTree,
    pub active_data: PageId,
    pub schema: Schema,
}

impl Catalog {

    /// Create a new, empty catalog, seralize it and write to memory
    pub fn init(pager: &mut Pager) -> DbResult<Self> {
        let class = Schema::from_static(CLASS_COLS);
        let attr = Schema::from_static(ATTR_COLS);

        let mut class_tree = BpTree::create(pager)?;
        let mut attr_tree = BpTree::create(pager)?;
        let class_page_id = pager.alloc();
        let attr_page_id = pager.alloc();
        let mut class_page = DataPage::new(class_page_id);
        let mut attr_page = DataPage::new(attr_page_id);
        let mut tables = HashMap::new();
        
        let catalog_tables = [
            (CLASS_TID, "class_catalog", class_tree.root.unwrap(), class_page_id, CLASS_COLS),
            (ATTR_TID, "attr_catalog", attr_tree.root.unwrap(), attr_page_id, ATTR_COLS),
        ];

        // bootstrapping happens here
        for (tid, name, root, active, cols) in catalog_tables {
            let row = ClassRow { tid, name: name.into(), root: root, active };
            insert_row(&mut class_tree,
                &mut class_page,
                &class,
                &class_key(tid),
                Vec::<Option<Value>>::from(row),
                pager)?;

            // The magic numbers here are "hard-coded", much like the schemas. Do not alter!
            for (i, col) in Schema::from_static(cols).0.into_iter().enumerate() {
                let row = AttrRow { attnum: i + 1, tid, col };
                let key = attr_key(tid, i + 1);
                insert_row(&mut attr_tree, &mut attr_page, &attr, &key, row.into(), pager)?;
            }

        }

        pager.write(class_page)?;
        pager.write(attr_page)?;
        pager.flush()?;

        tables.insert(CLASS_TID, TableMeta {
            tid: CLASS_TID,
            name: "class_catalog".into(),
            tree: class_tree,
            active_data: class_page_id,
            schema: Schema::from_static(CLASS_COLS),
        });

        tables.insert(ATTR_TID, TableMeta {
            tid: ATTR_TID,
            name: "attr_catalog".into(),
            tree: attr_tree,
            active_data: attr_page_id,
            schema: Schema::from_static(ATTR_COLS),
        });

        Ok(Self { tables, next_tid: FIRST_TID }) 
    }

    pub fn open(pager: &mut Pager) -> DbResult<Self> {
        todo!()
    }

    pub fn lookup_by_tid(&self) -> DbResult<TableMeta> {
        todo!()
    }

    pub fn lookup_by_name(&self) -> DbResult<TableMeta> {
        todo!()
    }

    pub fn create_table(&mut self, name: &str, cols: Schema) -> DbResult<()> {
        todo!()
    }
}

pub struct ClassRow {
    pub tid: usize,
    pub name: String,
    pub root: PageId,
    pub active: PageId,
}
 
impl TryFrom<Vec<Option<Value>>> for ClassRow {
    type Error = StoreErr;

    fn try_from(mut row: Vec<Option<Value>>) -> Result<Self, Self::Error> {
        Ok(Self {
            tid: field(&mut row, 0)?,
            name: field(&mut row, 1)?,
            root: field(&mut row, 2)?,
            active: field(&mut row, 3)?,
        })
    }
}

impl From<ClassRow> for Vec<Option<Value>> {
    fn from(row: ClassRow) -> Self {
        vec![
            Some(Value::Uint(row.tid)),
            Some(Value::Text(row.name.into())),
            Some(Value::Uint(row.root.get())),
            Some(Value::Uint(row.active.get())),
        ]
    }
}

pub struct AttrRow {
    pub attnum: usize,
    pub tid: usize,
    pub col: Column,
}

/// Get a primative from a particular column of a row (Vec<Option<Value>>)
fn field<T>(val: &mut [Option<Value>], col: usize) -> StoreResult<T>
where T: TryFrom<Value, Error = StoreErr> {
    val.get_mut(col)
        .ok_or(StoreErr::BadColIndex(col))?
        .take()
        .ok_or(StoreErr::NullField(col))?
        .try_into()
}

impl TryFrom<Vec<Option<Value>>> for AttrRow {
    type Error = StoreErr;

    fn try_from(mut row: Vec<Option<Value>>) -> Result<Self, Self::Error> {
        Ok(Self {
            attnum: field(&mut row, 0)?,
            tid: field(&mut row, 1)?,
            col: Column {
                name: field(&mut row, 2)?,
                ty: field(&mut row, 3)?,
                is_key: field(&mut row, 4)?,
                nullable: field(&mut row, 5)?,
                is_dead: field(&mut row, 6)?,
            },
        })
    }
}

impl From<AttrRow> for Vec<Option<Value>> {
    fn from(attr: AttrRow) -> Self {
        vec![
            Some(attr.attnum.into()),
            Some(attr.tid.into()),
            Some(attr.col.name.into()),
            Some((attr.col.ty as usize).into()),
            Some(attr.col.is_key.into()),
            Some(attr.col.nullable.into()),
            Some(attr.col.is_dead.into()),
        ]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::pager::Page;
    use tempfile::NamedTempFile;

    const CLASS_ROOT: usize = 1;
    const ATTR_ROOT: usize = 2;

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

        let mut cols: Vec<Column> = Vec::new();
        for rid in rids {
            let page = pager.read::<DataPage>(rid.page).unwrap();
            let bytes = page.get(rid.slot).unwrap();
            let row = attr_table.schema.decode(&bytes).unwrap();
            let attr = AttrRow::try_from(row).unwrap();
            if attr.tid == CLASS_TID {
                cols.push(attr.col);
            }
        }

        assert_eq!(Schema(cols), Schema::from_static(CLASS_COLS))
    }

}
