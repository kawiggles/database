use crate::VERSION;
use crate::store::{DEFAULT_ORDER, DEFAULT_FILE};
use crate::store::value::Value;
use crate::logs::DbError;

use bincode_next::{config, Encode, Decode};
use std::fs::{File, OpenOptions};
use std::io::{Write, Read, Seek, SeekFrom, BufReader};
use std::fmt;
use std::collections::HashMap;

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

pub trait Page: Sized {
    fn page_id(&self) -> PageId;
    fn write(pager: &mut Pager, page: &Self) -> Result<(), DbError>;
    fn read(pager: &mut Pager, id: PageId) -> Result<Self, DbError>;
}

#[derive(Eq, Hash, PartialEq, Encode, Decode, Clone, Copy, Debug)]
pub struct PageId(pub usize);

impl fmt::Display for PageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Encode, Decode)]
pub enum PageType {
    Index,
    Data,
}

// TODO: Make data representation more efficient (manual bincode)
#[derive(Encode, Decode)]
pub struct PageHeader {
    page_type: PageType, // 4 bytes
    page_id: PageId, // 8 bytes
    next_free: Option<PageId>, // 8 bytes (Option) + 8 bytes
} // Total: 28 bytes

pub fn write_page(file: &mut File, id: PageId, buf: &[u8; PAGE_SIZE]) -> Result<(), DbError> {
    file.seek(SeekFrom::Start((id.0 * PAGE_SIZE) as u64))?;
    file.write_all(buf)?;
    Ok(())
}

#[derive(Encode, Decode)]
pub struct IndexPage {
    pub header: PageHeader, // 28 bytes
    pub keys: Vec<String>, // 8 + ? Bytes
    pub node_type: NodeType, // 28 + ? Bytes
}

impl Page for IndexPage {
    fn page_id(&self) -> PageId {
        self.header.page_id
    }

    fn write(pager: &mut Pager, new_page: &Self) -> Result<(), DbError> {
        let file = &mut pager.file;
        let mut page = [0u8; PAGE_SIZE];
        bincode_next::encode_into_slice(&new_page, &mut page, INDEX_CONFIG)?;
        write_page(file, new_page.header.page_id, &page)?;
        Ok(())
    }

    fn read(pager: &mut Pager, id: PageId) -> Result<Self, DbError> {
        let file = &mut pager.file;
        let mut buf = [0u8; PAGE_SIZE];
        file.seek(SeekFrom::Start((id.0 * PAGE_SIZE) as u64))?;
        file.read_exact(&mut buf)?;
        let (page, size): (IndexPage, usize) = bincode_next::decode_from_slice(&buf, INDEX_CONFIG)?;

        if size > PAGE_SIZE {
            return Err(DbError::ReadOverflow)
        }

        Ok(page)
    }
}

#[derive(Encode, Decode)]
pub enum NodeType { // 4 bytes
    Branch { children: Vec<PageId> }, // 8 bytes + ? 8 byte PageIds
    Leaf { 
        pages: Vec<PageId>, // 8 bytes + ? 8 byte PageIds
        next: Option<PageId> // 8 bytes (Option) + 8 bytes
    } // 24 + 8? Bytes
} // max 28 + 8? Bytes

#[derive(Encode, Decode)]
pub struct DataPage {
    header: PageHeader,
    pub value: Value,
}

impl Page for DataPage {
    fn page_id(&self) -> PageId {
        self.header.page_id
    }

    fn write(pager: &mut Pager, new_page: &Self) -> Result<(), DbError> {
        let file = &mut pager.file;
        let mut page = [0u8; PAGE_SIZE];
        bincode_next::encode_into_slice(&new_page, &mut page, DATA_CONFIG)?;
        write_page(file, new_page.header.page_id, &page)?;
        Ok(())
    }

    fn read(pager: &mut Pager, id: PageId) -> Result<DataPage, DbError> {
        let file = &mut pager.file;
        let mut buf = [0u8; PAGE_SIZE];
        file.seek(SeekFrom::Start((id.0 * PAGE_SIZE) as u64))?;
        file.read_exact(&mut buf)?;
        let (page, size): (DataPage, usize) = bincode_next::decode_from_slice(&buf, DATA_CONFIG)?;

        if size > PAGE_SIZE {
            return Err(DbError::ReadOverflow)
        }

        Ok(page)
    }
}

#[derive(Encode, Decode)]
pub struct DbHeader {
    magic: [u8; 8],
    version: u32,
    root_page: Option<PageId>,
    order: usize,
    num_pages: usize,
    free_list_head: Option<PageId>,
}

impl DbHeader {
    fn write(&self, file: &mut File) -> Result<(), DbError> {
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
struct FreeListReader {
    _page_type: PageType,
    _page_id: PageId,
    next_free: Option<PageId>,
}

// TODO: write logs...
impl Pager {
    // Function to create a new database file if none exists
    pub fn new(path: &str) -> Result<(Self, Option<PageId>, usize), DbError> {
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
    pub fn open(path: &str) -> Result<(Self, Option<PageId>, usize), DbError> {
        // TODO: if no path, use default file (consider Option<&str>)
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;
        let mut reader = BufReader::new(&mut file);
        let header: DbHeader = bincode_next::decode_from_std_read(&mut reader, INDEX_CONFIG)?;

        if header.magic != MAGIC {
            return Err(DbError::BadFile);
        }

        let mut free_list: Vec<PageId> = Vec::new();
        let mut current = header.free_list_head;
        while let Some(id) = current {
            free_list.push(id);
            let mut buf = [0u8; PAGE_SIZE];
            file.seek(SeekFrom::Start((id.0 * PAGE_SIZE) as u64))?;
            file.read_exact(&mut buf)?;
            let (reader, _): (FreeListReader, _) = bincode_next::decode_from_slice(&buf, INDEX_CONFIG)?;
            current = reader.next_free;
        }

        Ok((Pager {
            file: file,
            free_list: free_list,
            dirty_cache: HashMap::new(),
            num_pages: header.num_pages,
        }, header.root_page, header.order))
    }

    // Clear out the cache and write it to disk
    fn flush(&mut self) -> Result<(), DbError> {
        let cache = std::mem::take(&mut self.dirty_cache);
        for (_, page) in cache {
            IndexPage::write(self, &page)?;
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
    pub fn free(&mut self, id: PageId) -> Result<(), DbError> {
        if let Some(prev_id) = self.free_list.last().copied() {
            let mut buf = [0u8; PAGE_SIZE];
            let new_header = PageHeader {
                page_type: PageType::Index, // This shouldn't matter
                page_id: prev_id,
                next_free: Some(id),
            };

            bincode_next::encode_into_slice(&new_header, &mut buf, INDEX_CONFIG)?;
            write_page(&mut self.file, prev_id, &buf)?;
        }

        self.free_list.push(id);
        Ok(())
    }

    // Write a new DbHeader and close the pager. 
    pub fn close(&mut self, root: Option<PageId>, order: usize) -> Result<(), DbError> {
        let new_dbheader = DbHeader {
            magic: MAGIC,
            version: VERSION,
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
        let (pager, root, order) = Pager::new(path).unwrap();
        assert!(root.is_none());
        assert_eq!(order, DEFAULT_ORDER);
        assert_eq!(pager.num_pages, 1);
    }

    #[test]
    fn pager_open() {
        let tmp = temp_path();
        let path = tmp.path().to_str().unwrap();
        Pager::new(path).unwrap();

        let (pager, root, order) = Pager::open(path).unwrap();
        assert!(root.is_none());
        assert_eq!(order, DEFAULT_ORDER);
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
            },
            keys: vec!["key1".into(), "key2".into()],
            node_type: NodeType::Leaf {
                pages: vec![], 
                next: None,
            }
        };
        Page::write(&mut pager, &new_page).unwrap();

        let id2 = pager.alloc();
        let new_page2 = IndexPage {
            header: PageHeader { 
                page_type: PageType::Index,
                page_id: id2,
                next_free: None,
            },
            keys: vec!["key1".into(), "key2".into()],
            node_type: NodeType::Leaf {
                pages: vec![], 
                next: None,
            }
        };
        Page::write(&mut pager, &new_page2).unwrap();
        let id3 = pager.alloc();
        let new_page3 = IndexPage {
            header: PageHeader { 
                page_type: PageType::Index,
                page_id: id3,
                next_free: None,
            },
            keys: vec!["key1".into(), "key2".into()],
            node_type: NodeType::Leaf {
                pages: vec![], 
                next: None,
            }
        };
        Page::write(&mut pager, &new_page3).unwrap();
        pager.free(id2).unwrap();
        pager.free(id3).unwrap();

        pager.close(Some(page_id), order).unwrap();

        let (reopened, root, order) = Pager::open(path).unwrap();
        assert_eq!(root, Some(page_id));
        assert_eq!(order, DEFAULT_ORDER);
        assert_eq!(reopened.num_pages, 4);
        assert_eq!(reopened.free_list, vec![id2, id3]);
    }

    #[test]
    fn pager_reject_bad_magic() {
        let tmp = temp_path();
        let path = tmp.path().to_str().unwrap();
        std::fs::write(path, &[0u8; PAGE_SIZE]).unwrap();
        assert!(matches!(Pager::open(path), Err(DbError::BadFile)));
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
            },
            keys: vec!["key1".into(), "key2".into()],
            node_type: NodeType::Leaf {
                pages: vec![], 
                next: None,
            }
        };

        Page::write(&mut pager, &new_page).unwrap();
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
            },
            keys: vec!["key1".into(), "key2".into()],
            node_type: NodeType::Leaf {
                pages: vec![], 
                next: None,
            }
        };
        
        assert!(IndexPage::read(&mut pager, id1).is_err());

        pager.dirty_cache.insert(id1, page1);
        let _ = pager.flush();

        let read = IndexPage::read(&mut pager, id1).unwrap();
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
            },
            value: Value::Int(3),
        };
        Page::write(&mut pager, &page1).unwrap();

        let id2 = pager.alloc();
        let page2 = IndexPage {
            header: PageHeader { 
                page_type: PageType::Index,
                page_id: id2,
                next_free: None,
            },
            keys: vec!["key1".into()],
            node_type: NodeType::Leaf {
                pages: vec![], 
                next: None,
            }
        };
        Page::write(&mut pager, &page2).unwrap();

        pager.free(id2).unwrap();
        pager.free(id1).unwrap();
        assert_eq!(pager.free_list.len(), 2);


        let id3 = pager.alloc();
        let page3 = DataPage {
            header: PageHeader { 
                page_type: PageType::Index,
                page_id: id3,
                next_free: None,
            },
            value: Value::Int(12),
        };
        Page::write(&mut pager, &page3).unwrap();

        assert_eq!(pager.num_pages, 3);
        let read: DataPage = Page::read(&mut pager, id1).unwrap();
        assert_eq!(read.value, Value::Int(12));
    }
}
