use anyhow::{anyhow, Result};
use libc::{c_char, c_uchar};
use libsodium_sys::{
    crypto_kdf_CONTEXTBYTES, crypto_kdf_KEYBYTES, crypto_kdf_derive_from_key, crypto_kdf_keygen,
    sodium_free, sodium_malloc,
};
use std::ffi::c_void;

const MASTER_KEY_SIZE: usize = crypto_kdf_KEYBYTES as usize;
const CONTEXT_SIZE: usize = crypto_kdf_CONTEXTBYTES as usize;

pub struct MasterKey {
    opaque: *mut c_uchar,
}

unsafe impl Sync for MasterKey {}
unsafe impl Send for MasterKey {}

impl MasterKey {
    pub fn new() -> Result<MasterKey> {
        let key;

        unsafe {
            key = sodium_malloc(MASTER_KEY_SIZE) as *mut c_uchar;
            if key.is_null() {
                return Err(anyhow!("Error allocating master key"));
            }
            crypto_kdf_keygen(key);
        }

        Ok(MasterKey { opaque: key })
    }

    pub fn from(hex: &str) -> Result<MasterKey> {
        let bytes = hex::decode(hex)?;

        if bytes.len() != MASTER_KEY_SIZE {
            return Err(anyhow!("Invalid master key size"));
        }

        let key;

        unsafe {
            key = sodium_malloc(MASTER_KEY_SIZE) as *mut c_uchar;
            if key.is_null() {
                return Err(anyhow!("Error allocating master key"));
            }

            std::ptr::copy_nonoverlapping(bytes.as_ptr(), key, MASTER_KEY_SIZE);
        }

        Ok(MasterKey { opaque: key })
    }

    pub fn derive_subkey(&self, subkey: &mut [u8], subkey_id: u64, context: &str) -> Result<()> {
        let mut ctx: [c_char; CONTEXT_SIZE] = [0; CONTEXT_SIZE];

        // Copy bytes/ASCII chars from context str to fixed-size buffer
        for i in 0..std::cmp::min(context.len(), ctx.len()) {
            ctx[i] = context.as_bytes()[i] as i8;
        }

        unsafe {
            if crypto_kdf_derive_from_key(
                subkey.as_mut_ptr(),
                subkey.len(),
                subkey_id,
                ctx.as_ptr(),
                self.opaque,
            ) != 0
            {
                return Err(anyhow!("Error deriving subkey"));
            }
        }

        Ok(())
    }
}

impl Drop for MasterKey {
    fn drop(&mut self) {
        unsafe {
            sodium_free(self.opaque as *mut c_void);
        }
    }
}

impl std::fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MasterKey")
            .field("opaque", &"*****")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::crypto::master_key::{MASTER_KEY_SIZE, MasterKey};
    use crate::crypto::init;

    #[test]
    fn create_and_drop() {
        init();
        let k = MasterKey::new();
        assert!(matches!(k, Ok{..}));
    }

    #[test]
    fn from_invalid_size() {
        init();
        let k = MasterKey::from("1234");
        assert!(matches!(k, Err{..}));        
    }

    #[test]
    fn from_valid_size() {
        init();
        let bytes = [43; MASTER_KEY_SIZE];
        let hexdump = hex::encode(bytes);
        let k = MasterKey::from(&hexdump);
        assert!(matches!(k, Ok{..}));        
    }

    #[test]
    fn derive() {
        init();
        let key = MasterKey::new().expect("MasterKey::new() failed");

        let mut subkey1 = [0; 32];
        key.derive_subkey(&mut subkey1, 1, "foobar")
            .expect("Key derivation failed");

        let mut subkey2 = [0; 32];
        key.derive_subkey(&mut subkey2, 1, "bar")
            .expect("Key derivation failed");
        assert_ne!(subkey1, subkey2);

        let mut subkey3 = [0; 32];
        key.derive_subkey(&mut subkey3, 1, "foobar")
            .expect("Key derivation failed");

        assert_eq!(subkey1, subkey3);
    }
}
