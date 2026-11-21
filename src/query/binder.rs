use crate::{
    errors::{DbResult, UserErr},
    store::{
        value::Value,
        catalog::Catalog,
        relation::schema::Type,
    },
    query::ast::{BOp, UOp, Statement, Create, Drop, Select, Insert, Update, Delete, Expr},
};

pub enum BoundStatement {
    Create(BoundCreate),
    Drop(BoundDrop),
    Select(BoundSelect),
    Insert(BoundInsert),
    Update(BoundUpdate),
    Delete(BoundDelete),
}

#[derive(Debug, PartialEq)]
struct BoundCreate {
    name: String,
    columns: Vec<BoundDefinition>,
    primary_key: Option<Vec<usize>>,
}

#[derive(Debug, PartialEq)]
struct BoundDrop(usize);

#[derive(Debug, PartialEq)]
struct BoundSelect {
    table: usize,
    projection: Vec<BoundProjection>,
    filter: Option<BoundExpr>
}

#[derive(Debug, PartialEq)]
struct BoundInsert {
    table: usize,
    columns: Vec<usize>, // indicies of columns
    values: Vec<Vec<BoundExpr>>,
}

#[derive(Debug, PartialEq)]
struct BoundUpdate {
    table: usize,
    assignments: Vec<(usize, BoundExpr)>, // (column index, value)
    filter: Option<BoundExpr>
}

#[derive(Debug, PartialEq)]
struct BoundDelete {
    table: usize,
    filter: Option<BoundExpr>
}

#[derive(Debug, PartialEq)]
struct BoundExpr { kind: BoundExprKind, ty: Type }

#[derive(Debug, PartialEq)]
enum BoundExprKind {
    Column { table: usize, index: usize },
    Literal(Value),
    Unary { op: UOp, expr: Box<BoundExpr> },
    Binary { left: Box::<BoundExpr>, op: BOp, right: Box<BoundExpr> },
    IsNull { expr: Box<BoundExpr>, negated: bool }
}

#[derive(Debug, PartialEq)]
struct BoundDefinition {
    name: String,
    ty: Type,
    nullable: bool,
}

#[derive(Debug, PartialEq)]
struct BoundProjection {
    expr: BoundExpr,
    name: String, // alias or column name
}

impl BoundStatement {
    pub fn bind_statement(statement: Statement, catalog: &Catalog) -> DbResult<Self> {
        match statement {
            Statement::Create(stmt) => Ok(BoundStatement::Create(bind_create(stmt, catalog)?)),
            Statement::Drop(stmt) => Ok(BoundStatement::Drop(bind_drop(stmt, catalog)?)),
            Statement::Select(stmt) => Ok(BoundStatement::Select(bind_select(stmt, catalog)?)),
            Statement::Insert(stmt) => Ok(BoundStatement::Insert(bind_insert(stmt, catalog)?)),
            Statement::Update(stmt) => Ok(BoundStatement::Update(bind_update(stmt, catalog)?)),
            Statement::Delete(stmt) => Ok(BoundStatement::Delete(bind_delete(stmt, catalog)?)),
        }
    }
}

fn bind_create(statement: Create, catalog: &Catalog) -> DbResult<BoundCreate> {
    if catalog.lookup_by_name(&statement.name).is_ok() {
        return Err(UserErr::TableExists)?;
    }

    let mut primary_key: Option<Vec<usize>> = None;
    let mut columns = Vec::new();
    for (i, def) in statement.columns.iter().enumerate() {
        if def.primary {
            if let Some(keys) = &mut primary_key {
                keys.push(i);
            } else {
                primary_key = Some(vec![i]);
            }
        }

        columns.push(BoundDefinition {
            name: def.name.clone(),
            ty: def.ty,
            nullable: !def.not_null,
        });
    }

    Ok(BoundCreate { name: statement.name, columns, primary_key })
}

fn bind_drop(statement: Drop, catalog: &Catalog) -> DbResult<BoundDrop> {
    let table = catalog.lookup_by_name(&statement.table.name)?;
    Ok(BoundDrop(table.tid))
}

fn bind_select(statement: Select, catalog: &Catalog) -> DbResult<BoundSelect> {
    todo!()
}

fn bind_insert(statement: Insert, catalog: &Catalog) -> DbResult<BoundInsert> {
    todo!()
}

fn bind_update(statement: Update, catalog: &Catalog) -> DbResult<BoundUpdate> {
    todo!()
}

fn bind_delete(statement: Delete, catalog: &Catalog) -> DbResult<BoundDelete> {
    todo!()
}

fn bind_expr(expr: Expr, catalog: &Catalog) -> DbResult<BoundExpr> {
    todo!()
}

#[cfg(test)]
mod tests {
    use crate::{query::ast::{Definition, TableRef}, store::relation::schema::{Column, Schema}};
    use super::*;
    use crate::store::pager::Pager;
    use tempfile::NamedTempFile;

    fn init_catalog() -> (Pager, Catalog) {
        let file = NamedTempFile::new().unwrap();
        let mut pager = Pager::new(file.path().to_str().unwrap()).unwrap(); // trust me bro
        let catalog = Catalog::init(&mut pager).unwrap();

        (pager, catalog)
    }

    #[test]
    fn bind_basic_create() {
        let (_, catalog) = init_catalog();

        let create = Create {
            name: "test".into(),
            columns: vec![
                Definition { name: "col1".into(), ty: Type::Uint, primary: true, not_null: true },
                Definition { name: "col2".into(), ty: Type::Text, primary: false, not_null: false },
            ],
        };

        let bound = bind_create(create, &catalog).unwrap();
        let expected = BoundCreate {
            name: "test".into(),
            columns: vec![
                BoundDefinition { name: "col1".into(), ty: Type::Uint, nullable: false },
                BoundDefinition { name: "col2".into(), ty: Type::Text, nullable: true },
            ],
            primary_key: Some(vec![0])
        };

        assert_eq!(bound, expected)
    }

    #[test]
    fn bind_basic_drop() {
        let (mut pager, mut catalog) = init_catalog();
        let cols: &[(&str, Type, bool)] = &[
            ("col1", Type::Uint, true),
            ("col2", Type::Text, false),
        ];

        let schema = Schema::from_static(cols, 3);
        catalog.create_table("table", schema, &mut pager).unwrap();

        let drop = Drop { table: TableRef { name: "table".into(), alias: None } };

        let bound = bind_drop(drop, &catalog).unwrap();
        let expected = BoundDrop(3);

        assert_eq!(bound, expected)
    }
}
