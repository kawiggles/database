use crate::{
    errors::{DbResult},
    store::{
        Store,
        relation::Row,
    },
};

trait Executor {
    fn next(&mut self, store: &mut Store) -> DbResult<Option<Row>>;
}

