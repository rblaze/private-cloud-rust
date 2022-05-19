use bytes::Buf;
use libsodium_sys::{
    crypto_generichash_BYTES, crypto_generichash_final, crypto_generichash_init,
    crypto_generichash_state, crypto_generichash_update,
};

const HASH_SIZE: usize = crypto_generichash_BYTES as usize;

#[derive(Clone, Debug)]
pub struct ChunkedHash {
    state: crypto_generichash_state,
}

impl ChunkedHash {
    pub fn new() -> ChunkedHash {
        let mut st = ChunkedHash {
            state: crypto_generichash_state { opaque: [0; 384] },
        };

        // Initialize hash without key and default output size
        unsafe {
            crypto_generichash_init(&mut st.state, std::ptr::null(), 0, HASH_SIZE);
        }

        st
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
    use crate::crypto::hash::ChunkedHash;
    use bytes::Buf;
    use std::collections::VecDeque;

    #[test]
    fn simple_hash() {
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
}
