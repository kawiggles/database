pub mod schema;
pub mod table;

use crate::{
    errors::{StoreResult, StoreErr},
    store::{Value, pager::PageId},
};

pub const RID_SIZE: usize = 10; // usize + u16 when serialized
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rid {
    pub page: PageId,
    pub slot: u16,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Row(pub Vec<Option<Value>>);

impl Row {

    /// Gets a raw value at index col
    pub fn field<T>(&self, col: usize) -> StoreResult<T> 
    where T: for<'a> TryFrom<&'a Value, Error = StoreErr> {
        self.0.get(col)
            .ok_or(StoreErr::BadColIndex(col))?
            .as_ref()
            .ok_or(StoreErr::NullField(col))?
            .try_into()
    }
}
