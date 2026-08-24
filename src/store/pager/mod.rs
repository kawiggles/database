pub mod page;
pub mod dbheader;
pub mod utils;
pub mod page_id;
pub mod branch;
pub mod leaf;
pub mod data;
pub mod overflow;
pub mod free;

pub use dbheader::DbHeader;
pub use utils::{ read_u16, read_u32, read_usize, read_str, scan_page };
pub use page_id::PageId;
pub use page::{Page, PageHeader, PageType, PageCursor, PAGE_SIZE };
pub use free::FreePage;
pub use overflow::OverflowPage;
pub use data::DataPage;
pub use leaf::LeafPage;
pub use branch::BranchPage;

use crate::{
    VERSION,
    errors::{StoreErr, StoreResult},
    tcp::DEFAULT_FILE,
    store::{ Rid, Value },
};

use std::{
    fs::{File, OpenOptions},
    io::{Write, Seek, SeekFrom },
    collections::HashMap,
};
use log::{info};

const MAGIC: [u8; 8] = *b"KAWIKADB";

// TODO: Figure out what this is
pub struct Oid(pub usize);


pub struct Pager {
    pub file: File,
    free_list: Vec<PageId>,
    dirty_cache: HashMap<PageId, Vec<u8>>,
    pub num_pages: usize,
    pub active_data: PageId,
}

pub enum AnyPage {
    Leaf(LeafPage),
    Branch(BranchPage),
    Data(DataPage),
    OverFlow(OverflowPage),
    Free(FreePage),
}

impl AnyPage {
    pub fn to_pagetype(&self) -> PageType {
        match self {
            AnyPage::Leaf(_) => PageType::Leaf,
            AnyPage::Branch(_) => PageType::Branch,
            AnyPage::Data(_) => PageType::Data,
            AnyPage::OverFlow(_) => PageType::Overflow,
            AnyPage::Free(_) => PageType::Free,
        }
    }
}

impl Pager {
    pub fn new(path: &str, new_order: usize) -> StoreResult<(Self, Option<PageId>, usize)> {
        let filepath = if path.is_empty() { DEFAULT_FILE } else { path };

        let new_head = DbHeader {
            magic: MAGIC,
            version: VERSION,
            page_size: PAGE_SIZE,
            root_page: None, // None means no root (no dip, me)
            order: new_order,
            num_pages: 1,
            free_list_head: None,
        };

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(filepath)?;
        new_head.write(&mut file)?;

        let active_data = DataPage::new().header().id;

        Ok((
            Pager { file, free_list: Vec::new(), dirty_cache: HashMap::new(), num_pages: 1, active_data },
            new_head.root_page,
            new_head.order
        ))
    }

    pub fn open(path: &str) -> StoreResult<(Self, Option<PageId>, usize)> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;
        let header = DbHeader::deserialize(&mut file)?;

        if header.magic != MAGIC {
            return Err(StoreErr::BadFile);
        }

        let mut free_list: Vec<PageId> = Vec::new();
        let mut current = header.free_list_head;
        while let Some(id) = current {
            free_list.push(id);
            let bytes = scan_page(id, &mut file)?;
            let header = PageHeader::deserialize(&mut &bytes[..])?;
            current = header.next;
        }

        Ok((Pager {
            file: file,
            free_list: free_list,
            dirty_cache: HashMap::new(),
            num_pages: header.num_pages,
        }, header.root_page, header.order))
    }

    pub fn read_any(&mut self, id: PageId) -> StoreResult<AnyPage> {
        let bytes = match self.dirty_cache.get(&id) {
            Some(bytes) => bytes.as_slice(),
            None => &scan_page(id, &mut self.file)?,
        };

        let mut cursor = PageCursor::new(&bytes);
        let header = PageHeader::deserialize(&mut &bytes[..])?;
        match header.pagetype {
            PageType::Leaf => Ok(AnyPage::Leaf(LeafPage::deserialize(header, &mut cursor)?)),
            PageType::Branch => Ok(AnyPage::Branch(BranchPage::deserialize(header, &mut cursor)?)),
            PageType::Data => Ok(AnyPage::Data(DataPage::deserialize(header, &mut cursor)?)),
            PageType::Overflow => {
                Ok(AnyPage::OverFlow(OverflowPage::deserialize(header, &mut cursor)?))
            },
            PageType::Free => Ok(AnyPage::Free(FreePage::deserialize(header, &mut cursor)?)),
        }
    }

    // e.g. read::<DataPage>
    pub fn read<T: Page>(&mut self, id: PageId) -> StoreResult<T> {
        let bytes = match self.dirty_cache.get(&id) {
            Some(bytes) => bytes.as_slice(),
            None => &scan_page(id, &mut self.file)?,
        };

        let mut cursor = PageCursor::new(bytes);
        let header = PageHeader::deserialize(&mut &bytes[..])?;

        if header.pagetype != T::pagetype() {
            return Err(StoreErr::UnexpectedPagetype { 
                found: header.pagetype,
                expected: T::pagetype()
            });
        }

        T::deserialize(header, &mut cursor)
    }

    pub fn write<T: Page>(&mut self, page: T) -> StoreResult<()> {
        let bytes = page.serialize()?;
        self.dirty_cache.insert(page.header().id, bytes);
        Ok(())
    }

    pub fn insert_data(&mut self, val: Value) -> StoreResult<Rid> {
        let current_page = self.read::<DataPage>(self.active_data)?;
        let free_space = current_page.header().upper - current_page.header().lower;
        let page = if free_space > val.to_bytes().len() as u16 {
            let new_data_id = self.alloc();
            // TODO: create new data page
            // TODO: if doesn't fit in new page, create overflow page
            self.active_data = new_data_id;
            new_data_id
        } else {
            self.active_data
        };

        let mut data_page = self.read::<DataPage>(page)?;
        Ok(data_page.insert(val)?)
    }

    // Clear out the cache and write it to disk
    pub fn flush(&mut self) -> StoreResult<()> {
        let cache = std::mem::take(&mut self.dirty_cache);

        for (id, page) in cache {
            self.file.seek(SeekFrom::Start((id.get() * PAGE_SIZE) as u64));
            self.file.write_all(&page)?;
        }
        Ok(())
    }

    // Construct page and serialize it
    pub fn alloc(&mut self) -> PageId {
        if self.free_list.is_empty() {
            let id = PageId::new(self.num_pages)
                .expect("PAGEID OF 0 USED");
            self.num_pages += 1;
            id
        } else {
            self.free_list.pop().unwrap()
        }
    }

    // Delete a page, cache invalidation issues happen here
    pub fn free(&mut self, id: PageId) -> StoreResult<()> {
        if let Some(prev_id) = self.free_list.last().copied() {
            let new_header = PageHeader {
                id: prev_id,
                pagetype: PageType::Free,
                next: Some(id),
                slots: 0,
                lower: 0,
                upper: 0,
            };
            self.write::<FreePage>(prev_id, FreePage(new_header));
        }

        self.write::<FreePage>(id, FreePage::new(id));
        self.free_list.push(id);
        self.dirty_cache.remove(&id);
        Ok(())
    }

    pub fn close(&mut self, root: Option<PageId>, order: usize) -> StoreResult<()> {
        info!(" - Pager is closing...");
        self.flush()?;
        let new_dbheader = DbHeader {
            magic: MAGIC,
            version: VERSION,
            page_size: PAGE_SIZE,
            root_page: root,
            order: order,
            num_pages: self.num_pages,
            free_list_head: self.free_list.first().copied(),
        };

        new_dbheader.write(&mut self.file)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn temp_path() -> NamedTempFile {
        NamedTempFile::new().unwrap()
    }

    #[test]
    fn pager_new() {
        let tmp = temp_path();
        let path = tmp.path().to_str().unwrap();
        let (pager, root, _) = Pager::new(path, 4).unwrap();
        assert!(root.is_none());
        assert_eq!(pager.num_pages, 1);
    }

    #[test]
    fn pager_open() {
        let tmp = temp_path();
        let path = tmp.path().to_str().unwrap();
        Pager::new(path, 4).unwrap();

        let (pager, root, _) = Pager::open(path).unwrap();
        assert!(root.is_none());
        assert_eq!(pager.num_pages, 1);
    }

}
