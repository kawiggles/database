use std::{
    fs::File,
    io::{Read, Write},
};

use crate::errors::StoreResult;

use super::{PageId, read_u32, read_usize};

const DBHEADER_SIZE: usize = 3000;
pub struct DbHeader {
    pub magic: [u8; 8],
    pub version: u32,
    pub page_size: usize,
    pub root_page: Option<PageId>,
    pub order: usize,
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
        let order = read_usize(file)?;
        let num_pages = read_usize(file)?;
        let free_list_head = PageId::new(read_usize(file)?);
        let active_data = PageId::new(read_usize(file)?);

        Ok(DbHeader{ magic, version, page_size, root_page, order, num_pages, free_list_head, active_data }) // cry about it
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

        buf.extend_from_slice(&self.order.to_le_bytes());
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

        file.write_all(&buf)?;
        Ok(())
    }
}
