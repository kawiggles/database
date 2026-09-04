pub mod page;
pub mod branch;
pub mod leaf;
pub mod data;
pub mod overflow;
pub mod free;

pub use page::{Page, PageHeader, PageType, PageCursor, PageId, PAGE_SIZE };
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
    collections::HashMap, fs::{File, OpenOptions}, io::{Read, Seek, SeekFrom::{self, Start}, Write}, str::from_utf8,
};
use log::{info};

const MAGIC: [u8; 8] = *b"KAWIKADB";

pub struct Pager {
    pub file: File,
    free_list: Vec<PageId>,
    dirty_cache: HashMap<PageId, Vec<u8>>,
    pub num_pages: usize,
    pub active_data: Option<PageId>,
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

    pub fn id(&self) -> PageId {
        match self {
            AnyPage::Leaf(l) => l.header().id,
            AnyPage::Branch(b) => b.header().id,
            AnyPage::Data(d) => d.header().id,
            AnyPage::OverFlow(o) => o.header().id,
            AnyPage::Free(f) => f.header().id,
        }
    }
}

impl Pager {
    pub fn new(path: &str) -> StoreResult<(Self, Option<PageId>)> {
        let filepath = if path.is_empty() { DEFAULT_FILE } else { path };

        let new_head = DbHeader {
            magic: MAGIC,
            version: VERSION,
            page_size: PAGE_SIZE,
            root_page: None, // None means no root (no dip, me)
            num_pages: 1,
            free_list_head: None,
            active_data: None,
        };

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(filepath)?;
        new_head.write(&mut file)?;

        Ok((Pager {
                file,
                free_list: Vec::new(),
                dirty_cache: HashMap::new(),
                num_pages: 1,
                active_data: None,
            },
            new_head.root_page
        ))
    }

    pub fn open(path: &str) -> StoreResult<(Self, Option<PageId>)> {
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
            file,
            free_list,
            dirty_cache: HashMap::new(),
            num_pages: header.num_pages,
            active_data: header.active_data,
        }, header.root_page))
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
            return Err(StoreErr::UnexpectedPagetype(header.pagetype));
        }

        T::deserialize(header, &mut cursor)
    }

    pub fn read_header(&mut self, id: PageId) -> StoreResult<PageHeader> {
        let bytes = match self.dirty_cache.get(&id) {
            Some(bytes) => bytes.as_slice(),
            None => &scan_page(id, &mut self.file)?,
        };
        let header = PageHeader::deserialize(&mut &bytes[..])?;
        Ok(header)
    }

    pub fn write<T: Page>(&mut self, page: T) -> StoreResult<()> {
        let bytes = page.serialize()?;
        self.dirty_cache.insert(page.header().id, bytes);
        Ok(())
    }

    // TODO: get rid of this nonsense and put it in datapage
    // Checks the current active data page's free space
    // If free space, return the active data page and next slot 
    // If not, alloc a new page id and return 1st slot
    // ALERT: YOU NEED TO ACTUALLY CREATE THIS DATA PAGE LATER
    pub fn check_active_data_space(&mut self, val: Value) -> StoreResult<Rid> {
        if let Some(pageid) = self.active_data {
            let page = self.read::<DataPage>(pageid)?;
            let free_space = page.header().upper - page.header().lower;
            if free_space > val.to_bytes().len() as u16 {
                let new_data_id = self.alloc();
                self.active_data = Some(new_data_id);
                Ok(Rid { page: new_data_id, slot: 1})
            } else {
                Ok(Rid { page: page.header().id, slot: page.header().slots + 1 })
            }
        } else {
            todo!();
            // TODO: create a new data page
        } 
    }

    // Clear out the cache and write it to disk
    pub fn flush(&mut self) -> StoreResult<()> {
        let cache = std::mem::take(&mut self.dirty_cache);

        for (id, page) in cache {
            self.file.seek(SeekFrom::Start((id.get() * PAGE_SIZE) as u64))?;
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
            self.write::<FreePage>(FreePage(new_header))?;
        }

        self.write::<FreePage>(FreePage::new(id))?;
        self.free_list.push(id);
        Ok(())
    }

    pub fn close(&mut self, root: Option<PageId>) -> StoreResult<()> {
        info!(" - Pager is closing...");
        self.flush()?;
        let new_dbheader = DbHeader {
            magic: MAGIC,
            version: VERSION,
            page_size: PAGE_SIZE,
            root_page: root,
            num_pages: self.num_pages,
            free_list_head: self.free_list.first().copied(),
            active_data: self.active_data,
        };

        new_dbheader.write(&mut self.file)?;
        Ok(())
    }
}

pub struct DbHeader {
    pub magic: [u8; 8],
    pub version: u32,
    pub page_size: usize,
    pub root_page: Option<PageId>,
    pub num_pages: usize,
    pub free_list_head: Option<PageId>,
    pub active_data: Option<PageId>,
}

impl DbHeader {
    pub fn deserialize(file: &mut File) -> StoreResult<Self> {
        let mut magic = [0u8; 8];
        file.read_exact(&mut magic)?;

        let version = read_u32(file)?;
        let page_size = read_usize(file)?;
        let root_page = PageId::new(read_usize(file)?);
        let num_pages = read_usize(file)?;
        let free_list_head = PageId::new(read_usize(file)?);
        let active_data = PageId::new(read_usize(file)?);

        Ok(DbHeader{ magic, version, page_size, root_page, num_pages, free_list_head, active_data }) 
    }

    pub fn write(&self, file: &mut File) -> StoreResult<()> {
        let mut buf: Vec<u8>  = Vec::new();

        buf.extend_from_slice(&self.magic);
        buf.extend_from_slice(&self.version.to_le_bytes());
        buf.extend_from_slice(&self.page_size.to_le_bytes());

        if let Some(id) = self.root_page {
            buf.extend_from_slice(&id.get().to_le_bytes());
        } else {
            buf.extend_from_slice(&(0 as usize).to_le_bytes());
        }

        buf.extend_from_slice(&self.num_pages.to_le_bytes());

        if let Some(id) = self.free_list_head {
            buf.extend_from_slice(&id.get().to_le_bytes());
        } else {
            buf.extend_from_slice(&(0 as usize).to_le_bytes());
        }

        if let Some(id) = self.active_data {
            buf.extend_from_slice(&id.get().to_le_bytes());
        } else {
            buf.extend_from_slice(&(0 as usize).to_le_bytes());
        }

        file.seek(SeekFrom::Start(0))?;
        file.write_all(&buf)?;
        Ok(())
    }
}

pub fn read_usize<R: Read>(bytes: &mut R) -> StoreResult<usize> {
    let mut buf = [0u8; 8];
    bytes.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf) as usize)
}

pub fn read_u32<R: Read>(bytes: &mut R) -> StoreResult<u32> {
    let mut buf = [0u8; 4];
    bytes.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

pub fn read_u16<R: Read>(bytes: &mut R) -> StoreResult<u16> {
    let mut buf = [0u8; 2];
    bytes.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

// IMPORTANT: this works by reading the rest of the bytes from the slot and turning into a string
// It breaks immediately if the thing you're looking to read from has more than one string
pub fn read_str<R: Read>(bytes: &mut R) -> StoreResult<String> {
    let mut buf: Vec<u8> = Vec::new();
    bytes.read_to_end(&mut buf)?;
    Ok(from_utf8(&buf)?.into())
}

pub fn scan_page(id: PageId, file: &mut File) -> StoreResult<[u8; PAGE_SIZE]> {
    file.seek(SeekFrom::Start((id.get() * PAGE_SIZE) as u64))?;
    let mut buf = [0u8; PAGE_SIZE];
    file.read_exact(&mut buf)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn temp_path() -> NamedTempFile {
        NamedTempFile::new().unwrap()
    }

    #[test]
    fn new() {
        let tmp = temp_path();
        let path = tmp.path().to_str().unwrap();
        let (pager, root) = Pager::new(path).unwrap();
        assert!(root.is_none());
        assert_eq!(pager.num_pages, 1);
    }

    #[test]
    fn open() {
        let tmp = temp_path();
        let path = tmp.path().to_str().unwrap();
        Pager::new(path).unwrap();

        let (pager, root) = Pager::open(path).unwrap();
        assert!(root.is_none());
        assert_eq!(pager.num_pages, 1);
    }

}
