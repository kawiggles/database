use crate::{
    errors::{DbResult, StoreResult, StoreErr},
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
    pub active_data: PageId,
    pub schema: Schema,
}

impl Catalog {
    pub fn init(pager: &mut Pager) -> DbResult<Self> {
        let class = Schema::from_static(CLASS_COLS);
        let attr = Schema::from_static(ATTR_COLS);

        let mut class_tree = BpTree::create(pager)?;
        let mut attr_tree = BpTree::create(pager)?;
        let class_page_id = pager.alloc();
        let attr_page_id = pager.alloc();
        let mut class_page = DataPage::new(class_page_id);
        let mut attr_page = DataPage::new(attr_page_id);
        
        let catalog_tables = [
            (CLASS_TID, "class_catalog", PageId::new(CLASS_ROOT).unwrap(),
                class_page.header().id, CLASS_COLS),
            (ATTR_TID, "attr_catalog", PageId::new(ATTR_ROOT).unwrap(),
                attr_page.header().id, ATTR_COLS),
        ];

        for (tid, name, root, active, cols) in catalog_tables {
            let row = ClassRow { tid, name: name.into(), root, active };
            insert_row(&mut class_tree,
                &mut class_page,
                &class,
                &class_key(tid),
                Vec::<Option<Value>>::from(row),
                pager)?;

            for (i, col) in cols.iter().enumerate() {
                let attnum = i + 1;
                let row = AttrRow { attnum, tid, name: , ty: , is_key:  };
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
            active_data: class_page_id,
            schema: class,
        });
        tables.insert(ATTR_TID, TableMeta {
            tid: ATTR_TID,
            name: "attr_catalog".into(),
            tree: attr_tree,
            active_data: attr_page_id,
            schema: attr,
        });
        
        Ok(Self { tables, next_tid: FIRST_TID }) 
    }

    pub fn open(pager: &mut Pager) -> DbResult<Self> {
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

    fn try_from(row: Vec<Option<Value>>) -> Result<Self, Self::Error> {
        let [tid, name, root, active]: [Option<Value>; 4] =
            row.try_into()
            .map_err(|r: Vec<_>| StoreErr::BadColCount{ expected: 4, found: r.len() })?;

        Ok(Self {
            tid: tid.and_then(|v| v.as_uint()).ok_or(StoreErr::AttrFieldDecodeErr(0))?,
            name: name.and_then(|v| v.as_text()).ok_or(StoreErr::AttrFieldDecodeErr(1))?,
            root: PageId::new(root
                .and_then(|v| v.as_uint())
                .ok_or(StoreErr::AttrFieldDecodeErr(2))?)
                .ok_or(StoreErr::PageIdZero)?,
            active: PageId::new(active
                .and_then(|v| v.as_uint())
                .ok_or(StoreErr::AttrFieldDecodeErr(3))?)
                .ok_or(StoreErr::PageIdZero)?,
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
    pub name: String,
    pub ty: Type,
    pub is_key: bool,
    pub not_null: bool,
    pub is_dead: bool,
}

// TODO: try using this instead
fn field<T>(val: Option<Value>, col: usize) -> StoreResult<T>
where T: TryFrom<Value, Error = StoreErr> {
    val.ok_or(StoreErr::NullField(col))?.try_into()
}

impl TryFrom<Vec<Option<Value>>> for AttrRow {
    type Error = StoreErr;

    fn try_from(row: Vec<Option<Value>>) -> Result<Self, Self::Error> {
        let [attnum, tid, name, ty, is_key, not_null, is_dead]: [Option<Value>; 7] =
            row.try_into()
            .map_err(|r: Vec<_>| StoreErr::BadColCount { expected: 7, found: r.len() })?;
        
        Ok(Self {
            // TODO: instead of as_T(), do TryFrom on Value
            attnum: attnum.and_then(|v| v.as_uint()).ok_or(StoreErr::AttrFieldDecodeErr(0))?,
            tid: tid.and_then(|v| v.as_uint()).ok_or(StoreErr::AttrFieldDecodeErr(1))?,
            name: name.and_then(|v| v.as_text()).ok_or(StoreErr::AttrFieldDecodeErr(2))?,
            ty: Type::try_from(ty.and_then(|v| v.as_uint()).ok_or(StoreErr::AttrFieldDecodeErr(3))?)?,
            is_key: is_key.and_then(|v| v.as_bool()).ok_or(StoreErr::AttrFieldDecodeErr(4))?,
            not_null: not_null.and_then(|v| v.as_bool()).ok_or(StoreErr::AttrFieldDecodeErr(5))?,
            is_dead: is_dead.and_then(|v| v.as_bool()).ok_or(StoreErr::AttrFieldDecodeErr(6))?,
        })
    }
}

impl From<AttrRow> for Vec<Option<Value>> {
    // TODO: change this to actually be dependent on input values
    fn from(attr: AttrRow) -> Self {
        let mut row: Vec<Option<Value>> = Vec::with_capacity(7); // Number of columns is Attr
        row.push(Some(Value::Uint(attr.attnum)));
        row.push(Some(Value::Uint(attr.tid)));
        row.push(Some(Value::Text(attr.name.into())));
        row.push(Some(Value::Uint(attr.ty as usize)));

        if attr.attnum == 1 {
            row.push(Some(Value::Bool(true)));
        } else { 
            row.push(Some(Value::Bool(false)));
        }
        row.push(Some(Value::Bool(true)));
        row.push(Some(Value::Bool(false)));

        row
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
            if row[1].clone().unwrap().as_uint().unwrap() == CLASS_TID { cols.push(Column::from_row(row).unwrap()); }
        }

        assert_eq!(Schema(cols), Schema::from_static(CLASS_COLS))
    }

}
