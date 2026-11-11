use crate::store::value::Value;
use super::{
    Expr, BOp,
};

enum BoundExpr {
    Literal(Value),
    Column(usize),
    Binary { op: BOp, l: Box<BoundExpr>, r: Box<BoundExpr> },
}
