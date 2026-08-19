use std::fs::File;
use std::str::from_utf8;
use std::io::{Read, Seek, SeekFrom};

use super::{PAGE_SIZE, PageId};
use crate::store::Rid;

use crate::errors::StoreResult;

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

pub fn read_str<R: Read>(bytes: &mut R) -> StoreResult<String> {
    let len = read_u16(bytes)?;
    let mut buf: Vec<u8> = vec![0; len as usize];
    bytes.read_exact(&mut buf)?;
    Ok(from_utf8(&buf)?.into())
}

pub fn scan_page(id: PageId, file: &mut File) -> StoreResult<[u8; PAGE_SIZE]> {
    file.seek(SeekFrom::Start((id.get() * PAGE_SIZE) as u64));
    let mut buf = [0u8; PAGE_SIZE];
    file.read_exact(&mut buf)?;
    Ok(buf)
}
