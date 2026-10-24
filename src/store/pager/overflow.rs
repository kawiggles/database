use super::{
    Page, PageId, PageType, PageHeader, Pager,
    data::MAX_FREE_SPACE,
    page::{PageCursor, PAGE_SIZE, PAGEID_SIZE, PAGEHEADER_SIZE, SLOT_POINTER_SIZE},
    read_usize,
};
use crate::errors::{StoreResult, StoreErr};

pub struct OverflowPage {
    header: PageHeader,
    next: Option<PageId>,
    data: Vec<u8>,
}

impl OverflowPage {
    pub fn new(id: PageId) -> Self {
        let header = PageHeader {
            id,
            pagetype: PageType::Overflow,
            slots: 1,
            lower: (PAGEHEADER_SIZE + SLOT_POINTER_SIZE) as u16,
            upper: (PAGE_SIZE - PAGE_SIZE) as u16,
        };

        Self { header, next: None, data: Vec::new() }
    }

    pub fn fill(&mut self, bytes: &[u8], pager: &mut Pager) -> StoreResult<()> {
        todo!()
    }

    fn refresh_header(&mut self) {
        self.header.slots = 2;
        self.header.lower = (PAGEHEADER_SIZE + 2 * SLOT_POINTER_SIZE) as u16;
        self.header.upper = PAGE_SIZE
            .checked_sub(self.data.len())
            .and_then(|v| v.checked_sub(PAGEID_SIZE))
            .unwrap_or(0) as u16;
    }
}

impl Page for OverflowPage {
    fn header(&self) -> &PageHeader {
        &self.header
    }

    fn set_id(&mut self, id: PageId) {
        self.header.id = id;
    }

    fn pagetype() -> PageType {
        PageType::Overflow
    }

    fn serialize(&self) -> StoreResult<Vec<u8>> {
        let mut bytes = vec![0u8; PAGE_SIZE];
        bytes[0..PAGEHEADER_SIZE].clone_from_slice(&self.header.serialize());

        let len = PAGE_SIZE - (self.data.len() + PAGEID_SIZE);
        let dir = PAGEHEADER_SIZE;
        if len < dir + PAGEID_SIZE {
            return Err(StoreErr::SlotOverwrite {
                page: self.header.id,
                len: self.data.len(),
                pagetype: PageType::Overflow
            });
        }

        let next = self.next
            .map(|id| id.get())
            .unwrap_or(0)
            .to_le_bytes();

        bytes[PAGE_SIZE-PAGEID_SIZE..PAGE_SIZE].copy_from_slice(&next);
        bytes[len..PAGE_SIZE-PAGEID_SIZE].copy_from_slice(&self.data);

        bytes[dir..dir+2].copy_from_slice(&((PAGE_SIZE - PAGEID_SIZE) as u16).to_le_bytes());
        bytes[dir+2..dir+4].copy_from_slice(&(PAGEID_SIZE as u16).to_le_bytes());
        bytes[dir+4..dir+6].copy_from_slice(&((PAGE_SIZE - PAGEID_SIZE - len) as u16).to_le_bytes());
        bytes[dir+6..dir+8].copy_from_slice(&(len as u16).to_le_bytes());

        Ok(bytes)
    }

    fn deserialize(header: PageHeader, cursor: &mut PageCursor) -> StoreResult<Self> {
        let next = PageId::new(read_usize(&mut cursor.next()?)?);
        let data = cursor.next()?.to_vec();

        Ok(Self { header, next, data })
    }
}

