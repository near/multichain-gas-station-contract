/// This is a modified version of code from Pyth, distributed under the
/// Apache 2.0 license. The modifications include:
///
/// - Updating the code to use near-sdk version 5.
/// - Stripping out some parts of the API that are not necessary for this project.
///
/// The original source can be found here: <https://github.com/pyth-network/pyth-crosschain/blob/586a4398bd2b1f178ee70a38ff101bd1aec8971f/target_chains/near/receiver/src/state.rs>
///
/// Original license text:
///
/// Copyright 2023 Pyth Contributors.
///
/// Licensed under the Apache License, Version 2.0 (the "License");
/// you may not use this file except in compliance with the License.
/// You may obtain a copy of the License at
///
///     http://www.apache.org/licenses/LICENSE-2.0
///
/// Unless required by applicable law or agreed to in writing, software
/// distributed under the License is distributed on an "AS IS" BASIS,
/// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
/// See the License for the specific language governing permissions and
/// limitations under the License.

use ethers_core::utils::hex;
use near_sdk::{ext_contract, json_types::{I64, U64}, near};

#[near]
#[repr(transparent)]
pub struct PriceIdentifier(pub [u8; 32]);

impl<'de> near_sdk::serde::Deserialize<'de> for PriceIdentifier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: near_sdk::serde::Deserializer<'de>,
    {
        /// A visitor that deserializes a hex string into a 32 byte array.
        struct IdentifierVisitor;

        impl<'de> near_sdk::serde::de::Visitor<'de> for IdentifierVisitor {
            /// Target type for either a hex string or a 32 byte array.
            type Value = [u8; 32];

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a hex string")
            }

            // When given a string, attempt a standard hex decode.
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: near_sdk::serde::de::Error,
            {
                if value.len() != 64 {
                    return Err(E::custom(format!(
                        "expected a 64 character hex string, got {}",
                        value.len()
                    )));
                }
                let mut bytes = [0u8; 32];
                hex::decode_to_slice(value, &mut bytes).map_err(E::custom)?;
                Ok(bytes)
            }
        }

        deserializer
            .deserialize_any(IdentifierVisitor)
            .map(PriceIdentifier)
    }
}

impl near_sdk::serde::Serialize for PriceIdentifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: near_sdk::serde::Serializer,
    {
        serializer.serialize_str(&hex::encode(self.0))
    }
}

/// A price with a degree of uncertainty, represented as a price +- a confidence interval.
///
/// The confidence interval roughly corresponds to the standard error of a normal distribution.
/// Both the price and confidence are stored in a fixed-point numeric representation,
/// `x * (10^expo)`, where `expo` is the exponent.
//
/// Please refer to the documentation at https://docs.pyth.network/documentation/pythnet-price-feeds/best-practices for how
/// to how this price safely.
#[derive(Debug,PartialEq, Eq)]
#[near(serializers = [borsh, json])]
pub struct Price {
    pub price:        I64,
    /// Confidence interval around the price
    pub conf:         U64,
    /// The exponent
    pub expo:         i32,
    /// Unix timestamp of when this price was computed
    pub publish_time: i64,
}

#[ext_contract(ext_pyth)]
pub trait Pyth {
    // See implementations for details, PriceIdentifier can be passed either as a 64 character
    // hex price ID which can be found on the Pyth homepage.
    fn price_feed_exists(&self, price_identifier: PriceIdentifier) -> bool;
    fn get_price(&self, price_identifier: PriceIdentifier) -> Option<Price>;
    fn get_price_unsafe(&self, price_identifier: PriceIdentifier) -> Option<Price>;
    fn get_price_no_older_than(&self, price_id: PriceIdentifier, age: u64) -> Option<Price>;
    fn get_ema_price(&self, price_id: PriceIdentifier) -> Option<Price>;
    fn get_ema_price_unsafe(&self, price_id: PriceIdentifier) -> Option<Price>;
    fn get_ema_price_no_older_than(&self, price_id: PriceIdentifier, age: u64) -> Option<Price>;
}
