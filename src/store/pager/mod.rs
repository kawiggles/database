pub mod page;

use crate::{
    VERSION,
    store::{
        value::Value,
        pager::page::{Page, PageId, PageHeader, PageType, IndexPage},
    },
    errors::{StoreErr, StoreResult},
    tcp::DEFAULT_FILE,
};

use std::{
    fs::{File, OpenOptions},
    os::unix::fs::FileExt,
    io::{Write, Read, Seek, SeekFrom, BufReader},
    collections::HashMap,
};
use log::{info};

// TODO: replace PAGE_SIZE instances with the page_size metadata for choices of page sizes
pub const PAGE_SIZE: usize = 4096;

const MAGIC: [u8; 8] = *b"KAWIKADB";

// TODO: Figure out what this is
pub struct Oid;

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
    fn read(&self, file: &mut File) -> StoreResult<()> {
        let mut page = [0u8; DBHEADER_SIZE];
        bincode_next::encode_into_slice(self, &mut page, INDEX_CONFIG)?;
        write_page(file, PageId(0), &page)?;
        Ok(())
    }

    fn write(&self, file: &mut File) -> StoreResult<()> {
    }
}

pub struct Pager {
    pub file: File,
    free_list: Vec<PageId>,
    dirty_cache: HashMap<PageId, Page>,
    pub num_pages: usize,
}

#[derive(Decode)]
struct FreeListRead {
    _page_type: PageType,
    _page_id: PageId,
    next_free: Option<PageId>,
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

        Ok((Pager {
            file: file,
            free_list: Vec::new(),
            dirty_cache: HashMap::new(),
            num_pages: 1,
        }, new_head.root_page, new_head.order))
    }

    // Function to open database if one exists
    pub fn open(path: &str) -> StoreResult<(Self, Option<PageId>, usize)> {
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

    // e.g. read::<DataPage>
    pub fn read<T: Page>(&mut self, id: PageId) -> StoreResult<T> {
        let mut buf = [0u8; PAGE_SIZE];
        self.file.read_at(&mut buf, (id.get() * PAGE_SIZE) as u64)?;
        let page: T = T::deserialize(buf)?;
        
        if page.header().pagetype != T::pagetype() {
            Err(/* Some error about wrong page found */),
        }

        Ok(page)
    }

    pub fn write_page<T: Page>(&mut self, page: T) {
        let bytes = page.serialize();
    }

    // Clear out the cache and write it to disk
    pub fn flush(&mut self) -> StoreResult<()> {
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

    #[test]
    fn pager_open_and_close() {
        let tmp = temp_path();
        let path = tmp.path().to_str().unwrap();
        let (mut pager, _, order) = Pager::new(path, 4).unwrap();

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
        let (mut pager, _, _) = Pager::new(path, 4).unwrap();
        let id1 = pager.alloc();
        let id2 = pager.alloc();
        assert!(id2.0 > id1.0);
        assert_eq!(pager.num_pages, 3);
    }

    #[test]
    fn pager_write_read_page() {
        let tmp = temp_path();
        let path = tmp.path().to_str().unwrap();
        let (mut pager, _, _) = Pager::new(path, 4).unwrap();
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
        let (mut pager, _, _) = Pager::new(path, 4).unwrap();

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
        let (mut pager, _, _) = Pager::new(path, 4).unwrap();

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
