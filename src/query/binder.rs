use crate::{
    errors::{DbResult, UserErr::{self, TableNotInScope}}, query::ast::{
        BOp, Create, Delete, Drop, Expr, Insert, Literal, Op, Projection, Select, Statement, UOp, Update
    }, store::{
        catalog::Catalog, relation::{schema::{Type, Column}, table::Table}, value::Value,
    },
};

pub(super) enum BoundStatement {
    Create(BoundCreate),
    Drop(BoundDrop),
    Select(BoundSelect),
    Insert(BoundInsert),
    Update(BoundUpdate),
    Delete(BoundDelete),
}

#[derive(Debug, PartialEq)]
pub(super) struct BoundCreate {
    name: String,
    columns: Vec<BoundDefinition>,
    primary_key: Option<Vec<usize>>,
}

#[derive(Debug, PartialEq)]
pub(super) struct BoundDrop(usize);

#[derive(Debug, PartialEq)]
pub(super) struct BoundSelect {
    table: usize,
    projections: Vec<BoundProjection>,
    filter: Option<BoundExpr>
}

#[derive(Debug, PartialEq)]
pub(super) struct BoundInsert {
    table: usize,
    columns: Vec<usize>, // indicies of columns
    values: Vec<Vec<BoundExpr>>,
}

#[derive(Debug, PartialEq)]
pub(super) struct BoundUpdate {
    table: usize,
    assignments: Vec<(usize, BoundExpr)>, // (column index, value)
    filter: Option<BoundExpr>
}

#[derive(Debug, PartialEq)]
pub(super) struct BoundDelete {
    table: usize,
    filter: Option<BoundExpr>
}

#[derive(Debug, PartialEq)]
pub(super) enum BoundExpr {
    Column { table: usize, index: usize, ty: Type },
    Literal(Option<Value>),
    Unary { op: UOp, expr: Box<BoundExpr> },
    Binary { left: Box::<BoundExpr>, op: BOp, right: Box<BoundExpr> },
    IsNull { expr: Box<BoundExpr>, negated: bool }
}

impl BoundExpr {
    fn ty(&self) -> Option<Type> {
        match self {
            Self::Column { ty, .. } => Some(*ty),
            Self::Literal(v) => v.as_ref().map(Value::ty),
            Self::IsNull { .. } => Some(Type::Bool),
            Self::Unary { op: UOp::Not, .. } => Some(Type::Bool),
            Self::Unary { expr, .. } => expr.ty(),
            Self::Binary { op, left, right } => {
                match op {
                    BOp::Add | BOp::Sub | BOp::Mul | BOp::Div => left.ty().or(right.ty()),
                    _ => Some(Type::Bool),
                }
            }
        }
    }

    fn coerce_to(self, col: &Column) -> DbResult<Self> {
        match self {
            Self::Literal(None) if !col.nullable => Err(UserErr::NotNullCol)?,
            Self::Literal(Some(literal)) => {
                let from = literal.ty();
                literal.coerce(col.ty)
                    .map(|v| BoundExpr::Literal(Some(v)))
                    .ok_or_else(|| UserErr::TypeMismatch { left: from, right: col.ty }.into())
            },
            e if e.ty().is_none_or(|t| t == col.ty) => Ok(e),
            e => Err(UserErr::TypeMismatch { left: e.ty().unwrap(), right: col.ty })?,
        }
    }
}

#[derive(Debug, PartialEq)]
pub(super) struct Binder<'a> {
    table: &'a Table,
    name: &'a str,
}

impl Binder<'_> {
    fn resolve(&self, qualifier: Option<&str>, column_name: &str) -> DbResult<BoundExpr> {
        if let Some(qual) = qualifier && qual != self.name {
            return Err(UserErr::TableNotInScope(qual.into()))?;
        } else {
            let (index, column) = self.table.schema.find(column_name)
                .ok_or_else(|| UserErr::ColumnNotFound {
                    table: self.name.into(),
                    column: column_name.into(),
                })?;

            Ok(BoundExpr::Column { table: column.tid, index, ty: column.ty })
        }
    }

    fn bind_expr(&self, expr: Expr) -> DbResult<BoundExpr> {
        match expr {
            Expr::ColumnRef { qualifier, column } => self.resolve(qualifier.as_deref(), &column),
            Expr::Literal(literal) => Ok(bind_literal(literal)),
            Expr::Unary { operator, expr } => self.bind_unary(*expr, operator),
            Expr::Binary { left, operator, right } => self.bind_binary(*left, *right, operator),
            Expr::IsNull { expr, negated } => self.bind_is_null(*expr, negated),
        }
    }

    fn bind_unary(&self, expr: Expr, op: UOp) -> DbResult<BoundExpr> {
        let operand = self.bind_expr(expr)?;

        // Type checking
        match (&op, operand.ty()) {
            (_, None) => {},
            (UOp::Not, Some(Type::Bool)) => {},
            (UOp::Neg, Some(Type::Int | Type::Float)) => {},
            (UOp::Pos, Some(Type::Int | Type::Uint | Type::Float)) => {},
            (_, Some(ty)) => return Err(UserErr::BadOperand { op: Op::U(op), ty })?,
        }

        // Negative numbers and such
        match (op, operand) {
            (UOp::Neg, BoundExpr::Literal(Some(Value::Int(n)))) => {
                Ok(BoundExpr::Literal(Some(Value::Int(-n))))
            },
            (UOp::Neg, BoundExpr::Literal(Some(Value::Float(n)))) => {
                Ok(BoundExpr::Literal(Some(Value::Float(-n))))
            },
            (UOp::Not, BoundExpr::Literal(Some(Value::Bool(n)))) => {
                Ok(BoundExpr::Literal(Some(Value::Bool(!n))))
            },
            (UOp::Pos, operand) => Ok(operand),
            (op, operand) => Ok(BoundExpr::Unary { op, expr: Box::new(operand) })
        }
    }

    fn bind_binary(&self, left: Expr, right: Expr, op: BOp) -> DbResult<BoundExpr> {
        let (left, right) = unify(self.bind_expr(left)?, self.bind_expr(right)?)?;

        match (&op, left.ty().or(right.ty())) {
            (_, None) => {},
            (BOp::And | BOp::Or, Some(Type::Bool)) => {},
            (BOp::Eq | BOp::NotEq, Some(_)) => {},
            (BOp::Lt | BOp::Gt | BOp::LtEq | BOp::GtEq, Some(ty)) if ty != Type::Blob => {},
            (BOp::Add | BOp::Sub | BOp::Mul | BOp::Div, Some(Type::Int | Type::Uint | Type::Float)) => {},
            (_, Some(ty)) => return Err(UserErr::BadOperand { op: Op::B(op), ty })?,
        }

        Ok(BoundExpr::Binary { left: Box::new(left), op, right: Box::new(right) })
    }

    fn bind_is_null(&self, expr: Expr, negated: bool) -> DbResult<BoundExpr> {
        let operand = self.bind_expr(expr)?;
        Ok(BoundExpr::IsNull { expr: Box::new(operand), negated })
    }

    fn bind_wildcard(&self) -> impl Iterator<Item = BoundProjection> + '_ {
        self.table.schema.0.iter().enumerate()
            .filter(|(_, col)| !col.is_dead)
            .map(|(i, col)| {
                BoundProjection {
                    expr: BoundExpr::Column { table: col.tid, index: i, ty: col.ty },
                    name: col.name.clone(),
                }
            })
    }

    fn bind_filter(&self, expr: Expr) -> DbResult<BoundExpr> {
        let filter = self.bind_expr(expr)?;
        match filter.ty() {
            None | Some(Type::Bool) => Ok(filter),
            Some(_) => Err(UserErr::NonBoolFilter)?,
        }
    }
}

#[derive(Debug, PartialEq)]
pub(super) struct BoundDefinition {
    name: String,
    ty: Type,
    nullable: bool,
}

#[derive(Debug, PartialEq)]
pub(super) struct BoundProjection {
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
    let binder = Binder {
        table: catalog.lookup_by_name(&statement.table.name)?,
        name: &statement.table.alias.unwrap_or(statement.table.name),
    };

    let mut projections: Vec<BoundProjection> = Vec::new();
    for projection in statement.projections {
        match projection {
            Projection::Wildcard => projections.extend(binder.bind_wildcard()),
            Projection::QualifiedWildcard(qualifier) => {
                if qualifier != binder.name { return Err(TableNotInScope(qualifier))?; }
                projections.extend(binder.bind_wildcard());
            },
            Projection::Expr { expr, alias } => {
                let name = alias.unwrap_or_else(|| expr.default_name());
                projections.push(BoundProjection { expr: binder.bind_expr(expr)?, name });
            },
        }
    }

    let filter = statement.filter.map(|expr| binder.bind_filter(expr)).transpose()?;

    Ok(BoundSelect { table: binder.table.tid, projections, filter })
}

fn bind_insert(statement: Insert, catalog: &Catalog) -> DbResult<BoundInsert> {
    let binder = Binder {
        table: catalog.lookup_by_name(&statement.table.name)?,
        name: &statement.table.alias.unwrap_or(statement.table.name),
    };

    let mut columns = Vec::new();
    for name in statement.columns {
        if let Some(column) = binder.table.schema.find(&name) {
            columns.push(column);
        } else {
            return Err(UserErr::ColumnNotFound {
                column: name.into(),
                table: binder.table.name.clone()
            })?;
        }
    }

    let mut values = Vec::new();
    for row in statement.values {
        if row.len() != columns.len() {
            return Err(UserErr::ColValMismatch)?;
        }

        // type checking
        values.push(row.into_iter()
            .zip(&columns)
            .map(|(expr, (_, col))| { binder.bind_expr(expr)?.coerce_to(col) })
            .collect::<DbResult<Vec<_>>>()?);
    }
    
    Ok(BoundInsert {
        table: binder.table.tid,
        columns: columns.iter().map(|c| { c.0 }).collect(),
        values
    })
}

fn bind_update(statement: Update, catalog: &Catalog) -> DbResult<BoundUpdate> {
    todo!()
}

fn bind_delete(statement: Delete, catalog: &Catalog) -> DbResult<BoundDelete> {
    todo!()
}

fn bind_literal(literal: Literal) -> BoundExpr {
    match literal {
        Literal::Str(string) => BoundExpr::Literal(Some(Value::Text(string))),
        Literal::Int(int) => BoundExpr::Literal(Some(Value::Int(int))),
        Literal::Float(float) => BoundExpr::Literal(Some(Value::Float(float))),
        Literal::Bool(b) => BoundExpr::Literal(Some(Value::Bool(b))),
        Literal::Null => BoundExpr::Literal(None),
    }
}

fn unify(left: BoundExpr, right: BoundExpr) -> DbResult<(BoundExpr, BoundExpr)> {
    let (lt, rt) = match (left.ty(), right.ty()) {
        (Some(lt), Some(rt)) if lt != rt => (lt, rt),
        _ => return Ok((left, right))
    };

    let mismatch = || UserErr::TypeMismatch { left: lt, right: rt };

    // for edge case, try lt to rt and rt to lt, currently fails sometimes for lt to rt
    match (left, right) {
        (BoundExpr::Literal(Some(v)), right) => {
            let v = v.coerce(rt).ok_or_else(mismatch)?;
            Ok((BoundExpr::Literal(Some(v)), right))
        },
        (left, BoundExpr::Literal(Some(v))) => {
            let v = v.coerce(lt).ok_or_else(mismatch)?;
            Ok((left, BoundExpr::Literal(Some(v))))
        },
        _ => Err(mismatch())?,
    }
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
