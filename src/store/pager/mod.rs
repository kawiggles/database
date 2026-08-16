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
pub use utils::{ read_u16, read_u32, read_usize, scan_page };
pub use page_id::PageId;
pub use page::{Page, PageHeader, PageType, PAGE_SIZE};
pub use free::FreePage;
pub use overflow::OverflowPage;
pub use data::DataPage;
pub use leaf::LeafPage;
pub use branch::BranchPage;

use crate::{
    VERSION,
    errors::{StoreErr, StoreResult},
    tcp::DEFAULT_FILE,
};

use std::{
    fs::{File, OpenOptions},
    io::{Write, Read, Seek, SeekFrom },
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

        Ok((
            Pager { file, free_list: Vec::new(), dirty_cache: HashMap::new(), num_pages: 1, },
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

    pub fn read_header(&mut self, id: PageId) -> StoreResult<PageHeader> {
        let bytes = scan_page(id, &mut self.file)?;
        let header = PageHeader::deserialize(&mut &bytes[..])?;
        Ok(header)
    }

    // e.g. read::<DataPage>
    pub fn read<T: Page>(&mut self, id: PageId) -> StoreResult<T> {
        let page: T = T::deserialize(&scan_page(id, &mut self.file)?)?;
        
        if page.header().pagetype != T::pagetype() {
            return Err(StoreErr::UnexpectedPagetype { 
                found: page.header().pagetype,
                expected: T::pagetype()
            });
        }

        Ok(page)
    }

    pub fn write<T: Page>(&mut self, id: PageId, page: T) {
        let bytes = page.serialize();
        self.file.seek(SeekFrom::Start((id.get() * PAGE_SIZE) as u64));
        self.file.write_all(&bytes);
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
                table_oid: Oid(0),
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
