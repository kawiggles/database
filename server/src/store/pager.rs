use crate::VERSION;
use crate::store::{DEFAULT_ORDER};
use crate::store::value::Value;
use crate::logs::{StoreErr};
use crate::tcp::DEFAULT_FILE;

use bincode_next::{config, Encode, Decode};
use std::fs::{File, OpenOptions};
use std::os::unix::fs::FileExt;
use std::io::{Write, Read, Seek, SeekFrom, BufReader};
use std::fmt;
use std::collections::HashMap;

// Pager Constants
//__________________________________________________________________________________________________

// TODO: replace PAGE_SIZE instances with the page_size metadata for choices of page sizes
pub const PAGE_SIZE: usize = 4096;

const INDEX_CONFIG: config::Configuration<config::BigEndian, config::Fixint, config::Limit<4096>> = 
    config::standard()
    .with_big_endian()
    .with_fixed_int_encoding()
    .with_limit::<PAGE_SIZE>();

const DATA_CONFIG: config::Configuration<config::BigEndian, config::Varint, config::Limit<4096>> = 
    config::standard()
    .with_big_endian()
    .with_limit::<4096>();

const MAGIC: [u8; 8] = *b"KAWIKADB";

// Page Trait
//__________________________________________________________________________________________________

pub trait Page: Sized {
    fn page_id(&self) -> PageId;
    fn write(pager: &mut Pager, page: Self) -> Result<(), StoreErr>;
    fn read(pager: &Pager, id: PageId) -> Result<Self, StoreErr>;
}

pub fn write_page(file: &mut File, id: PageId, buf: &[u8; PAGE_SIZE]) -> Result<(), StoreErr> {
    file.seek(SeekFrom::Start((id.0 * PAGE_SIZE) as u64))?;
    file.write_all(buf)?;
    Ok(())
}

#[derive(Eq, Hash, PartialEq, Encode, Decode, Clone, Copy, Debug)]
pub struct PageId(pub usize);

impl fmt::Display for PageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// TODO: Make data representation more efficient (manual bincode)
#[derive(Encode, Decode, Clone)]
pub struct PageHeader {
    pub page_type: PageType, // 4 bytes
    pub page_id: PageId, // 8 bytes
    pub next_free: Option<PageId>, // 8 bytes (Option) + 8 bytes
    pub next_over: Option<PageId>, // 16 Bytes
} // Total: 44 bytes

#[derive(Encode, Decode, Clone)]
pub enum PageType {
    Index,
    Data,
}

#[derive(Encode, Decode, Clone)]
pub struct IndexPage {
    pub header: PageHeader, // 44 bytes
    pub keys: Vec<String>, // 8 + ? Bytes
    pub node_type: NodeType, // 28 + ? Bytes
                             // Max order is 251 (trust this math) with PAGE_SIZE 4096
}

impl IndexPage {
    pub fn new_leaf(id: PageId, keys: Vec<String>, ids: Vec<PageId>, next: Option<PageId>) -> Self {
        IndexPage { 
            header: PageHeader {
                page_type: PageType::Index,
                page_id: id,
                next_free: None,
                next_over: None,
            },
            keys: keys,
            node_type: NodeType::Leaf {
                pages: ids,
                next: next,
            }
        }
    }

    pub fn new_branch(id: PageId, keys: Vec<String>, ids: Vec<PageId>) -> Self {
        IndexPage { 
            header: PageHeader {
                page_type: PageType::Index,
                page_id: id,
                next_free: None,
                next_over: None,
            },
            keys: keys,
            node_type: NodeType::Branch {
                children: ids,
            }
        }
    }
}

impl Page for IndexPage {
    fn page_id(&self) -> PageId {
        self.header.page_id
    }

    fn write(pager: &mut Pager, new_page: Self) -> Result<(), StoreErr> {
        pager.dirty_cache.insert(new_page.header.page_id, new_page);
        Ok(())
    }

    fn read(pager: &Pager, id: PageId) -> Result<Self, StoreErr> {
        if let Some(cached) = pager.dirty_cache.get(&id) {
            return Ok(cached.clone());
        }

        let file = &pager.file;
        let mut buf = [0u8; PAGE_SIZE];
        file.read_at(&mut buf, (id.0 * PAGE_SIZE) as u64)?;
        let (page, size): (IndexPage, usize) = bincode_next::decode_from_slice(&buf, INDEX_CONFIG)?;

        if size > PAGE_SIZE {
            return Err(StoreErr::ReadOverflow)
        }

        Ok(page)
    }
}

#[derive(Encode, Decode, Clone)] // Trust this math
pub enum NodeType { // 4 bytes
    Branch { children: Vec<PageId> }, // 8 bytes + ? 8 byte PageIds
    Leaf { 
        pages: Vec<PageId>, // 8 bytes + ? 8 byte PageIds
        next: Option<PageId> // 8 bytes (Option) + 8 bytes
    } // 28 + 8? Bytes
} // max 28 + 8? Bytes where "?" is number of keys

#[derive(Encode, Decode)]
pub struct DataPage {
    pub header: PageHeader, // 44 bytes
    pub value: Value,
}

impl DataPage {
    pub fn new(pager: &mut Pager, val: Value) -> Result<PageId, StoreErr> {
        let id = pager.alloc();
        let page = DataPage {
            header: PageHeader {
                page_type: PageType::Data,
                page_id: id,
                next_free: None,
                next_over: None, // writeover logic could happen at later point
            },
            value: val,
        };
        Page::write(pager, page)?;
        Ok(id)
    }
}

impl Page for DataPage {
    fn page_id(&self) -> PageId {
        self.header.page_id
    }

    fn write(pager: &mut Pager, new_page: Self) -> Result<(), StoreErr> {
        let file = &mut pager.file;
        let mut page = [0u8; PAGE_SIZE];
        bincode_next::encode_into_slice(&new_page, &mut page, DATA_CONFIG)?;
        write_page(file, new_page.header.page_id, &page)?;
        Ok(())
    }

    fn read(pager: &Pager, id: PageId) -> Result<DataPage, StoreErr> {
        let file = &pager.file;
        let mut buf = [0u8; PAGE_SIZE];
        file.read_at(&mut buf, (id.0 * PAGE_SIZE) as u64)?;
        let (page, size): (DataPage, usize) = bincode_next::decode_from_slice(&buf, DATA_CONFIG)?;

        if size > PAGE_SIZE {
            return Err(StoreErr::ReadOverflow)
        }

        Ok(page)
    }
}

#[derive(Encode, Decode)]
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
    fn write(&self, file: &mut File) -> Result<(), StoreErr> {
        let mut page = [0u8; PAGE_SIZE];
        bincode_next::encode_into_slice(self, &mut page, INDEX_CONFIG)?;
        write_page(file, PageId(0), &page)?;
        Ok(())
    }
}

pub struct Pager {
    pub file: File,
    free_list: Vec<PageId>,
    dirty_cache: HashMap<PageId, IndexPage>,
    pub num_pages: usize,
}

#[derive(Decode)]
struct FreeListRead {
    _page_type: PageType,
    _page_id: PageId,
    next_free: Option<PageId>,
}

// TODO: write logs...
impl Pager {
    // Function to create a new database file if none exists
    pub fn new(path: &str) -> Result<(Self, Option<PageId>, usize), StoreErr> {
        let filepath = {
            if path.is_empty() {
                DEFAULT_FILE
            } else {
                path
            }
        };

        let new_head = DbHeader{
            magic: MAGIC,
            version: VERSION,
            page_size: PAGE_SIZE,
            root_page: None, // None means no root
            // TODO: add way to change db order (requires variable page size)
            order: DEFAULT_ORDER,
            num_pages: 1,
            free_list_head: None,
        };

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(filepath)?;
        new_head.write(&mut file)?;

        Ok((Pager {
            file: file,
            free_list: Vec::new(),
            dirty_cache: HashMap::new(),
            num_pages: 1,
        }, new_head.root_page, new_head.order))
    }

    // Function to open database if one exists
    pub fn open(path: &str) -> Result<(Self, Option<PageId>, usize), StoreErr> {
        // TODO: if no path, use default file (consider Option<&str>)
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;
        let mut reader = BufReader::new(&mut file);
        let header: DbHeader = bincode_next::decode_from_std_read(&mut reader, INDEX_CONFIG)?;

        if header.magic != MAGIC {
            return Err(StoreErr::BadFile);
        }

        let mut free_list: Vec<PageId> = Vec::new();
        let mut current = header.free_list_head;
        while let Some(id) = current {
            free_list.push(id);
            let mut buf = [0u8; PAGE_SIZE];
            file.seek(SeekFrom::Start((id.0 * PAGE_SIZE) as u64))?;
            file.read_exact(&mut buf)?;
            let (read, _): (FreeListRead, _) = bincode_next::decode_from_slice(&buf, INDEX_CONFIG)?;
            current = read.next_free;
        }

        Ok((Pager {
            file: file,
            free_list: free_list,
            dirty_cache: HashMap::new(),
            num_pages: header.num_pages,
        }, header.root_page, header.order))
    }

    // Clear out the cache and write it to disk
    pub fn flush(&mut self) -> Result<(), StoreErr> {
        let cache = std::mem::take(&mut self.dirty_cache);
        for (_, page) in cache {
            let file = &mut self.file;
            let mut new_page = [0u8; PAGE_SIZE];
            bincode_next::encode_into_slice(&page, &mut new_page, INDEX_CONFIG)?;
            write_page(file, page.header.page_id, &new_page)?;
        }
        Ok(())
    }

    // Construct page and serialize it
    pub fn alloc(&mut self) -> PageId {
        if self.free_list.is_empty() {
            let id = PageId(self.num_pages);
            self.num_pages += 1;
            id
        } else {
            self.free_list.pop().unwrap()
        }
    }

    // Delete a page
    pub fn free(&mut self, id: PageId) -> Result<(), StoreErr> {
        if let Some(prev_id) = self.free_list.last().copied() {
            let mut buf = [0u8; PAGE_SIZE];
            let new_header = PageHeader {
                page_type: PageType::Index, // This shouldn't matter
                page_id: prev_id,
                next_free: Some(id),
                next_over: None,
            };

            bincode_next::encode_into_slice(&new_header, &mut buf, INDEX_CONFIG)?;
            write_page(&mut self.file, prev_id, &buf)?;
        }

        let mut buf = [0u8; PAGE_SIZE];
        let new_tail = PageHeader {
            page_type: PageType::Index,
            page_id: id,
            next_free: None,
            next_over: None,
        };
        bincode_next::encode_into_slice(&new_tail, &mut buf, INDEX_CONFIG)?;
        write_page(&mut self.file, id, &buf)?;

        self.free_list.push(id);
        self.dirty_cache.remove(&id);
        Ok(())
    }

    // Write a new DbHeader and close the pager. 
    pub fn close(&mut self, root: Option<PageId>, order: usize) -> Result<(), StoreErr> {
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
        let (pager, root, _) = Pager::new(path).unwrap();
        assert!(root.is_none());
        assert_eq!(pager.num_pages, 1);
    }

    #[test]
    fn pager_open() {
        let tmp = temp_path();
        let path = tmp.path().to_str().unwrap();
        Pager::new(path).unwrap();

        let (pager, root, _) = Pager::open(path).unwrap();
        assert!(root.is_none());
        assert_eq!(pager.num_pages, 1);
    }

    #[test]
    fn pager_open_and_close() {
        let tmp = temp_path();
        let path = tmp.path().to_str().unwrap();
        let (mut pager, _, order) = Pager::new(path).unwrap();

        let page_id = pager.alloc();
        let new_page = IndexPage {
            header: PageHeader { 
                page_type: PageType::Index,
                page_id: page_id,
                next_free: None,
                next_over: None,
            },
            keys: vec!["key1".into(), "key2".into()],
            node_type: NodeType::Leaf {
                pages: vec![], 
                next: None,
            }
        };
        Page::write(&mut pager, new_page).unwrap();

        let id2 = pager.alloc();
        let new_page2 = IndexPage {
            header: PageHeader { 
                page_type: PageType::Index,
                page_id: id2,
                next_free: None,
                next_over: None,
            },
            keys: vec!["key1".into(), "key2".into()],
            node_type: NodeType::Leaf {
                pages: vec![], 
                next: None,
            }
        };
        Page::write(&mut pager, new_page2).unwrap();
        let id3 = pager.alloc();
        let new_page3 = IndexPage {
            header: PageHeader { 
                page_type: PageType::Index,
                page_id: id3,
                next_free: None,
                next_over: None,
            },
            keys: vec!["key1".into(), "key2".into()],
            node_type: NodeType::Leaf {
                pages: vec![], 
                next: None,
            }
        };
        Page::write(&mut pager, new_page3).unwrap();
        pager.flush().unwrap();
        pager.free(id2).unwrap();
        pager.free(id3).unwrap();

        pager.close(Some(page_id), order).unwrap();

        let (reopened, root, _) = Pager::open(path).unwrap();
        assert_eq!(root, Some(page_id));
        assert_eq!(reopened.num_pages, 4);
        assert_eq!(reopened.free_list, vec![id2, id3]);
    }

    #[test]
    fn pager_reject_bad_magic() {
        let tmp = temp_path();
        let path = tmp.path().to_str().unwrap();
        std::fs::write(path, &[0u8; PAGE_SIZE]).unwrap();
        assert!(matches!(Pager::open(path), Err(StoreErr::BadFile)));
    }

    #[test]
    fn pager_alloc() {
        let tmp = temp_path();
        let path = tmp.path().to_str().unwrap();
        let (mut pager, _, _) = Pager::new(path).unwrap();
        let id1 = pager.alloc();
        let id2 = pager.alloc();
        assert!(id2.0 > id1.0);
        assert_eq!(pager.num_pages, 3);
    }

    #[test]
    fn pager_write_read_page() {
        let tmp = temp_path();
        let path = tmp.path().to_str().unwrap();
        let (mut pager, _, _) = Pager::new(path).unwrap();
        let page_id = pager.alloc();

        let new_page = IndexPage {
            header: PageHeader { 
                page_type: PageType::Index,
                page_id: page_id,
                next_free: None,
                next_over: None,
            },
            keys: vec!["key1".into(), "key2".into()],
            node_type: NodeType::Leaf {
                pages: vec![], 
                next: None,
            }
        };
        Page::write(&mut pager, new_page).unwrap();
        pager.flush().unwrap();

        let read = IndexPage::read(&mut pager, page_id).unwrap();
        assert_eq!(read.keys, vec!["key1", "key2"]);
    }

    #[test]
    fn pager_flush() {
        let tmp = temp_path();
        let path = tmp.path().to_str().unwrap();
        let (mut pager, _, _) = Pager::new(path).unwrap();

        let id1 = pager.alloc();
        let page1 = IndexPage {
            header: PageHeader { 
                page_type: PageType::Index,
                page_id: id1,
                next_free: None,
                next_over: None,
            },
            keys: vec!["key1".into(), "key2".into()],
            node_type: NodeType::Leaf {
                pages: vec![], 
                next: None,
            }
        };

        pager.dirty_cache.insert(id1, page1);
        pager.flush().unwrap();

        let read = IndexPage::read(&pager, id1).unwrap();
        assert_eq!(read.keys, vec!["key1", "key2"]);
        assert_eq!(pager.num_pages, 2);
    }

    #[test]
    fn pager_free_and_add() {
        let tmp = temp_path();
        let path = tmp.path().to_str().unwrap();
        let (mut pager, _, _) = Pager::new(path).unwrap();

        let id1 = pager.alloc();
        let page1 = DataPage {
            header: PageHeader { 
                page_type: PageType::Index,
                page_id: id1,
                next_free: None,
                next_over: None,
            },
            value: Value::Int(3),
        };
        Page::write(&mut pager, page1).unwrap();

        let id2 = pager.alloc();
        let page2 = IndexPage {
            header: PageHeader { 
                page_type: PageType::Index,
                page_id: id2,
                next_free: None,
                next_over: None,
            },
            keys: vec!["key1".into()],
            node_type: NodeType::Leaf {
                pages: vec![], 
                next: None,
            }
        };
        Page::write(&mut pager, page2).unwrap();
        pager.flush().unwrap();

        pager.free(id2).unwrap();
        pager.free(id1).unwrap();
        assert_eq!(pager.free_list.len(), 2);


        let id3 = pager.alloc();
        let page3 = DataPage {
            header: PageHeader { 
                page_type: PageType::Index,
                page_id: id3,
                next_free: None,
                next_over: None,
            },
            value: Value::Int(12),
        };
        Page::write(&mut pager, page3).unwrap();

        assert_eq!(pager.num_pages, 3);
        let read: DataPage = Page::read(&mut pager, id1).unwrap();
        assert_eq!(read.value, Value::Int(12));
    }
}
