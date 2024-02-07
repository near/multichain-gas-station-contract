use std::str::FromStr;

use ethers_core::{
    types::{transaction::eip2718::TypedTransaction, TransactionRequest, U256},
    utils::rlp::{Decodable, Rlp},
};
use getrandom::{register_custom_getrandom, Error};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env,
    json_types::{U128, U64},
    near_bindgen, require,
    serde::{Deserialize, Serialize},
    store::{UnorderedMap, UnorderedSet, Vector},
    AccountId, BorshStorageKey, PanicOnDefault, Promise, PromiseError, PromiseOrValue,
};
use near_sdk_contract_tools::{event, ft::ext_nep141, owner::*, standard::nep297::Event, Owner};
use schemars::JsonSchema;

pub mod foreign_address;
use foreign_address::ForeignAddress;

pub mod kdf;
use kdf::get_mpc_address;

pub mod valid_transaction_request;
use valid_transaction_request::ValidTransactionRequest;

pub mod oracle;
use oracle::{ext_oracle, process_oracle_result, PriceData};

pub mod signer_contract;
use signer_contract::{ext_signer, MpcSignature};

pub mod signature_request;
use signature_request::{SignatureRequest, SignatureRequestStatus};

pub type ForeignChainTokenAmount = ethers_core::types::U256;

// TODO: Storage management
/// A successful request will emit two events, one for the request and one for
/// the finalized transaction, in that order. The `id` field will be the same
/// for both events.
///
/// IDs are arbitrarily chosen by the contract. An ID is guaranteed to be unique
/// within the contract.
#[event(version = "0.1.0", standard = "x-multichain-sig")]
pub enum ContractEvent {
    TransactionSequenceCreated {
        foreign_chain_id: String,
        pending_transaction_sequence: PendingTransactionSequence,
    },
    TransactionSequenceSigned {
        foreign_chain_id: String,
        sender_local_address: AccountId,
        signed_transactions: Vec<String>,
    },
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq, Eq)]
#[serde(crate = "near_sdk::serde")]
pub struct TransactionDetails {
    pub signed_transaction: String,
    pub signed_paymaster_transaction: String,
}

#[derive(
    BorshSerialize,
    BorshDeserialize,
    Serialize,
    Deserialize,
    JsonSchema,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
)]
#[serde(crate = "near_sdk::serde")]
pub struct Flags {
    pub is_sender_whitelist_enabled: bool,
    pub is_receiver_whitelist_enabled: bool,
}

#[derive(
    Serialize,
    Deserialize,
    BorshSerialize,
    BorshDeserialize,
    JsonSchema,
    Clone,
    Debug,
    PartialEq,
    Eq,
)]
#[serde(crate = "near_sdk::serde")]
pub struct TransactionCreation {
    pub id: U64,
    pub pending_signature_count: u32,
}

#[derive(
    Serialize,
    Deserialize,
    BorshSerialize,
    BorshDeserialize,
    JsonSchema,
    Clone,
    Debug,
    PartialEq,
    Eq,
)]
#[serde(crate = "near_sdk::serde")]
pub struct PaymasterConfiguration {
    pub nonce: u32,
    pub key_path: String,
}

impl PaymasterConfiguration {
    pub fn next_nonce(&mut self) -> u32 {
        let nonce = self.nonce;
        self.nonce += 1;
        nonce
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct ForeignChainConfiguration {
    pub paymasters: Vector<PaymasterConfiguration>,
    pub next_paymaster: u32,
    pub transfer_gas: [u64; 4],
    pub fee_rate: (u128, u128),
    pub oracle_asset_id: String,
}

impl ForeignChainConfiguration {
    pub fn transfer_gas(&self) -> U256 {
        U256(self.transfer_gas)
    }

    pub fn next_paymaster(&mut self) -> Option<&mut PaymasterConfiguration> {
        let next_paymaster = self.next_paymaster;
        self.next_paymaster = (self.next_paymaster + 1) % self.paymasters.len();
        let paymaster = self.paymasters.get_mut(next_paymaster);
        paymaster
    }

    fn foreign_token_price(
        &self,
        oracle_local_asset_id: &str,
        price_data: &PriceData,
        foreign_tokens: ForeignChainTokenAmount,
    ) -> u128 {
        let foreign_token_price =
            process_oracle_result(oracle_local_asset_id, &self.oracle_asset_id, price_data);

        // calculate fee based on currently known price, and include fee rate
        let a = foreign_tokens * U256::from(foreign_token_price.0) * U256::from(self.fee_rate.0);
        let (b, rem) = a.div_mod(U256::from(foreign_token_price.1) * U256::from(self.fee_rate.1));
        // round up
        if rem.is_zero() { b } else { b + 1 }.as_u128()
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(crate = "near_sdk::serde")]
pub struct GetForeignChain {
    pub chain_id: U64,
    pub oracle_asset_id: String,
}

#[derive(
    Serialize,
    Deserialize,
    BorshSerialize,
    BorshDeserialize,
    JsonSchema,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Clone,
    Debug,
)]
#[serde(crate = "near_sdk::serde")]
pub enum AssetId {
    Native,
    Nep141(AccountId),
}

impl AssetId {
    pub fn transfer(&self, receiver_id: AccountId, amount: impl Into<u128>) -> Promise {
        match self {
            AssetId::Native => Promise::new(receiver_id).transfer(amount.into()),
            AssetId::Nep141(contract_id) => ext_nep141::ext(contract_id.clone()).ft_transfer(
                receiver_id,
                U128(amount.into()),
                None,
            ),
        }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, BorshSerialize, BorshDeserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct AssetBalance {
    pub asset_id: AssetId,
    pub amount: U128,
}

impl AssetBalance {
    pub fn native(amount: impl Into<U128>) -> Self {
        Self {
            asset_id: AssetId::Native,
            amount: amount.into(),
        }
    }

    pub fn nep141(account_id: AccountId, amount: impl Into<U128>) -> Self {
        Self {
            asset_id: AssetId::Nep141(account_id),
            amount: amount.into(),
        }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, BorshSerialize, BorshDeserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct PendingTransactionSequence {
    pub created_by_id: AccountId,
    pub signature_requests: Vec<SignatureRequest>,
    pub created_at_block_timestamp_ns: U64,
    pub escrow: Option<AssetBalance>,
}

impl PendingTransactionSequence {
    pub fn all_signed(&self) -> bool {
        self.signature_requests
            .iter()
            .all(SignatureRequest::is_signed)
    }
}

#[derive(BorshSerialize, BorshDeserialize, BorshStorageKey, Hash, Clone, Debug, PartialEq, Eq)]
pub enum StorageKey {
    SenderWhitelist,
    ReceiverWhitelist,
    ForeignChains,
    Paymasters(u64),
    PendingTransactions,
    CollectedFees,
}

// TODO: Pausability
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault, Debug, Owner)]
#[near_bindgen]
pub struct Contract {
    pub next_unique_id: u64,
    pub signer_contract_id: AccountId,
    pub signer_contract_public_key: Option<near_sdk::PublicKey>,
    pub oracle_id: AccountId,
    pub oracle_local_asset_id: String,
    pub flags: Flags,
    pub expire_transaction_after_ns: u64, // TODO: Make configurable
    pub foreign_chains: UnorderedMap<u64, ForeignChainConfiguration>,
    pub sender_whitelist: UnorderedSet<AccountId>,
    pub receiver_whitelist: UnorderedSet<ForeignAddress>,
    pub pending_transaction_sequences: UnorderedMap<u64, PendingTransactionSequence>,
    pub collected_fees: UnorderedMap<AssetId, U128>,
}

#[near_bindgen]
impl Contract {
    #[cfg(feature = "new_debug")]
    #[init(ignore_state)]
    pub fn new_debug(
        signer_contract_id: AccountId,
        oracle_id: AccountId,
        oracle_local_asset_id: String,
    ) -> Self {
        let mut contract = Self {
            next_unique_id: 0,
            signer_contract_id,
            signer_contract_public_key: None, // Loaded asynchronously
            oracle_id,
            oracle_local_asset_id,
            flags: Flags::default(),
            expire_transaction_after_ns: 5 * 60 * 1_000_000_000, // 5 minutes
            foreign_chains: UnorderedMap::new(StorageKey::ForeignChains),
            sender_whitelist: UnorderedSet::new(StorageKey::SenderWhitelist),
            receiver_whitelist: UnorderedSet::new(StorageKey::ReceiverWhitelist),
            pending_transaction_sequences: UnorderedMap::new(StorageKey::PendingTransactions),
            collected_fees: UnorderedMap::new(StorageKey::CollectedFees),
        };

        Owner::update_owner(&mut contract, Some(env::predecessor_account_id()));

        contract
    }

    #[init]
    pub fn new(
        signer_contract_id: AccountId,
        oracle_id: AccountId,
        oracle_local_asset_id: String,
    ) -> Self {
        let mut contract = Self {
            next_unique_id: 0,
            signer_contract_id,
            signer_contract_public_key: None, // Loaded asynchronously
            oracle_id,
            oracle_local_asset_id,
            flags: Flags::default(),
            expire_transaction_after_ns: 5 * 60 * 1_000_000_000, // 5 minutes
            foreign_chains: UnorderedMap::new(StorageKey::ForeignChains),
            sender_whitelist: UnorderedSet::new(StorageKey::SenderWhitelist),
            receiver_whitelist: UnorderedSet::new(StorageKey::ReceiverWhitelist),
            pending_transaction_sequences: UnorderedMap::new(StorageKey::PendingTransactions),
            collected_fees: UnorderedMap::new(StorageKey::CollectedFees),
        };

        Owner::init(&mut contract, &env::predecessor_account_id());

        contract
    }

    // Public contract config getters/setters

    pub fn refresh_signer_public_key(&mut self) -> Promise {
        self.assert_owner();

        ext_signer::ext(self.signer_contract_id.clone())
            .public_key()
            .then(Self::ext(env::current_account_id()).refresh_signer_public_key_callback())
    }

    #[private]
    pub fn refresh_signer_public_key_callback(
        &mut self,
        #[callback_result] public_key: Result<near_sdk::PublicKey, PromiseError>,
    ) {
        let public_key = public_key.unwrap_or_else(|_| {
            env::panic_str("Failed to load signer public key from the signer contract")
        });
        self.signer_contract_public_key = Some(public_key);
    }

    pub fn get_flags(&self) -> &Flags {
        &self.flags
    }

    pub fn set_flags(&mut self, flags: Flags) {
        self.assert_owner();
        self.flags = flags;
    }

    pub fn get_receiver_whitelist(&self) -> Vec<&ForeignAddress> {
        self.receiver_whitelist.iter().collect()
    }

    pub fn add_to_receiver_whitelist(&mut self, addresses: Vec<ForeignAddress>) {
        self.assert_owner();
        for address in addresses {
            self.receiver_whitelist.insert(address);
        }
    }

    pub fn remove_from_receiver_whitelist(&mut self, addresses: Vec<ForeignAddress>) {
        self.assert_owner();
        for address in addresses {
            self.receiver_whitelist.remove(&address);
        }
    }

    pub fn clear_receiver_whitelist(&mut self) {
        self.assert_owner();
        self.receiver_whitelist.clear();
    }

    pub fn get_sender_whitelist(&self) -> Vec<&AccountId> {
        self.sender_whitelist.iter().collect()
    }

    pub fn add_to_sender_whitelist(&mut self, addresses: Vec<AccountId>) {
        self.assert_owner();
        for address in addresses {
            self.sender_whitelist.insert(address);
        }
    }

    pub fn remove_from_sender_whitelist(&mut self, addresses: Vec<AccountId>) {
        self.assert_owner();
        for address in addresses {
            self.sender_whitelist.remove(&address);
        }
    }

    pub fn clear_sender_whitelist(&mut self) {
        self.assert_owner();
        self.sender_whitelist.clear();
    }

    pub fn add_foreign_chain(
        &mut self,
        chain_id: U64,
        oracle_asset_id: String,
        transfer_gas: U128,
        fee_rate: (U128, U128),
    ) {
        self.assert_owner();

        self.foreign_chains.insert(
            chain_id.0,
            ForeignChainConfiguration {
                next_paymaster: 0,
                oracle_asset_id,
                transfer_gas: U256::from(transfer_gas.0).0,
                fee_rate: (fee_rate.0.into(), fee_rate.1.into()),
                paymasters: Vector::new(StorageKey::Paymasters(chain_id.0)),
            },
        );
    }

    pub fn set_foreign_chain_oracle_asset_id(&mut self, chain_id: U64, oracle_asset_id: String) {
        self.assert_owner();
        if let Some(config) = self.foreign_chains.get_mut(&chain_id.0) {
            config.oracle_asset_id = oracle_asset_id;
        } else {
            env::panic_str("Foreign chain does not exist");
        }
    }

    pub fn set_foreign_chain_transfer_gas(&mut self, chain_id: U64, transfer_gas: U128) {
        self.assert_owner();
        if let Some(config) = self.foreign_chains.get_mut(&chain_id.0) {
            config.transfer_gas = U256::from(transfer_gas.0).0;
        } else {
            env::panic_str("Foreign chain does not exist");
        }
    }

    pub fn remove_foreign_chain(&mut self, chain_id: U64) {
        self.assert_owner();
        if let Some((_, mut config)) = self.foreign_chains.remove_entry(&chain_id.0) {
            config.paymasters.clear();
        }
    }

    pub fn get_foreign_chains(&self) -> Vec<GetForeignChain> {
        self.foreign_chains
            .iter()
            .map(|(chain_id, config)| GetForeignChain {
                chain_id: (*chain_id).into(),
                oracle_asset_id: config.oracle_asset_id.clone(),
            })
            .collect()
    }

    pub fn add_paymaster(&mut self, chain_id: U64, nonce: u32, key_path: String) -> u32 {
        self.assert_owner();

        require!(
            AccountId::from_str(&key_path).is_err(),
            "Paymaster key path must not be a valid account id",
        );

        let chain = self
            .foreign_chains
            .get_mut(&chain_id.0)
            .unwrap_or_else(|| env::panic_str("Foreign chain does not exist"));

        let index = chain.paymasters.len();

        chain
            .paymasters
            .push(PaymasterConfiguration { nonce, key_path });

        index
    }

    pub fn set_paymaster_nonce(&mut self, chain_id: U64, index: u32, nonce: u32) {
        self.assert_owner();
        let chain = self
            .foreign_chains
            .get_mut(&chain_id.0)
            .unwrap_or_else(|| env::panic_str("Foreign chain does not exist"));

        let paymaster = chain.paymasters.get_mut(index).unwrap_or_else(|| {
            env::panic_str("Invalid index");
        });

        paymaster.nonce = nonce;
    }

    /// Note: If a transaction is _already_ pending signatures with the
    /// paymaster getting removed, this method will not prevent those payloads
    /// from getting signed.
    pub fn remove_paymaster(&mut self, chain_id: U64, index: u32) {
        self.assert_owner();
        let chain = self
            .foreign_chains
            .get_mut(&chain_id.0)
            .unwrap_or_else(|| env::panic_str("Foreign chain does not exist"));

        if index < chain.paymasters.len() {
            chain.paymasters.swap_remove(index);
            chain.next_paymaster %= chain.paymasters.len();
        } else {
            env::panic_str("Invalid index");
        }
    }

    pub fn get_paymasters(&self, chain_id: U64) -> Vec<&PaymasterConfiguration> {
        self.foreign_chains
            .get(&chain_id.0)
            .unwrap_or_else(|| env::panic_str("Foreign chain does not exist"))
            .paymasters
            .iter()
            .collect()
    }

    pub fn list_transactions(
        &self,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> std::collections::HashMap<String, &PendingTransactionSequence> {
        let mut v: Vec<_> = self.pending_transaction_sequences.iter().collect();

        v.sort_by_cached_key(|&(id, _)| *id);

        v.into_iter()
            .skip(offset.map_or(0, |o| o as usize))
            .take(limit.map_or(usize::MAX, |l| l as usize))
            .map(|(id, tx)| (id.to_string(), tx))
            .collect()
    }

    pub fn get_transaction(&self, id: U64) -> Option<&PendingTransactionSequence> {
        self.pending_transaction_sequences.get(&id.0)
    }

    pub fn withdraw_collected_fees(&mut self, asset_id: AssetId, amount: Option<U128>) -> Promise {
        near_sdk::assert_one_yocto();
        self.assert_owner();
        let fees = self
            .collected_fees
            .get_mut(&asset_id)
            .unwrap_or_else(|| env::panic_str("No fee entry for provided asset ID"));

        let amount = amount.unwrap_or(U128(fees.0));

        fees.0 = fees
            .0
            .checked_sub(amount.0)
            .unwrap_or_else(|| env::panic_str("Not enough fees to withdraw"));

        asset_id.transfer(self.own_get_owner().unwrap(), amount)
    }

    pub fn get_collected_fees(&self) -> std::collections::HashMap<&AssetId, &U128> {
        self.collected_fees.iter().collect()
    }

    pub fn estimate_gas_cost(&self, transaction_rlp_hex: String, price_data: PriceData) -> U128 {
        let transaction =
            ValidTransactionRequest::try_from(decode_transaction_request(&transaction_rlp_hex))
                .unwrap_or_else(|e| env::panic_str(&format!("Invalid transaction request: {e}")));

        let foreign_chain_configuration = self
            .foreign_chains
            .get(&transaction.chain_id)
            .unwrap_or_else(|| {
                env::panic_str(&format!(
                    "Paymaster not supported for chain id {}",
                    transaction.chain_id
                ))
            });

        let paymaster_transaction_gas = foreign_chain_configuration.transfer_gas();
        let request_tokens_for_gas =
            (transaction.gas() + paymaster_transaction_gas) * transaction.gas_price();

        foreign_chain_configuration
            .foreign_token_price(
                &self.oracle_local_asset_id,
                &price_data,
                request_tokens_for_gas,
            )
            .into()
    }

    // Private helper methods

    fn generate_unique_id(&mut self) -> u64 {
        let id = self.next_unique_id;
        self.next_unique_id = self.next_unique_id.checked_add(1).unwrap_or_else(|| {
            env::panic_str("Failed to generate unique ID");
        });
        id
    }

    fn filter_transaction(&self, sender_id: &AccountId, transaction: &ValidTransactionRequest) {
        // Check receiver whitelist
        if self.flags.is_receiver_whitelist_enabled {
            require!(
                self.receiver_whitelist.contains(&transaction.receiver),
                "Receiver is not whitelisted",
            );
        }

        // Check sender whitelist
        if self.flags.is_sender_whitelist_enabled {
            require!(
                self.sender_whitelist.contains(sender_id),
                "Sender is not whitelisted",
            );
        }
    }

    fn insert_pending_transaction(
        &mut self,
        pending_transaction: PendingTransactionSequence,
    ) -> TransactionCreation {
        let pending_signature_count = pending_transaction.signature_requests.len() as u32;

        let id = self.generate_unique_id();

        self.pending_transaction_sequences
            .insert(id, pending_transaction);

        TransactionCreation {
            id: id.into(),
            pending_signature_count,
        }
    }

    // Public methods

    #[payable]
    pub fn create_transaction(
        &mut self,
        transaction_rlp_hex: String,
        use_paymaster: Option<bool>,
    ) -> PromiseOrValue<TransactionCreation> {
        let deposit = env::attached_deposit();
        require!(deposit > 0, "Deposit is required to pay for gas");

        let transaction =
            ValidTransactionRequest::try_from(decode_transaction_request(&transaction_rlp_hex))
                .unwrap_or_else(|e| env::panic_str(&format!("Invalid transaction request: {e}")));

        // Guarantees invariants required in callback
        self.filter_transaction(&env::predecessor_account_id(), &transaction);

        let use_paymaster = use_paymaster.unwrap_or(false);

        if use_paymaster {
            let chain_id = transaction.chain_id();
            let foreign_chain_configuration = self
                .foreign_chains
                .get(&chain_id.as_u64())
                .unwrap_or_else(|| {
                    env::panic_str(&format!("Paymaster not supported for chain id {chain_id}"))
                });

            ext_oracle::ext(self.oracle_id.clone())
                .get_price_data(Some(vec![
                    self.oracle_local_asset_id.clone(),
                    foreign_chain_configuration.oracle_asset_id.clone(),
                ]))
                .then(
                    Self::ext(env::current_account_id()).create_transaction_callback(
                        env::predecessor_account_id(),
                        deposit.into(),
                        transaction,
                    ),
                )
                .into()
        } else {
            let predecessor = env::predecessor_account_id();

            let chain_id = transaction.chain_id;

            let pending_transaction_sequence = PendingTransactionSequence {
                signature_requests: vec![SignatureRequest::new(&predecessor, transaction)],
                created_by_id: predecessor,
                created_at_block_timestamp_ns: env::block_timestamp().into(),
                escrow: None,
            };

            ContractEvent::TransactionSequenceCreated {
                foreign_chain_id: chain_id.to_string(),
                pending_transaction_sequence: pending_transaction_sequence.clone(),
            }
            .emit();

            PromiseOrValue::Value(self.insert_pending_transaction(pending_transaction_sequence))
        }
    }

    #[private]
    pub fn create_transaction_callback(
        &mut self,
        #[serializer(borsh)] sender: AccountId,
        #[serializer(borsh)] deposit: near_sdk::json_types::U128,
        #[serializer(borsh)] transaction_request: ValidTransactionRequest,
        #[callback_result] result: Result<PriceData, PromiseError>,
    ) -> TransactionCreation {
        // TODO: Ensure that deposit is returned if any recoverable errors are encountered.
        let foreign_chain_configuration = self
            .foreign_chains
            .get_mut(&transaction_request.chain_id)
            .unwrap_or_else(|| {
                env::panic_str(&format!(
                    "Paymaster not supported for chain id {}",
                    transaction_request.chain_id
                ))
            });

        let price_data = result.unwrap_or_else(|_| env::panic_str("Failed to fetch price data"));

        let paymaster_transaction_gas = foreign_chain_configuration.transfer_gas();
        let gas_price = transaction_request.gas_price();
        let request_tokens_for_gas =
            (transaction_request.gas() + paymaster_transaction_gas) * gas_price; // Validation ensures gas is set.

        let fee = foreign_chain_configuration.foreign_token_price(
            &self.oracle_local_asset_id,
            &price_data,
            request_tokens_for_gas,
        );
        let deposit = deposit.0;

        match deposit.checked_sub(fee) {
            None => {
                env::panic_str(&format!(
                    "Attached deposit ({deposit}) is less than fee ({fee})"
                ));
            }
            Some(0) => {} // No refund; payment is exact.
            Some(refund) => {
                // Refund excess
                Promise::new(sender.clone()).transfer(refund);
            }
        }

        let paymaster = foreign_chain_configuration
            .next_paymaster()
            .unwrap_or_else(|| env::panic_str("No paymasters found"));

        let sender_foreign_address = get_mpc_address(
            self.signer_contract_public_key.clone().unwrap_or_else(|| {
                env::panic_str("The signer contract public key must be refreshed by calling `refresh_signer_public_key`")
            }),
            &env::current_account_id(),
            sender.as_str(),
        )
        .unwrap_or_else(|e| env::panic_str(&format!("Failed to calculate MPC address: {e}")));

        let chain_id = transaction_request.chain_id;

        let paymaster_transaction = ValidTransactionRequest {
            chain_id,
            receiver: sender_foreign_address,
            value: request_tokens_for_gas.0,
            gas: paymaster_transaction_gas.0,
            gas_price: gas_price.0,
            data: vec![],
            nonce: U256::from(paymaster.next_nonce()).0,
        };

        let signature_requests = vec![
            SignatureRequest::new(&paymaster.key_path, paymaster_transaction),
            SignatureRequest::new(&sender, transaction_request.clone()),
        ];

        let pending_transaction_sequence = PendingTransactionSequence {
            signature_requests,
            created_by_id: sender,
            created_at_block_timestamp_ns: env::block_timestamp().into(),
            escrow: Some(AssetBalance::native(fee)),
        };

        ContractEvent::TransactionSequenceCreated {
            foreign_chain_id: chain_id.to_string(),
            pending_transaction_sequence: pending_transaction_sequence.clone(),
        }
        .emit();

        self.insert_pending_transaction(pending_transaction_sequence)
    }

    pub fn sign_next(&mut self, id: U64) -> Promise {
        let id = id.0;

        let transaction = self
            .pending_transaction_sequences
            .get_mut(&id)
            .unwrap_or_else(|| {
                env::panic_str(&format!("Transaction signature request {id} not found"))
            });

        // ensure not expired
        require!(
            env::block_timestamp()
                <= self.expire_transaction_after_ns + transaction.created_at_block_timestamp_ns.0,
            "Transaction is expired",
        );

        // ensure only signed by original creator
        require!(
            transaction.created_by_id == env::predecessor_account_id(),
            "Predecessor must be the transaction creator",
        );

        let (index, next_signature_request) = transaction
            .signature_requests
            .iter_mut()
            .enumerate()
            .find(|(_, r)| r.is_pending())
            .unwrap_or_else(|| env::panic_str("No pending or non-in-flight signature requests"));

        next_signature_request.status = SignatureRequestStatus::InFlight;

        ext_signer::ext(self.signer_contract_id.clone()) // TODO: Gas.
            .sign(
                <TypedTransaction as From<ValidTransactionRequest>>::from(
                    next_signature_request.transaction.clone(),
                )
                .sighash()
                .0,
                &next_signature_request.key_path,
            )
            .then(Self::ext(env::current_account_id()).sign_next_callback(id.into(), index as u32))
    }

    #[private]
    pub fn sign_next_callback(
        &mut self,
        id: U64,
        index: u32,
        #[callback_result] result: Result<MpcSignature, PromiseError>,
    ) -> String {
        let id = id.0;

        let pending_transaction_sequence = self
            .pending_transaction_sequences
            .get_mut(&id)
            .unwrap_or_else(|| env::panic_str(&format!("Pending transaction {id} not found")));

        let request = pending_transaction_sequence
            .signature_requests
            .get_mut(index as usize)
            .unwrap_or_else(|| {
                env::panic_str(&format!(
                    "Signature request {id}.{index} not found in transaction",
                ))
            });

        if !request.is_in_flight() {
            env::panic_str(&format!(
                "Inconsistent state: Signature request {id}.{index} should be in-flight but is not"
            ));
        }

        // TODO: What to do if signing fails?
        let signature = result
            .unwrap_or_else(|e| env::panic_str(&format!("Failed to produce signature: {e:?}")))
            .try_into()
            .unwrap_or_else(|e| env::panic_str(&format!("Failed to decode signature: {e:?}")));

        let transaction: TypedTransaction = request.transaction.clone().into();

        let rlp_signed = transaction.rlp_signed(&signature);

        request.set_signature(signature);

        // Remove escrow from record.
        // This is important to ensuring that refund logic works correctly.
        if let Some(escrow) = pending_transaction_sequence.escrow.take() {
            let collected_fees = self
                .collected_fees
                .entry(escrow.asset_id.clone())
                .or_insert(U128(0));
            collected_fees.0 += escrow.amount.0;
        }

        let chain_id = request.transaction.chain_id;

        let all_signatures = pending_transaction_sequence
            .signature_requests
            .iter()
            .try_fold(vec![], |mut v, r| {
                if let SignatureRequestStatus::Signed { signature } = &r.status {
                    v.push((r.transaction.clone(), signature.clone()));
                    Some(v)
                } else {
                    None
                }
            });

        if let Some(all_signatures) = all_signatures {
            ContractEvent::TransactionSequenceSigned {
                foreign_chain_id: chain_id.to_string(),
                sender_local_address: pending_transaction_sequence.created_by_id.clone(),
                signed_transactions: all_signatures
                    .into_iter()
                    .map(|(t, s)| hex::encode(t.into_typed_transaction().rlp_signed(&s.into())))
                    .collect(),
            }
            .emit();
            // Remove transaction if all requests have been signed
            // TODO: Is this over-eager?
            self.pending_transaction_sequences.remove(&id);
        }

        hex::encode(&rlp_signed)
    }

    pub fn remove_transaction(&mut self, id: U64) -> PromiseOrValue<()> {
        let transaction = self
            .pending_transaction_sequences
            .get(&id.0)
            .unwrap_or_else(|| env::panic_str("Transaction not found"));

        require!(
            transaction.created_by_id == env::predecessor_account_id(),
            "Unauthorized"
        );

        for signature_request in &transaction.signature_requests {
            require!(
                !signature_request.is_in_flight(),
                "Signature request is in-flight and cannot be removed",
            );
        }

        let ret = transaction
            .escrow
            .as_ref()
            .map(|escrow| {
                PromiseOrValue::Promise(
                    escrow
                        .asset_id
                        .transfer(transaction.created_by_id.clone(), escrow.amount),
                )
            })
            .unwrap_or(PromiseOrValue::Value(()));

        self.pending_transaction_sequences.remove(&id.0);

        ret
    }
}

fn decode_transaction_request(rlp_hex: &str) -> TransactionRequest {
    let rlp_bytes = hex::decode(rlp_hex)
        .unwrap_or_else(|_| env::panic_str("Error decoding `transaction_rlp` as hex"));
    let rlp = Rlp::new(&rlp_bytes);
    TransactionRequest::decode(&rlp).unwrap_or_else(|_| {
        env::panic_str("Error decoding `transaction_rlp` as transaction request RLP")
    })
}

register_custom_getrandom!(custom_getrandom);

pub fn custom_getrandom(buf: &mut [u8]) -> Result<(), Error> {
    buf.copy_from_slice(&env::random_seed_array());
    Ok(())
}
