use ethers::{
    types::{transaction::eip2718::TypedTransaction, NameOrAddress, TransactionRequest, U256},
    utils::rlp::{Decodable, Rlp},
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env,
    json_types::U64,
    near_bindgen, require,
    serde::{Deserialize, Serialize},
    store::UnorderedSet,
    AccountId, BorshStorageKey, PanicOnDefault, Promise, PromiseError,
};
use near_sdk_contract_tools::{event, owner::*, standard::nep297::Event, Owner};

mod oracle;

mod signer_contract;
use oracle::{ext_oracle, AssetOptionalPrice, PriceData};
use signer_contract::{ext_signer, MpcSignature};

mod utils;
use utils::*;

mod xchain_address;
use xchain_address::XChainAddress;

type XChainTokenAmount = ethers::types::U256;

/// A successful request will emit two events, one for the request and one for
/// the finalized transaction, in that order. The `id` field will be the same
/// for both events.
///
/// IDs are arbitrarily chosen by the contract. An ID is guaranteed to be unique
/// within the contract.
#[event(version = "0.1.0", standard = "x-multichain-sig")]
pub enum ContractEvent {
    RequestTransactionSignature {
        xchain_id: String,
        sender_address: Option<XChainAddress>,
        unsigned_transaction: String,
        request_tokens_for_gas: Option<XChainTokenAmount>,
    },
    FinalizeTransactionSignature {
        xchain_id: String,
        sender_address: Option<XChainAddress>,
        signed_transaction: String,
        signed_paymaster_transaction: String,
        request_tokens_for_gas: Option<XChainTokenAmount>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct TransactionDetails {
    signed_transaction: String,
    signed_paymaster_transaction: String,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Debug, Default)]
#[serde(crate = "near_sdk::serde")]
pub struct Flags {
    is_sender_whitelist_enabled: bool,
    is_receiver_whitelist_enabled: bool,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct GasTokenPrice {
    pub local_per_xchain: (u128, u128),
    pub updated_at_block_height: u64,
}

#[derive(BorshSerialize, BorshDeserialize, BorshStorageKey, Hash, Clone, Debug, PartialEq, Eq)]
pub enum StorageKey {
    SenderWhitelist,
    ReceiverWhitelist,
}

#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault, Debug, Owner)]
#[near_bindgen]
pub struct Contract {
    /// Identifies the target chain to the off-chain relayer.
    /// For example, "ETH", "zkSync", etc.
    pub foreign_chain: String,
    /// The identifier that the foreign chain uses to identify itself.
    /// For example, 1 for ETH mainnet, 97 for BSC mainnet...
    pub foreign_internal_chain_id: U64,
    pub signer_contract_id: AccountId,
    pub oracle_id: AccountId,
    pub oracle_local_asset_id: String,
    pub oracle_xchain_asset_id: String,
    pub sender_whitelist: UnorderedSet<XChainAddress>,
    pub receiver_whitelist: UnorderedSet<XChainAddress>,
    pub flags: Flags,
    pub gas_token_price: Option<GasTokenPrice>,
    pub price_scale: (u128, u128),
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(
        foreign_chain: String,
        foreign_internal_chain_id: U64,
        signer_contract_id: AccountId,
        oracle_id: AccountId,
        oracle_local_asset_id: String,
        oracle_xchain_asset_id: String,
    ) -> Self {
        let mut contract = Self {
            foreign_chain,
            foreign_internal_chain_id,
            signer_contract_id,
            oracle_id,
            oracle_local_asset_id,
            oracle_xchain_asset_id,
            sender_whitelist: UnorderedSet::new(StorageKey::SenderWhitelist),
            receiver_whitelist: UnorderedSet::new(StorageKey::ReceiverWhitelist),
            flags: Flags::default(),
            gas_token_price: None,
            price_scale: (120, 100), // +20% on top of market price
        };

        Owner::init(&mut contract, &env::predecessor_account_id());

        contract.fetch_oracle(); // update oracle immediately

        contract
    }

    // Public contract config getters/setters

    pub fn get_flags(&self) -> &Flags {
        &self.flags
    }

    pub fn set_flags(&mut self, flags: Flags) {
        self.assert_owner();
        self.flags = flags;
    }

    pub fn get_receiver_whitelist(&self) -> Vec<&XChainAddress> {
        self.receiver_whitelist.iter().collect()
    }

    pub fn add_to_receiver_whitelist(&mut self, addresses: Vec<XChainAddress>) {
        self.assert_owner();
        for address in addresses {
            self.receiver_whitelist.insert(address);
        }
    }

    pub fn remove_from_receiver_whitelist(&mut self, addresses: Vec<XChainAddress>) {
        self.assert_owner();
        for address in addresses {
            self.receiver_whitelist.remove(&address);
        }
    }

    pub fn clear_receiver_whitelist(&mut self) {
        self.assert_owner();
        self.receiver_whitelist.clear();
    }

    pub fn get_sender_whitelist(&self) -> Vec<&XChainAddress> {
        self.sender_whitelist.iter().collect()
    }

    pub fn add_to_sender_whitelist(&mut self, addresses: Vec<XChainAddress>) {
        self.assert_owner();
        for address in addresses {
            self.sender_whitelist.insert(address);
        }
    }

    pub fn remove_from_sender_whitelist(&mut self, addresses: Vec<XChainAddress>) {
        self.assert_owner();
        for address in addresses {
            self.sender_whitelist.remove(&address);
        }
    }

    pub fn clear_sender_whitelist(&mut self) {
        self.assert_owner();
        self.sender_whitelist.clear();
    }

    pub fn fetch_oracle(&mut self) -> Promise {
        // TODO: Does this method need access control or assert_one_yocto?
        ext_oracle::ext(self.oracle_id.clone()).get_price_data(Some(vec![
            self.oracle_local_asset_id.clone(),
            self.oracle_xchain_asset_id.clone(),
        ]))
    }

    #[private]
    pub fn fetch_oracle_callback(
        &mut self,
        #[callback_result] result: Result<PriceData, PromiseError>,
    ) {
        let price_data = result.unwrap_or_else(|_| env::panic_str("Failed to fetch price data"));

        let (local_price, xchain_price) = match &price_data.prices[..] {
            [AssetOptionalPrice {
                asset_id: first_asset_id,
                price: Some(first_price),
            }, AssetOptionalPrice {
                asset_id: second_asset_id,
                price: Some(second_price),
            }] if first_asset_id == &self.oracle_local_asset_id
                && second_asset_id == &self.oracle_xchain_asset_id =>
            {
                (first_price, second_price)
            }
            _ => env::panic_str("Invalid price data"),
        };

        self.gas_token_price = Some(GasTokenPrice {
            local_per_xchain: (
                xchain_price.multiplier.0 * u128::from(local_price.decimals),
                local_price.multiplier.0 * u128::from(xchain_price.decimals),
            ),
            updated_at_block_height: env::block_height(),
        });
    }

    // Private helper methods

    fn price_of_gas(&self, request_tokens_for_gas: XChainTokenAmount) -> Option<u128> {
        // calculate fee based on currently known price, and include scaling factor
        // TODO: Check price data freshness
        let conversion_rate = self.gas_token_price.as_ref()?.local_per_xchain;
        let a =
            request_tokens_for_gas * U256::from(conversion_rate.0) * U256::from(self.price_scale.0);
        let (b, rem) = a.div_mod(U256::from(conversion_rate.1) * U256::from(self.price_scale.1));
        // round up
        Some(if rem.is_zero() { b } else { b + 1 }.as_u128())
    }

    fn validate_transaction(&self, transaction: &TypedTransaction) {
        require!(
            transaction.gas().is_some() && transaction.gas_price().is_some(),
            "Gas must be explicitly specified",
        );

        if let Some(chain_id) = transaction.chain_id() {
            require!(
                chain_id.as_u64() == self.foreign_internal_chain_id.0,
                "Chain ID mismatch"
            );
        }

        // Validate receiver
        let receiver: Option<XChainAddress> = match transaction.to() {
            Some(NameOrAddress::Name(_)) => {
                env::panic_str("ENS names are not supported");
            }
            Some(NameOrAddress::Address(address)) => Some(address.into()),
            None => None,
        };

        // Validate receiver
        if let Some(ref receiver) = receiver {
            // Check receiver whitelist
            if self.flags.is_receiver_whitelist_enabled {
                require!(
                    self.receiver_whitelist.contains(receiver),
                    "Receiver is not whitelisted",
                );
            }
        } else {
            // No receiver means contract deployment
            env::panic_str("Deployment is not allowed");
        };

        // Check sender whitelist
        if self.flags.is_sender_whitelist_enabled {
            require!(
                self.sender_whitelist.contains(
                    &transaction
                        .from()
                        .unwrap_or_else(|| env::panic_str("Sender whitelist is enabled"))
                        .into()
                ),
                "Sender is not whitelisted",
            );
        }
    }

    fn request_signature(&mut self, key_path: String, transaction: &TypedTransaction) -> Promise {
        ext_signer::ext(self.signer_contract_id.clone()) // TODO: Gas.
            .sign(transaction.sighash().0, key_path)
    }

    // Public methods

    #[payable]
    pub fn xchain_sign(
        &mut self,
        transaction_json: Option<TypedTransaction>,
        transaction_rlp: Option<String>,
    ) -> Promise {
        let mut transaction = extract_transaction(transaction_json, transaction_rlp);

        self.validate_transaction(&transaction);
        transaction.set_chain_id(self.foreign_internal_chain_id.0);

        let request_tokens_for_gas = tokens_for_gas(&transaction).unwrap(); // Validation ensures gas is set.
        let fee = self
            .price_of_gas(request_tokens_for_gas)
            .unwrap_or_else(|| env::panic_str("No gas price available"));
        let deposit = env::attached_deposit();

        match deposit.checked_sub(fee) {
            None => {
                env::panic_str(&format!(
                    "Attached deposit ({deposit}) is less than fee ({fee})"
                ));
            }
            Some(0) => {} // No refund; payment is exact.
            Some(refund) => {
                // Refund excess
                Promise::new(env::predecessor_account_id()).transfer(refund);
            }
        }

        ContractEvent::RequestTransactionSignature {
            xchain_id: self.foreign_chain.clone(),
            sender_address: transaction.from().map(Into::into),
            unsigned_transaction: hex::encode(&transaction.rlp()),
            request_tokens_for_gas: Some(request_tokens_for_gas),
        }
        .emit();

        let paymaster_transaction: TypedTransaction = TransactionRequest {
            chain_id: Some(self.foreign_internal_chain_id.0.into()),
            from: None, // TODO: PK gen
            to: Some((*transaction.from().unwrap()).into()),
            value: Some(request_tokens_for_gas),
            ..Default::default()
        }
        .into();

        self.request_signature("$".to_string(), &paymaster_transaction)
            .and(self.request_signature(env::predecessor_account_id().to_string(), &transaction))
            .then(
                Self::ext(env::current_account_id())
                    .request_signature_callback(paymaster_transaction, transaction),
            )
    }

    #[private]
    pub fn request_signature_callback(
        &mut self,
        paymaster_transaction: TypedTransaction,
        transaction: TypedTransaction,
        #[callback_result] paymaster_result: Result<MpcSignature, PromiseError>,
        #[callback_result] result: Result<MpcSignature, PromiseError>,
    ) -> TransactionDetails {
        fn unwrap_signature(
            result: Result<MpcSignature, PromiseError>,
        ) -> ethers::types::Signature {
            result
                .unwrap_or_else(|e| env::panic_str(&format!("Failed to produce signature: {e:?}")))
                .try_into()
                .unwrap_or_else(|e| env::panic_str(&format!("Failed to decode signature: {e:?}")))
        }

        let signature = unwrap_signature(result);
        let paymaster_signature = unwrap_signature(paymaster_result);

        let rlp_signed = transaction.rlp_signed(&signature);
        let rlp_signed_hex = hex::encode(&rlp_signed);

        let request_tokens_for_gas = tokens_for_gas(&transaction);

        let paymaster_rlp_signed = paymaster_transaction.rlp_signed(&paymaster_signature);
        let paymaster_rlp_signed_hex = hex::encode(&paymaster_rlp_signed);

        ContractEvent::FinalizeTransactionSignature {
            xchain_id: self.foreign_chain.clone(),
            sender_address: transaction.from().map(Into::into),
            signed_transaction: rlp_signed_hex.clone(),
            signed_paymaster_transaction: paymaster_rlp_signed_hex.clone(),
            request_tokens_for_gas,
        }
        .emit();

        TransactionDetails {
            signed_transaction: rlp_signed_hex,
            signed_paymaster_transaction: paymaster_rlp_signed_hex,
        }
    }
}

fn extract_transaction(
    transaction_json: Option<TypedTransaction>,
    transaction_rlp: Option<String>,
) -> TypedTransaction {
    transaction_json
        .or_else(|| {
            transaction_rlp.map(|rlp_hex| {
                let rlp_bytes = hex::decode(rlp_hex)
                    .unwrap_or_else(|_| env::panic_str("Error decoding `transaction_rlp` as hex"));
                let rlp = Rlp::new(&rlp_bytes);
                TypedTransaction::decode(&rlp).unwrap_or_else(|_| {
                    env::panic_str("Error decoding `transaction_rlp` as transaction RLP")
                })
            })
        })
        .unwrap_or_else(|| {
            env::panic_str(
                "A transaction must be provided in `transaction_json` or `transaction_rlp`",
            )
        })
}
