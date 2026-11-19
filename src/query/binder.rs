use crate::store::value::Value;

enum BoundExpr {
    Literal(Value),
    Column(usize),
}
