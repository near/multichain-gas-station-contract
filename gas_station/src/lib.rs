use ethers_core::{
    types::{transaction::eip2718::TypedTransaction, Eip1559TransactionRequest, U256},
    utils::{
        hex,
        rlp::{Decodable, Rlp},
    },
};
use lib::{
    asset::{AssetBalance, AssetId},
    chain_key::ext_chain_key_token,
    foreign_address::ForeignAddress,
    oracle::{ext_oracle, PriceData},
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
use near_sdk_contract_tools::{owner::*, pause::*};
use near_sdk_contract_tools::{standard::nep297::Event, Owner, Pause};
use schemars::JsonSchema;

pub mod chain_configuration;
use chain_configuration::ChainConfiguration;

pub mod contract_event;
use contract_event::{ContractEvent, TransactionSequenceCreated, TransactionSequenceSigned};

mod impl_chain_key_nft;
pub use impl_chain_key_nft::Nep171ReceiverMsg;
#[cfg(feature = "debug")]
mod impl_debug;
mod impl_management;
mod impl_nep141_receiver;

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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(crate = "near_sdk::serde")]
pub struct Nep141ReceiverCreateTransactionArgs {
    pub token_id: String,
    pub transaction_rlp_hex: String,
    pub use_paymaster: Option<bool>,
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
pub struct TransactionSequenceCreation {
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
pub struct UserChainKey {
    pub public_key_bytes: Vec<u8>,
    pub authorization: ChainKeyAuthorization,
}

#[derive(
    Serialize,
    Deserialize,
    BorshSerialize,
    BorshDeserialize,
    JsonSchema,
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
)]
#[serde(crate = "near_sdk::serde")]
pub enum ChainKeyAuthorization {
    Owned,
    Approved(u32),
}

impl ChainKeyAuthorization {
    pub fn is_owned(&self) -> bool {
        matches!(self, Self::Owned)
    }

    pub fn is_approved(&self) -> bool {
        matches!(self, Self::Approved(..))
    }

    pub fn is_approved_with_id(&self, approval_id: u32) -> bool {
        self == &Self::Approved(approval_id)
    }

    pub fn to_approval_id(&self) -> Option<u32> {
        if let Self::Approved(approval_id) = self {
            Some(*approval_id)
        } else {
            None
        }
    }
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
    SupportedAssets,
    UserChainKeys,
    UserChainKeysFor(AccountId),
    PaymasterKeys,
}

// TODO: Cooldown timer/lock on nft keys before they can be returned to the user or used again in the gas station contract to avoid race condition
// TODO: Storage management
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault, Debug, Owner, Pause)]
#[near_bindgen]
pub struct Contract {
    pub next_unique_id: u64,
    pub signer_contract_id: AccountId,
    pub oracle_id: AccountId,
    pub supported_assets_oracle_asset_ids: UnorderedMap<AssetId, String>,
    pub flags: Flags,
    pub expire_sequence_after_blocks: u64,
    pub foreign_chains: UnorderedMap<u64, ChainConfiguration>,
    pub user_chain_keys: UnorderedMap<AccountId, UnorderedMap<String, UserChainKey>>,
    pub paymaster_keys: UnorderedMap<String, Vec<u8>>,
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
        supported_assets_oracle_asset_ids: Vec<(AssetId, String)>,
        expire_sequence_after_blocks: Option<U64>,
    ) -> Self {
        let mut contract = Self {
            next_unique_id: 0,
            signer_contract_id,
            oracle_id,
            supported_assets_oracle_asset_ids: UnorderedMap::new(StorageKey::SupportedAssets),
            flags: Flags::default(),
            expire_sequence_after_blocks: expire_sequence_after_blocks
                .map_or(DEFAULT_EXPIRE_SEQUENCE_AFTER_BLOCKS, u64::from),
            foreign_chains: UnorderedMap::new(StorageKey::ForeignChains),
            user_chain_keys: UnorderedMap::new(StorageKey::UserChainKeys),
            paymaster_keys: UnorderedMap::new(StorageKey::PaymasterKeys),
            sender_whitelist: UnorderedSet::new(StorageKey::SenderWhitelist),
            receiver_whitelist: UnorderedSet::new(StorageKey::ReceiverWhitelist),
            pending_transaction_sequences: UnorderedMap::new(
                StorageKey::PendingTransactionSequences,
            ),
            signed_transaction_sequences: Vector::new(StorageKey::SignedTransactionSequences),
            collected_fees: UnorderedMap::new(StorageKey::CollectedFees),
        };

        contract
            .supported_assets_oracle_asset_ids
            .extend(supported_assets_oracle_asset_ids);

        Owner::init(&mut contract, &env::predecessor_account_id());

        contract
    }

    // Public methods

    #[payable]
    pub fn create_transaction(
        &mut self,
        token_id: String,
        transaction_rlp_hex: String,
        use_paymaster: Option<bool>,
    ) -> PromiseOrValue<TransactionSequenceCreation> {
        self.create_transaction_inner(
            token_id,
            env::predecessor_account_id(),
            transaction_rlp_hex,
            use_paymaster,
            AssetBalance::native(env::attached_deposit()),
        )
    }

    fn create_transaction_inner(
        &mut self,
        token_id: String,
        account_id: AccountId,
        transaction_rlp_hex: String,
        use_paymaster: Option<bool>,
        deposit: AssetBalance,
    ) -> PromiseOrValue<TransactionSequenceCreation> {
        <Self as Pause>::require_unpaused();

        let transaction =
            ValidTransactionRequest::try_from(decode_transaction_request(&transaction_rlp_hex))
                .unwrap_or_reject();

        // Guarantees invariants required in callback
        self.filter_transaction(&account_id, &transaction);

        // Assert predecessor can use requested key path
        let user_keys = self
            .user_chain_keys
            .get(&account_id)
            .expect_or_reject("No managed keys for predecessor");
        require!(
            user_keys.contains_key(&token_id),
            "Predecessor unauthorized for the requested chain key token ID",
        );

        let use_paymaster = use_paymaster.unwrap_or(false);

        if use_paymaster {
            require!(deposit.amount.0 > 0, "Deposit is required to pay for gas");

            let supported_asset_oracle_asset_id = self
                .supported_assets_oracle_asset_ids
                .get(&deposit.asset_id)
                .expect_or_reject("Unsupported deposit asset");

            let chain_id = transaction.chain_id();
            let foreign_chain_configuration = self.get_chain(chain_id.as_u64()).unwrap_or_reject();

            ext_oracle::ext(self.oracle_id.clone())
                .get_price_data(Some(vec![
                    supported_asset_oracle_asset_id.clone(),
                    foreign_chain_configuration.oracle_asset_id.clone(),
                ]))
                .then(
                    Self::ext(env::current_account_id()).create_transaction_callback(
                        account_id,
                        token_id,
                        deposit,
                        supported_asset_oracle_asset_id.clone(),
                        transaction,
                    ),
                )
                .into()
        } else {
            let chain_id = transaction.chain_id;

            let authorization = user_keys.get(&token_id).unwrap_or_reject().authorization;

            let pending_transaction_sequence = PendingTransactionSequence {
                signature_requests: vec![SignatureRequest::new(
                    &token_id,
                    authorization,
                    transaction,
                    false,
                )],
                created_by_account_id: account_id,
                created_at_block_height: env::block_height().into(),
                escrow: None,
            };

            let creation = self.insert_transaction_sequence(pending_transaction_sequence.clone());

            ContractEvent::TransactionSequenceCreated(TransactionSequenceCreated {
                id: creation.id,
                foreign_chain_id: chain_id.to_string(),
                pending_transaction_sequence,
            })
            .emit();

            PromiseOrValue::Value(creation)
        }
    }

    #[private]
    pub fn create_transaction_callback(
        &mut self,
        #[serializer(borsh)] sender: AccountId,
        #[serializer(borsh)] token_id: String,
        #[serializer(borsh)] deposit: AssetBalance,
        #[serializer(borsh)] oracle_asset_id: String,
        #[serializer(borsh)] transaction_request: ValidTransactionRequest,
        #[callback_result] result: Result<PriceData, PromiseError>,
    ) -> TransactionSequenceCreation {
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
            &oracle_asset_id,
            &price_data,
            request_tokens_for_gas,
        );
        let deposit_amount = deposit.amount.0;

        match deposit_amount.checked_sub(fee) {
            None => {
                env::panic_str(&format!(
                    "Attached deposit ({deposit_amount}) is less than fee ({fee})"
                ));
            }
            Some(0) => {} // No refund; payment is exact.
            Some(refund) => {
                // Refund excess
                deposit.asset_id.transfer(sender.clone(), refund);
            }
        }

        let mut paymaster = foreign_chain_configuration
            .next_paymaster_notmut()
            .expect_or_reject("No paymasters found");

        let user_key = self
            .user_chain_keys
            .get(&sender)
            .expect_or_reject("No managed keys for sender")
            .get(&token_id)
            .expect_or_reject("Sender is unauthorized for the requested key path");

        let sender_foreign_address =
            ForeignAddress::from_raw_public_key(&user_key.public_key_bytes);

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

        foreign_chain_configuration
            .paymasters
            .insert(&paymaster.token_id, &paymaster);

        let signature_requests = vec![
            SignatureRequest::new(
                &paymaster.token_id,
                ChainKeyAuthorization::Owned,
                paymaster_transaction,
                true,
            ),
            SignatureRequest::new(
                &token_id,
                user_key.authorization,
                transaction_request.clone(),
                false,
            ),
        ];

        let pending_transaction_sequence = PendingTransactionSequence {
            signature_requests,
            created_by_account_id: sender,
            created_at_block_height: env::block_height().into(),
            escrow: Some(AssetBalance::native(fee)),
        };

        let creation = self.insert_transaction_sequence(pending_transaction_sequence.clone());

        ContractEvent::TransactionSequenceCreated(TransactionSequenceCreated {
            id: creation.id,
            foreign_chain_id: chain_id.to_string(),
            pending_transaction_sequence,
        })
        .emit();

        creation
    }

    pub fn sign_next(&mut self, id: U64) -> Promise {
        <Self as Pause>::require_unpaused();

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

        let payload = {
            let mut sighash = <TypedTransaction as From<ValidTransactionRequest>>::from(
                next_signature_request.transaction.clone(),
            )
            .sighash()
            .to_fixed_bytes();
            sighash.reverse();
            sighash
        };

        #[allow(clippy::cast_possible_truncation)]
        ext_chain_key_token::ext(self.signer_contract_id.clone()) // TODO: Gas.
            .ckt_sign_hash(
                next_signature_request.token_id.clone(),
                None,
                payload.to_vec(),
                next_signature_request.authorization.to_approval_id(),
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(near_sdk::Gas::ONE_TERA * 3)
                    .with_unused_gas_weight(0)
                    .sign_next_callback(id.into(), index as u32),
            )
    }

    #[private]
    pub fn sign_next_callback(
        &mut self,
        id: U64,
        index: u32,
        #[callback_result] result: Result<String, PromiseError>,
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
            .parse()
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
                id: id.into(),
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
        <Self as Pause>::require_unpaused();

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

    fn insert_transaction_sequence(
        &mut self,
        pending_transaction: PendingTransactionSequence,
    ) -> TransactionSequenceCreation {
        #[allow(clippy::cast_possible_truncation)]
        let pending_signature_count = pending_transaction.signature_requests.len() as u32;

        let id = self.generate_unique_id();

        self.pending_transaction_sequences
            .insert(id, pending_transaction);

        TransactionSequenceCreation {
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
