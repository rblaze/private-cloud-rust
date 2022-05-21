use anyhow::{anyhow, Result};
use libsodium_sys::{sodium_free, sodium_malloc};
use std::ffi::c_void;

#[derive(Debug)]
pub struct SecureMemory {
    data: *mut c_void,
    size: usize,
}

unsafe impl Sync for SecureMemory {}
unsafe impl Send for SecureMemory {}

impl SecureMemory {
    pub fn new(size: usize) -> Result<SecureMemory> {
        let data = unsafe { sodium_malloc(size) };

        if data.is_null() {
            return Err(anyhow!("Error allocating secure memory"));
        }

        Ok(SecureMemory { data, size })
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.data as *const u8
    }

    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.data as *mut u8
    }
}

impl Drop for SecureMemory {
    fn drop(&mut self) {
        unsafe {
            sodium_free(self.data);
        }
    }
}

// There is no guarantee about allocation alignment. u8 is a safe cast target.
impl AsRef<[u8]> for SecureMemory {
    fn as_ref(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.data as *const u8, self.size) }
    }
}

impl AsMut<[u8]> for SecureMemory {
    fn as_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.data as *mut u8, self.size) }
    }
}

#[cfg(test)]
mod tests {
    use crate::crypto::init;
    use crate::crypto::secure_memory::SecureMemory;

    #[test]
    fn create_fill_and_drop() {
        init();
        let mut m = SecureMemory::new(50).expect("SecureMemory allocation failed");

        let slice = m.as_mut();

        for i in 0..slice.len() {
            slice[i] = (i % 100) as u8;
        }
    }
}
