use std::fmt::Display;

pub mod asset;
pub mod chain_key;
pub mod foreign_address;
pub mod kdf;
pub mod oracle;
pub mod pyth;
pub mod signer;

pub trait Rejectable<T> {
    fn unwrap_or_reject(self) -> T;
    fn expect_or_reject(self, msg: impl Display) -> T;
}

#[inline]
fn do_panic(msg: &str) -> ! {
    #[cfg(target_arch = "wasm32")]
    {
        near_sdk::env::panic_str(msg)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        panic!("{msg}")
    }
}

impl<T, E: Display> Rejectable<T> for Result<T, E> {
    fn unwrap_or_reject(self) -> T {
        self.unwrap_or_else(|e| do_panic(&e.to_string()))
    }

    fn expect_or_reject(self, msg: impl Display) -> T {
        self.unwrap_or_else(|e| do_panic(&format!("{msg}: {e}")))
    }
}

impl<T> Rejectable<T> for Option<T> {
    fn unwrap_or_reject(self) -> T {
        self.unwrap_or_else(|| do_panic("Attempted to unwrap None"))
    }

    fn expect_or_reject(self, msg: impl Display) -> T {
        self.unwrap_or_else(|| do_panic(&msg.to_string()))
    }
}

#[cfg(target_arch = "wasm32")]
mod custom_getrandom {
    #![allow(clippy::no_mangle_with_rust_abi)]

    use getrandom::{register_custom_getrandom, Error};
    use near_sdk::env;

    register_custom_getrandom!(custom_getrandom);

    #[allow(clippy::unnecessary_wraps)]
    pub fn custom_getrandom(buf: &mut [u8]) -> Result<(), Error> {
        buf.copy_from_slice(&env::random_seed_array());
        Ok(())
    }
}
