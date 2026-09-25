use crate::{
    errors::{DbResult},
    store::pager::Pager,
};

trait Executor {
    fn next(&mut self, ctx: &mut ExecCtx) -> DbResult<Option<Tuple>>;
}

struct ExecCtx<'a> {
    pager: &'a mut Pager,
}

struct Tuple;
