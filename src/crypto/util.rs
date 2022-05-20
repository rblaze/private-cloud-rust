use libsodium_sys::sodium_init;

pub fn init() {
    unsafe {
        sodium_init();
    }
}