use crate::crypto::master_key::MasterKey;
use anyhow::Result;
use bytes::Buf;
use libsodium_sys::{
    crypto_generichash_BYTES, crypto_generichash_KEYBYTES, crypto_generichash_final,
    crypto_generichash_init, crypto_generichash_state, crypto_generichash_update,
};

const HASH_SIZE: usize = crypto_generichash_BYTES as usize;
const HASH_KEY_SIZE: usize = crypto_generichash_KEYBYTES as usize;

// Not using protected memory for this key: it is for hash value randomization, not for security.
pub struct HashKey {
    opaque: [u8; HASH_KEY_SIZE],
}

impl std::fmt::Debug for HashKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChunkedHashKey")
            .field("opaque", &"*****")
            .finish()
    }
}

impl HashKey {
    pub fn new(master_key: &MasterKey, keyid: u64, context: &str) -> Result<HashKey> {
        let mut key = [0; HASH_KEY_SIZE];

        master_key.derive_subkey(&mut key, keyid, context)?;

        Ok(HashKey { opaque: key })
    }
}

#[derive(Clone, Debug)]
pub struct ChunkedHash {
    state: crypto_generichash_state,
}

impl ChunkedHash {
    pub fn new() -> ChunkedHash {
        let mut state = crypto_generichash_state { opaque: [0; 384] };

        // Initialize keyless hash with default output size
        unsafe {
            crypto_generichash_init(&mut state, std::ptr::null(), 0, HASH_SIZE);
        }

        ChunkedHash { state }
    }

    pub fn keyed(key: &HashKey) -> ChunkedHash {
        let mut state = crypto_generichash_state { opaque: [0; 384] };

        // Initialize keyed hash with default output size
        unsafe {
            crypto_generichash_init(&mut state, key.opaque.as_ptr(), key.opaque.len(), HASH_SIZE);
        }

        ChunkedHash { state }
    }

    pub fn update(&mut self, mut data: impl Buf) {
        while data.has_remaining() {
            let chunk = data.chunk();
            let chunklen = chunk.len();

            unsafe {
                crypto_generichash_update(&mut self.state, chunk.as_ptr(), chunklen as u64);
            }

            data.advance(chunklen);
        }
    }

    pub fn finalize(mut self) -> [u8; HASH_SIZE] {
        let mut hash = [0; HASH_SIZE];

        unsafe {
            crypto_generichash_final(&mut self.state, hash.as_mut_ptr(), hash.len());
        }

        hash
    }
}

#[cfg(test)]
mod tests {
    use crate::crypto::hash::{ChunkedHash, HashKey};
    use crate::crypto::init;
    use crate::crypto::master_key::MasterKey;
    use bytes::{Buf, Bytes};
    use std::collections::VecDeque;

    #[test]
    fn simple_hash() {
        init();
        let mut input = vec![];
        let mut v: u8 = 41;

        input.resize_with(9876501, || {
            v = v.wrapping_add(1);
            v
        });

        let mut hash = ChunkedHash::new();
        hash.update(input.as_slice());
        let output = hash.finalize();
        let repr = hex::encode(output);

        assert_eq!(
            repr,
            "8665019f9bc50eaf32f020c89c03564ffd8ac47a180a1079e07b43a6ab1abe35"
        );
    }

    #[test]
    fn internal_chain() {
        init();
        let mut v: u8 = 41;
        let mut input1 = VecDeque::new();

        input1.resize_with(1234567, || {
            v = v.wrapping_add(1);
            v
        });

        let mut input2 = VecDeque::new();
        input2.resize_with(9876501 - input1.len(), || {
            v = v.wrapping_add(1);
            v
        });

        let mut hash = ChunkedHash::new();
        hash.update(input1.chain(input2));
        let output = hash.finalize();
        let repr = hex::encode(output);

        assert_eq!(
            repr,
            "8665019f9bc50eaf32f020c89c03564ffd8ac47a180a1079e07b43a6ab1abe35"
        );
    }

    #[test]
    fn external_chain() {
        init();
        let mut v: u8 = 41;
        let mut input1 = VecDeque::new();

        input1.resize_with(1234567, || {
            v = v.wrapping_add(1);
            v
        });

        let mut input2 = VecDeque::new();
        input2.resize_with(9876501 - input1.len(), || {
            v = v.wrapping_add(1);
            v
        });

        let mut hash = ChunkedHash::new();
        hash.update(input1);
        hash.update(input2);
        let output = hash.finalize();
        let repr = hex::encode(output);

        assert_eq!(
            repr,
            "8665019f9bc50eaf32f020c89c03564ffd8ac47a180a1079e07b43a6ab1abe35"
        );
    }

    #[test]
    fn keyed_hash() {
        init();

        let master_key = MasterKey::new().expect("failed to create master key");
        let hash_key_ctx1 = HashKey::new(&master_key, 1, "ctx").expect("failed to create hash key");
        let hash_key_ctx2 = HashKey::new(&master_key, 2, "ctx").expect("failed to create hash key");
        let hash_key_context1 =
            HashKey::new(&master_key, 1, "context").expect("failed to create hash key");
        let hash_key_ctx1_dup =
            HashKey::new(&master_key, 1, "ctx").expect("failed to create hash key");
        let data = Bytes::from("This is test message");

        let mut hash = ChunkedHash::keyed(&hash_key_ctx1);
        hash.update(data.to_owned());
        let result_ctx1 = hash.finalize();

        let mut hash = ChunkedHash::keyed(&hash_key_ctx2);
        hash.update(data.to_owned());
        let result_ctx2 = hash.finalize();

        let mut hash = ChunkedHash::keyed(&hash_key_context1);
        hash.update(data.to_owned());
        let result_context1 = hash.finalize();

        let mut hash = ChunkedHash::keyed(&hash_key_ctx1_dup);
        hash.update(data.to_owned());
        let result_ctx1_dup = hash.finalize();

        assert_ne!(result_ctx1, result_ctx2);
        assert_ne!(result_ctx1, result_context1);
        assert_ne!(result_ctx2, result_context1);
        assert_eq!(result_ctx1, result_ctx1_dup);
    }
}
