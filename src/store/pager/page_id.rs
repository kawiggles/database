use std::{
    num::NonZeroUsize, u8,
    fmt::{Display, Formatter, Result},
};

#[derive(Eq, Hash, PartialEq, Clone, Copy, Debug)]
pub struct PageId(pub NonZeroUsize);

impl PageId {
    pub fn new(offset: usize) -> Option<Self> {
        NonZeroUsize::new(offset).map(PageId)
    }

    pub fn get(self) -> usize {
        self.0.get()
    }
}

impl Display for PageId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.0)
    }
}
