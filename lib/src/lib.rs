use std::fmt::Display;

use near_sdk::env;

pub mod foreign_address;
pub mod gas_station;
pub mod kdf;
pub mod oracle;
pub mod signer;

pub trait Rejectable<T> {
    fn unwrap_or_reject(self) -> T;
    fn expect_or_reject(self, msg: impl Display) -> T;
}

impl<T, E: Display> Rejectable<T> for Result<T, E> {
    fn unwrap_or_reject(self) -> T {
        self.unwrap_or_else(|e| env::panic_str(&e.to_string()))
    }

    fn expect_or_reject(self, msg: impl Display) -> T {
        self.unwrap_or_else(|e| env::panic_str(&format!("{}: {e}", msg)))
    }
}

impl<T> Rejectable<T> for Option<T> {
    fn unwrap_or_reject(self) -> T {
        self.unwrap_or_else(|| env::panic_str("Attempted to unwrap None"))
    }

    fn expect_or_reject(self, msg: impl Display) -> T {
        self.unwrap_or_else(|| env::panic_str(&msg.to_string()))
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
