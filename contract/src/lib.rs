use ethers_core::{
    types::{transaction::eip2718::TypedTransaction, Eip1559TransactionRequest, U256},
    utils::{
        hex::{self},
        rlp::{Decodable, Rlp},
    },
};
use lib::{
    foreign_address::ForeignAddress,
    kdf::get_mpc_address,
    oracle::{ext_oracle, PriceData},
    signer::{ext_signer, MpcSignature},
    Rejectable,
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env,
    json_types::{U128, U64},
    near_bindgen, require,
    serde::{Deserialize, Serialize},
    store::{UnorderedMap, UnorderedSet, Vector},
    AccountId, BorshStorageKey, PanicOnDefault, Promise, PromiseError, PromiseOrValue,
};
#[allow(clippy::wildcard_imports)]
use near_sdk_contract_tools::{owner::*, standard::nep297::Event, Owner};
use schemars::JsonSchema;

pub mod asset;
use asset::{AssetBalance, AssetId};

pub mod chain_configuration;
use chain_configuration::ChainConfiguration;

pub mod contract_event;
use contract_event::{ContractEvent, TransactionSequenceCreated, TransactionSequenceSigned};

#[cfg(feature = "debug")]
mod impl_debug;

mod impl_management;

pub mod valid_transaction_request;
use thiserror::Error;
use valid_transaction_request::ValidTransactionRequest;

pub mod signature_request;
use signature_request::{SignatureRequest, Status};

const DEFAULT_EXPIRE_SEQUENCE_AFTER_BLOCKS: u64 = 5 * 60; // 5ish minutes at 1s/block

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

#[derive(Serialize, JsonSchema)]
#[serde(crate = "near_sdk::serde")]
pub struct GetForeignChain {
    pub chain_id: U64,
    pub oracle_asset_id: String,
}

#[derive(
    Serialize,
    Deserialize,
    JsonSchema,
    BorshSerialize,
    BorshDeserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
)]
#[serde(crate = "near_sdk::serde")]
pub struct PendingTransactionSequence {
    pub created_by_account_id: AccountId,
    pub signature_requests: Vec<SignatureRequest>,
    pub created_at_block_height: U64,
    pub escrow: Option<AssetBalance>,
}

impl PendingTransactionSequence {
    pub fn all_signed(&self) -> bool {
        self.signature_requests
            .iter()
            .all(SignatureRequest::is_signed)
    }
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct TransactionSequenceSignedEventAt {
    pub block_height: u64,
    pub event: contract_event::TransactionSequenceSigned,
}

#[derive(BorshSerialize, BorshDeserialize, BorshStorageKey, Hash, Clone, Debug, PartialEq, Eq)]
pub enum StorageKey {
    SenderWhitelist,
    ReceiverWhitelist,
    ForeignChains,
    Paymasters(u64),
    PendingTransactionSequences,
    CollectedFees,
    SignedTransactionSequences,
}

// TODO: Pausability
// TODO: Storage management
// TODO: Ensure sufficient balance on foreign chain
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault, Debug, Owner)]
#[near_bindgen]
pub struct Contract {
    pub next_unique_id: u64,
    pub signer_contract_id: AccountId,
    pub signer_contract_public_key: Option<near_sdk::PublicKey>,
    pub oracle_id: AccountId,
    pub oracle_local_asset_id: String,
    pub flags: Flags,
    pub expire_sequence_after_blocks: u64,
    pub foreign_chains: UnorderedMap<u64, ChainConfiguration>,
    pub sender_whitelist: UnorderedSet<AccountId>,
    pub receiver_whitelist: UnorderedSet<ForeignAddress>,
    pub pending_transaction_sequences: UnorderedMap<u64, PendingTransactionSequence>,
    /// TODO: Hopefully temporary measure to eliminate the need for an indexer.
    pub signed_transaction_sequences: Vector<TransactionSequenceSignedEventAt>,
    pub collected_fees: UnorderedMap<AssetId, U128>,
}

#[allow(clippy::needless_pass_by_value)]
#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(
        signer_contract_id: AccountId,
        oracle_id: AccountId,
        oracle_local_asset_id: String,
        expire_sequence_after_blocks: Option<U64>,
    ) -> Self {
        let mut contract = Self {
            next_unique_id: 0,
            signer_contract_id,
            signer_contract_public_key: None, // Loaded asynchronously
            oracle_id,
            oracle_local_asset_id,
            flags: Flags::default(),
            expire_sequence_after_blocks: expire_sequence_after_blocks
                .map_or(DEFAULT_EXPIRE_SEQUENCE_AFTER_BLOCKS, u64::from),
            foreign_chains: UnorderedMap::new(StorageKey::ForeignChains),
            sender_whitelist: UnorderedSet::new(StorageKey::SenderWhitelist),
            receiver_whitelist: UnorderedSet::new(StorageKey::ReceiverWhitelist),
            pending_transaction_sequences: UnorderedMap::new(
                StorageKey::PendingTransactionSequences,
            ),
            signed_transaction_sequences: Vector::new(StorageKey::SignedTransactionSequences),
            collected_fees: UnorderedMap::new(StorageKey::CollectedFees),
        };

        Owner::init(&mut contract, &env::predecessor_account_id());

        contract
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
                .unwrap_or_reject();

        // Guarantees invariants required in callback
        self.filter_transaction(&env::predecessor_account_id(), &transaction);

        let use_paymaster = use_paymaster.unwrap_or(false);

        if use_paymaster {
            let chain_id = transaction.chain_id();
            let foreign_chain_configuration = self.get_chain(chain_id.as_u64()).unwrap_or_reject();

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
                signature_requests: vec![SignatureRequest::new(&predecessor, transaction, false)],
                created_by_account_id: predecessor,
                created_at_block_height: env::block_height().into(),
                escrow: None,
            };

            ContractEvent::TransactionSequenceCreated(TransactionSequenceCreated {
                foreign_chain_id: chain_id.to_string(),
                pending_transaction_sequence: pending_transaction_sequence.clone(),
            })
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
            .expect_or_reject(ChainConfigurationDoesNotExistError {
                chain_id: transaction_request.chain_id,
            });

        let price_data = result.ok().expect_or_reject("Failed to fetch price data");

        let paymaster_transaction_gas = foreign_chain_configuration.transfer_gas();
        let request_tokens_for_gas = (transaction_request.gas() + paymaster_transaction_gas)
            * transaction_request.max_fee_per_gas(); // Validation ensures gas is set.

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
            .expect_or_reject("No paymasters found");

        let sender_foreign_address = get_mpc_address(
            self.signer_contract_public_key.clone().expect_or_reject("The signer contract public key must be refreshed by calling `refresh_signer_public_key`"),
            &env::current_account_id(),
            sender.as_str(),
        )
        .expect_or_reject("Failed to calculate MPC address");

        let chain_id = transaction_request.chain_id;

        let paymaster_transaction = ValidTransactionRequest {
            chain_id,
            to: sender_foreign_address,
            value: request_tokens_for_gas.0,
            gas: paymaster_transaction_gas.0,
            data: vec![],
            nonce: U256::from(paymaster.next_nonce()).0,
            access_list_rlp: vec![0xc0 /* rlp encoding for empty list */],
            max_priority_fee_per_gas: transaction_request.max_priority_fee_per_gas,
            max_fee_per_gas: transaction_request.max_fee_per_gas,
        };

        paymaster.minimum_available_balance = U256(paymaster.minimum_available_balance)
            .checked_sub(request_tokens_for_gas)
            .expect_or_reject("Paymaster does not have enough funds")
            .0;

        let signature_requests = vec![
            SignatureRequest::new(&paymaster.key_path, paymaster_transaction, true),
            SignatureRequest::new(&sender, transaction_request.clone(), false),
        ];

        let pending_transaction_sequence = PendingTransactionSequence {
            signature_requests,
            created_by_account_id: sender,
            created_at_block_height: env::block_height().into(),
            escrow: Some(AssetBalance::native(fee)),
        };

        ContractEvent::TransactionSequenceCreated(TransactionSequenceCreated {
            foreign_chain_id: chain_id.to_string(),
            pending_transaction_sequence: pending_transaction_sequence.clone(),
        })
        .emit();

        self.insert_pending_transaction(pending_transaction_sequence)
    }

    pub fn sign_next(&mut self, id: U64) -> Promise {
        let id = id.0;

        let transaction = self
            .pending_transaction_sequences
            .get_mut(&id)
            .expect_or_reject(TransactionSequenceDoesNotExistError {
                transaction_sequence_id: id,
            });

        // ensure not expired
        require!(
            env::block_height()
                <= self.expire_sequence_after_blocks + transaction.created_at_block_height.0,
            "Transaction is expired",
        );

        // ensure only signed by original creator
        require!(
            transaction.created_by_account_id == env::predecessor_account_id(),
            "Predecessor must be the transaction creator",
        );

        let (index, next_signature_request) = transaction
            .signature_requests
            .iter_mut()
            .enumerate()
            .find(|(_, r)| r.is_pending())
            .expect_or_reject("No pending or non-in-flight signature requests");

        next_signature_request.status = Status::InFlight;

        #[allow(clippy::cast_possible_truncation)]
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
            .expect_or_reject(TransactionSequenceDoesNotExistError {
                transaction_sequence_id: id,
            });

        let request = pending_transaction_sequence
            .signature_requests
            .get_mut(index as usize)
            .expect_or_reject(SignatureRequestDoesNoteExistError {
                transaction_sequence_id: id,
                index,
            });

        if !request.is_in_flight() {
            env::panic_str(&format!(
                "Inconsistent state: Signature request {id}.{index} should be in-flight but is not"
            ));
        }

        // TODO: What to do if signing fails?
        // TODO: Refund the amount to the paymaster account?
        let signature = result
            .ok()
            .expect_or_reject("Failed to produce signature")
            .try_into()
            .unwrap_or_reject();

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
                if let Status::Signed { signature } = &r.status {
                    v.push((r.transaction.clone(), signature.clone()));
                    Some(v)
                } else {
                    None
                }
            });

        if let Some(all_signatures) = all_signatures {
            let e = TransactionSequenceSigned {
                foreign_chain_id: chain_id.to_string(),
                created_by_account_id: pending_transaction_sequence.created_by_account_id.clone(),
                signed_transactions: all_signatures
                    .into_iter()
                    .map(|(t, s)| {
                        hex::encode_prefixed(t.into_typed_transaction().rlp_signed(&s.into()))
                    })
                    .collect(),
            };

            self.signed_transaction_sequences
                .push(TransactionSequenceSignedEventAt {
                    block_height: env::block_height(),
                    event: e.clone(),
                });

            ContractEvent::TransactionSequenceSigned(e).emit();

            // Remove transaction if all requests have been signed
            // TODO: Is this over-eager?
            self.pending_transaction_sequences.remove(&id);
        }

        hex::encode_prefixed(&rlp_signed)
    }

    pub fn remove_transaction(&mut self, id: U64) -> PromiseOrValue<()> {
        let transaction = self
            .pending_transaction_sequences
            .get(&id.0)
            .expect_or_reject(TransactionSequenceDoesNotExistError {
                transaction_sequence_id: id.0,
            });

        require!(
            transaction.created_by_account_id == env::predecessor_account_id(),
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
            .map_or(PromiseOrValue::Value(()), |escrow| {
                PromiseOrValue::Promise(
                    escrow
                        .asset_id
                        .transfer(transaction.created_by_account_id.clone(), escrow.amount),
                )
            });

        self.pending_transaction_sequences.remove(&id.0);

        ret
    }
}

#[derive(Debug, Error)]
#[error("Configuration for chain ID {chain_id} does not exist")]
pub struct ChainConfigurationDoesNotExistError {
    pub chain_id: u64,
}

#[derive(Debug, Error)]
#[error("Transaction sequence with ID {transaction_sequence_id} does not exist")]
pub struct TransactionSequenceDoesNotExistError {
    pub transaction_sequence_id: u64,
}

#[derive(Debug, Error)]
#[error("Signature request {transaction_sequence_id}.{index} does not exist")]
pub struct SignatureRequestDoesNoteExistError {
    pub transaction_sequence_id: u64,
    pub index: u32,
}

impl Contract {
    fn get_chain(
        &self,
        chain_id: u64,
    ) -> Result<&ChainConfiguration, ChainConfigurationDoesNotExistError> {
        self.foreign_chains
            .get(&chain_id)
            .ok_or(ChainConfigurationDoesNotExistError { chain_id })
    }

    fn get_chain_mut(
        &mut self,
        chain_id: u64,
    ) -> Result<&mut ChainConfiguration, ChainConfigurationDoesNotExistError> {
        self.foreign_chains
            .get_mut(&chain_id)
            .ok_or(ChainConfigurationDoesNotExistError { chain_id })
    }

    fn generate_unique_id(&mut self) -> u64 {
        let id = self.next_unique_id;
        self.next_unique_id = self
            .next_unique_id
            .checked_add(1)
            .expect_or_reject("Failed to generate unique ID");
        id
    }

    fn filter_transaction(&self, sender_id: &AccountId, transaction: &ValidTransactionRequest) {
        // Check receiver whitelist
        if self.flags.is_receiver_whitelist_enabled {
            require!(
                self.receiver_whitelist.contains(&transaction.to),
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
        #[allow(clippy::cast_possible_truncation)]
        let pending_signature_count = pending_transaction.signature_requests.len() as u32;

        let id = self.generate_unique_id();

        self.pending_transaction_sequences
            .insert(id, pending_transaction);

        TransactionCreation {
            id: id.into(),
            pending_signature_count,
        }
    }
}

fn decode_transaction_request(rlp_hex: &str) -> Eip1559TransactionRequest {
    let rlp_bytes =
        hex::decode(rlp_hex).expect_or_reject("Error decoding `transaction_rlp` as hex");
    let rlp = Rlp::new(&rlp_bytes);
    Eip1559TransactionRequest::decode(&rlp)
        .expect_or_reject("Error decoding `transaction_rlp` as transaction request RLP")
}
