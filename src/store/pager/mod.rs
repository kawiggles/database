pub mod page;

use crate::{
    VERSION,
    store::{
        pager::page::{
            Page, PageId, PageHeader, PageType, PAGE_SIZE ,
            FreePage, 
        },
    },
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

const DBHEADER_SIZE: usize = 3000;
pub struct DbHeader {
    magic: [u8; 8],
    version: u32,
    page_size: usize,
    root_page: Option<PageId>,
    order: usize,
    num_pages: usize,
    free_list_head: Option<PageId>,
}

impl DbHeader {
    fn deserialize(file: &mut File) -> StoreResult<Self> {
        let mut magic = [0u8; 8];
        file.read_exact(&mut magic)?;

        let version = read_u32(file)?;
        let page_size = read_usize(file)?;
        let root_page = PageId::new(read_usize(file)?);
        let order = read_usize(file)?;
        let num_pages = read_usize(file)?;
        let free_list_head = PageId::new(read_usize(file)?);

        Ok(DbHeader { magic, version, page_size, root_page, order, num_pages, free_list_head })
    }

    fn write(&self, file: &mut File) -> StoreResult<()> {
        let mut buf: Vec<u8>  = Vec::new();

        buf.extend_from_slice(&self.magic);
        buf.extend_from_slice(&self.version.to_le_bytes());
        buf.extend_from_slice(&self.page_size.to_le_bytes());

        if let Some(id) = self.root_page {
            buf.extend_from_slice(&id.get().to_le_bytes());
        } else {
            buf.extend_from_slice(&(0 as usize).to_le_bytes());
        }

        buf.extend_from_slice(&self.order.to_le_bytes());
        buf.extend_from_slice(&self.num_pages.to_le_bytes());

        if let Some(id) = self.free_list_head {
            buf.extend_from_slice(&id.get().to_le_bytes());
        } else {
            buf.extend_from_slice(&(0 as usize).to_le_bytes());
        }

        file.write_all(&buf)?;
        Ok(())
    }
}

pub struct Pager {
    pub file: File,
    free_list: Vec<PageId>,
    // TODO: figure this out
    dirty_cache: HashMap<PageId, Vec<u8>>,
    pub num_pages: usize,
}

impl Pager {
    // TODO: Why do I return this tuple?
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
            let header = PageHeader::read(id, &mut file)?;
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
        let header = PageHeader::read(id, &mut self.file)?;
        Ok(header)
    }

    // e.g. read::<DataPage>
    pub fn read<T: Page>(&mut self, id: PageId) -> StoreResult<T> {
        self.file.seek(SeekFrom::Start((id.get() * PAGE_SIZE) as u64));
        let mut buf = [0u8; PAGE_SIZE];
        self.file.read_exact(&mut buf);

        let page: T = T::deserialize(&buf)?;
        
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

pub fn read_usize(file: &mut File) -> StoreResult<usize> {
    let mut buf = [0u8; 8];
    file.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf) as usize)
}

pub fn read_u32(file: &mut File) -> StoreResult<u32> {
    let mut buf = [0u8; 4];
    file.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

pub fn read_u16(file: &mut File) -> StoreResult<u16> {
    let mut buf = [0u8; 2];
    file.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
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
