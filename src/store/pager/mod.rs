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
};

use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    str::from_utf8,
};
use log::{warn};

const MAGIC: [u8; 8] = *b"KAWIKADB";

pub struct Pager {
    pub file: File,
    free_list: Vec<PageId>,
    dirty_cache: HashMap<PageId, Vec<u8>>,
    pub num_pages: usize,
}

pub enum AnyPage {
    Leaf(LeafPage),
    Branch(BranchPage),
    Data(DataPage),
    OverFlow(OverflowPage),
    Free(FreePage),
}

impl AnyPage {
    /// Returns the PageType of the page.
    pub fn pagetype(&self) -> PageType {
        match self {
            AnyPage::Leaf(_) => PageType::Leaf,
            AnyPage::Branch(_) => PageType::Branch,
            AnyPage::Data(_) => PageType::Data,
            AnyPage::OverFlow(_) => PageType::Overflow,
            AnyPage::Free(_) => PageType::Free,
        }
    }

    /// Returns the PageId of the page.
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
    /// Initializes a new Pager, using file at 'path' or DEFAULT_FILE if empty.
    pub fn new(path: &str) -> StoreResult<Self> {
        warn!("Generating new pager at path {}", path);
        let new_head = DbHeader {
            magic: MAGIC,
            version: VERSION,
            page_size: PAGE_SIZE,
            num_pages: 1,
            free_list_head: None,
        };

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(if path.is_empty() { DEFAULT_FILE } else { path })?;
        new_head.write(&mut file)?;

        Ok(Pager {
                file,
                free_list: Vec::new(),
                dirty_cache: HashMap::new(),
                num_pages: 1,
        })
    }

    /// Opens and existing Pager, using file at 'path' or DEFAULT_FILE if empty.
    pub fn open(path: &str) -> StoreResult<Self> {
        warn!("Opening pager at path {}", path);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(if path.is_empty() { DEFAULT_FILE } else { path })?;
        let header = DbHeader::read(&mut file)?;

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

        Ok(Pager {
            file,
            free_list,
            dirty_cache: HashMap::new(),
            num_pages: header.num_pages,
        }) 
    }

    /// Read an ambiguous page, pattern match over PageType manually.
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

    /// Read a specific PageType T, e.g. read::<DataPage>(**id**).
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

    /// Like read(), but only returns the PageHeader.
    pub fn read_header(&mut self, id: PageId) -> StoreResult<PageHeader> {
        // This works because PageHeader is the same for all pages.
        // It can be used to bypass matching over read_any
        let bytes = match self.dirty_cache.get(&id) {
            Some(bytes) => bytes.as_slice(),
            None => &scan_page(id, &mut self.file)?,
        };
        let header = PageHeader::deserialize(&mut &bytes[..])?;
        Ok(header)
    }

    /// Write a page of PageType T to disk, e.g. write::<BranchPage>(**page**)
    pub fn write<T: Page>(&mut self, page: T) -> StoreResult<()> {
        let bytes = page.serialize()?;
        self.dirty_cache.insert(page.header().id, bytes);
        Ok(())
    }

    /// Clear out the dirty cache and write it to disk.
    pub fn flush(&mut self) -> StoreResult<()> {
        let cache = std::mem::take(&mut self.dirty_cache);

        for (id, page) in cache {
            self.file.seek(SeekFrom::Start((id.get() * PAGE_SIZE) as u64))?;
            self.file.write_all(&page)?;
        }
        Ok(())
    }

    /// Pulls a PageId from the free_list, or generates a new PageId.
    pub fn alloc(&mut self) -> PageId {
        if self.free_list.is_empty() {
            let id = PageId::new(self.num_pages)
                .expect("PAGEID OF 0 USED");
            self.num_pages += 1;
            id
        } else {
            let return_id = self.free_list.pop().unwrap();

            if let Some(new_tail_id) = self.free_list.last() {
                let mut tail = self.read::<FreePage>(*new_tail_id)
                    .expect("Non-FreePage found in free list");
                tail.sever();
                self.write::<FreePage>(tail).expect("Failed to write free_list tail");
            }

            return_id
        }
    }

    /// Delete a page by PageId, cache invalidation issues happen here.
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

    // Closes the Pager and the 
    pub fn close(&mut self) -> StoreResult<()> {
        warn!("Closing Pager");
        self.flush()?;
        let new_dbheader = DbHeader {
            magic: MAGIC,
            version: VERSION,
            page_size: PAGE_SIZE,
            num_pages: self.num_pages,
            free_list_head: self.free_list.first().copied(),
        };

        new_dbheader.write(&mut self.file)?;
        Ok(())
    }
}

pub struct DbHeader {
    pub magic: [u8; 8],
    pub version: u32,
    pub page_size: usize,
    pub num_pages: usize,
    pub free_list_head: Option<PageId>,
}

impl DbHeader {
    /// Read the database header from a file.
    pub fn read(file: &mut File) -> StoreResult<Self> {
        let mut magic = [0u8; 8];
        file.read_exact(&mut magic)?;

        let version = read_u32(file)?;
        let page_size = read_usize(file)?;
        let num_pages = read_usize(file)?;
        let free_list_head = PageId::new(read_usize(file)?);

        Ok(DbHeader{ magic, version, page_size, num_pages, free_list_head }) 
    }

    /// Write the database header to file.
    pub fn write(&self, file: &mut File) -> StoreResult<()> {
        let mut buf: Vec<u8>  = Vec::new();

        buf.extend_from_slice(&self.magic);
        buf.extend_from_slice(&self.version.to_le_bytes());
        buf.extend_from_slice(&self.page_size.to_le_bytes());

        buf.extend_from_slice(&self.num_pages.to_le_bytes());

        if let Some(id) = self.free_list_head {
            buf.extend_from_slice(&id.get().to_le_bytes());
        } else {
            buf.extend_from_slice(&(0 as usize).to_le_bytes());
        }

        file.seek(SeekFrom::Start(0))?;
        file.write_all(&buf)?;
        Ok(())
    }
}

/// Reads a single usize/u64 little-endian.
pub fn read_usize<R: Read>(bytes: &mut R) -> StoreResult<usize> {
    let mut buf = [0u8; 8];
    bytes.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf) as usize)
}

/// Reads a single u32 little-endian.
pub fn read_u32<R: Read>(bytes: &mut R) -> StoreResult<u32> {
    let mut buf = [0u8; 4];
    bytes.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

/// Reads a single u16 little-endian.
pub fn read_u16<R: Read>(bytes: &mut R) -> StoreResult<u16> {
    let mut buf = [0u8; 2];
    bytes.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

/// Reads a single byte.
pub fn read_byte<R: Read>(bytes: &mut R) -> StoreResult<u8> {
    let mut buf = [0u8; 1];
    bytes.read_exact(&mut buf)?;
    Ok(u8::from_le_bytes(buf))
}

/// Reads strings little-endian.
/// IMPORTANT: this works by reading the rest of the bytes from the slot and turning into a string.
/// It breaks immediately if the thing you're looking to read from has more than one string.
pub fn read_str<R: Read>(bytes: &mut R) -> StoreResult<String> {
    let mut buf: Vec<u8> = Vec::new();
    bytes.read_to_end(&mut buf)?;
    Ok(from_utf8(&buf)?.into())
}

/// Reads an entire page as a slice.
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
        let pager = Pager::new(path).unwrap();
        assert_eq!(pager.num_pages, 1);
    }

    #[test]
    fn open() {
        let tmp = temp_path();
        let path = tmp.path().to_str().unwrap();
        Pager::new(path).unwrap();

        let pager = Pager::open(path).unwrap();
        assert_eq!(pager.num_pages, 1);
    }

}
