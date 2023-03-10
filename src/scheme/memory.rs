use alloc::sync::Arc;
use spin::RwLock;

use crate::context;
use crate::context::memory::{AddrSpace, Grant};
use crate::memory::{free_frames, used_frames, PAGE_SIZE};

use crate::syscall::data::{Map, StatVfs};
use crate::syscall::error::*;
use crate::syscall::scheme::Scheme;

pub struct MemoryScheme;

impl MemoryScheme {
    pub fn new() -> Self {
        MemoryScheme
    }

    pub fn fmap_anonymous(addr_space: &Arc<RwLock<AddrSpace>>, map: &Map) -> Result<usize> {
        let (requested_page, page_count) = crate::syscall::validate::validate_region(map.address, map.size)?;

        let page = addr_space
            .write()
            .mmap((map.address != 0).then_some(requested_page), page_count, map.flags, |page, flags, mapper, flusher| {
                Ok(Grant::zeroed(page, page_count, flags, mapper, flusher)?)
            })?;

        Ok(page.start_address().data())
    }
}
impl Scheme for MemoryScheme {
    fn open(&self, _path: &str, _flags: usize, _uid: u32, _gid: u32) -> Result<usize> {
        Ok(0)
    }

    fn fstatvfs(&self, _file: usize, stat: &mut StatVfs) -> Result<usize> {
        let used = used_frames() as u64;
        let free = free_frames() as u64;

        stat.f_bsize = PAGE_SIZE as u32;
        stat.f_blocks = used + free;
        stat.f_bfree = free;
        stat.f_bavail = stat.f_bfree;

        Ok(0)
    }

    fn fmap(&self, _id: usize, map: &Map) -> Result<usize> {
        Self::fmap_anonymous(&Arc::clone(context::current()?.read().addr_space()?), map)
    }

    fn fcntl(&self, _id: usize, _cmd: usize, _arg: usize) -> Result<usize> {
        Ok(0)
    }

    fn fpath(&self, _id: usize, buf: &mut [u8]) -> Result<usize> {
        let mut i = 0;
        let scheme_path = b"memory:";
        while i < buf.len() && i < scheme_path.len() {
            buf[i] = scheme_path[i];
            i += 1;
        }
        Ok(i)
    }

    fn close(&self, _id: usize) -> Result<usize> {
        Ok(0)
    }
}
impl crate::scheme::KernelScheme for MemoryScheme {
    fn kfmap(&self, _number: usize, addr_space: &Arc<RwLock<AddrSpace>>, map: &Map, _consume: bool) -> Result<usize> {
        Self::fmap_anonymous(addr_space, map)
    }
}
