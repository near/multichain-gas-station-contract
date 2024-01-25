use ethers::{
    types::{
        transaction::eip2718::TypedTransaction, NameOrAddress, TransactionRequest, H160, U256,
    },
    utils::rlp::{Decodable, Rlp},
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env,
    json_types::U64,
    near_bindgen, require,
    serde::{Deserialize, Serialize},
    store::{UnorderedMap, UnorderedSet, Vector},
    AccountId, BorshStorageKey, PanicOnDefault, Promise, PromiseError, PromiseOrValue,
};
use near_sdk_contract_tools::{event, owner::*, standard::nep297::Event, Owner};

mod oracle;
use oracle::{ext_oracle, process_oracle_result, PriceData};

mod signer_contract;
use signer_contract::{ext_signer, MpcSignature};

mod signature_request;
use signature_request::{SignatureRequest, SignatureRequestStatus};

mod utils;
use utils::*;

mod foreign_address;
use foreign_address::ForeignAddress;

type XChainTokenAmount = ethers::types::U256;

// TODO: Events
/// A successful request will emit two events, one for the request and one for
/// the finalized transaction, in that order. The `id` field will be the same
/// for both events.
///
/// IDs are arbitrarily chosen by the contract. An ID is guaranteed to be unique
/// within the contract.
// #[event(version = "0.1.0", standard = "x-multichain-sig")]
// pub enum ContractEvent {
//     RequestTransactionSignature {
//         xchain_id: String,
//         sender_address: Option<XChainAddress>,
//         unsigned_transaction: String,
//         request_tokens_for_gas: Option<XChainTokenAmount>,
//     },
//     FinalizeTransactionSignature {
//         xchain_id: String,
//         sender_address: Option<XChainAddress>,
//         signed_transaction: String,
//         signed_paymaster_transaction: String,
//         request_tokens_for_gas: Option<XChainTokenAmount>,
//     },
// }

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
pub struct TransactionInitiation {
    id: U64,
    pending_signature_count: u32,
}

#[derive(Serialize, BorshSerialize, BorshDeserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct PaymasterConfiguration {
    foreign_address: ForeignAddress,
    key_path: String,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct ForeignChainConfiguration {
    paymasters: Vector<PaymasterConfiguration>,
    next_paymaster: u32,
    oracle_asset_id: String,
}

impl ForeignChainConfiguration {
    pub fn next_paymaster(&mut self) -> Option<&PaymasterConfiguration> {
        let paymaster = self.paymasters.get(self.next_paymaster);
        self.next_paymaster = (self.next_paymaster + 1) % self.paymasters.len();
        paymaster
    }
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct GetForeignChain {
    chain_id: U64,
    oracle_asset_id: String,
}

#[derive(Serialize, Deserialize, BorshSerialize, BorshDeserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct PendingTransaction {
    sender_id: AccountId,
    signature_requests: Vec<SignatureRequest>,
}

impl PendingTransaction {
    pub fn all_signed(&self) -> bool {
        self.signature_requests.iter().all(|r| r.is_signed())
    }
}

#[derive(BorshSerialize, BorshDeserialize, BorshStorageKey, Hash, Clone, Debug, PartialEq, Eq)]
pub enum StorageKey {
    SenderWhitelist,
    ReceiverWhitelist,
    ForeignChains,
    Paymasters(u64),
    PendingTransactions,
}

#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault, Debug, Owner)]
#[near_bindgen]
pub struct Contract {
    pub next_unique_id: u64,
    pub signer_contract_id: AccountId,
    pub oracle_id: AccountId,
    pub oracle_local_asset_id: String,
    pub foreign_chains: UnorderedMap<u64, ForeignChainConfiguration>,
    pub sender_whitelist: UnorderedSet<ForeignAddress>,
    pub receiver_whitelist: UnorderedSet<ForeignAddress>,
    pub flags: Flags,
    pub price_scale: (u128, u128),
    pub pending_transactions: UnorderedMap<u64, PendingTransaction>,
}

fn transaction_fee(
    conversion_rate: (u128, u128),
    price_scale: (u128, u128),
    request_tokens_for_gas: XChainTokenAmount,
) -> u128 {
    // calculate fee based on currently known price, and include scaling factor
    let a = request_tokens_for_gas * U256::from(conversion_rate.0) * U256::from(price_scale.0);
    let (b, rem) = a.div_mod(U256::from(conversion_rate.1) * U256::from(price_scale.1));
    // round up
    if rem.is_zero() { b } else { b + 1 }.as_u128()
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(
        signer_contract_id: AccountId,
        oracle_id: AccountId,
        oracle_local_asset_id: String,
    ) -> Self {
        let mut contract = Self {
            next_unique_id: 0,
            signer_contract_id,
            oracle_id,
            oracle_local_asset_id,
            foreign_chains: UnorderedMap::new(StorageKey::ForeignChains),
            sender_whitelist: UnorderedSet::new(StorageKey::SenderWhitelist),
            receiver_whitelist: UnorderedSet::new(StorageKey::ReceiverWhitelist),
            flags: Flags::default(),
            price_scale: (120, 100), // +20% on top of oracle price
            pending_transactions: UnorderedMap::new(StorageKey::PendingTransactions),
        };

        Owner::init(&mut contract, &env::predecessor_account_id());

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

    pub fn get_sender_whitelist(&self) -> Vec<&ForeignAddress> {
        self.sender_whitelist.iter().collect()
    }

    pub fn add_to_sender_whitelist(&mut self, addresses: Vec<ForeignAddress>) {
        self.assert_owner();
        for address in addresses {
            self.sender_whitelist.insert(address);
        }
    }

    pub fn remove_from_sender_whitelist(&mut self, addresses: Vec<ForeignAddress>) {
        self.assert_owner();
        for address in addresses {
            self.sender_whitelist.remove(&address);
        }
    }

    pub fn clear_sender_whitelist(&mut self) {
        self.assert_owner();
        self.sender_whitelist.clear();
    }

    pub fn configure_foreign_chain(&mut self, chain_id: U64, oracle_asset_id: String) {
        self.assert_owner();
        let config =
            self.foreign_chains
                .entry(chain_id.0)
                .or_insert_with(|| ForeignChainConfiguration {
                    next_paymaster: 0,
                    oracle_asset_id: String::new(),
                    paymasters: Vector::new(StorageKey::Paymasters(chain_id.0)),
                });

        config.oracle_asset_id = oracle_asset_id;
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

    pub fn add_paymaster(
        &mut self,
        chain_id: U64,
        foreign_address: ForeignAddress,
        key_path: String,
    ) -> u32 {
        self.assert_owner();
        let chain = self
            .foreign_chains
            .get_mut(&chain_id.0)
            .unwrap_or_else(|| env::panic_str("Foreign chain does not exist"));

        let index = chain.paymasters.len();

        chain.paymasters.push(PaymasterConfiguration {
            foreign_address,
            key_path,
        });

        index
    }

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

    pub fn get_paymasters(&self, chain_id: U64) -> Vec<PaymasterConfiguration> {
        self.foreign_chains
            .get(&chain_id.0)
            .unwrap_or_else(|| env::panic_str("Foreign chain does not exist"))
            .paymasters
            .iter()
            .cloned()
            .collect()
    }

    pub fn get_transaction(&self, id: U64) -> Option<&PendingTransaction> {
        self.pending_transactions.get(&id.0)
    }

    // Private helper methods

    fn generate_unique_id(&mut self) -> u64 {
        let id = self.next_unique_id;
        self.next_unique_id = self.next_unique_id.checked_add(1).unwrap_or_else(|| {
            env::panic_str("Failed to generate unique ID");
        });
        id
    }

    fn validate_transaction(&self, transaction: &TypedTransaction) {
        require!(
            transaction.gas().is_some() && transaction.gas_price().is_some(),
            "Gas must be explicitly specified",
        );

        require!(
            transaction.chain_id().is_some(),
            "Chain ID must be explicitly specified",
        );

        // Validate receiver
        let receiver: Option<ForeignAddress> = match transaction.to() {
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

    fn insert_pending_transaction(
        &mut self,
        pending_transaction: PendingTransaction,
    ) -> TransactionInitiation {
        let pending_signature_count = pending_transaction.signature_requests.len() as u32;

        let id = self.generate_unique_id();

        self.pending_transactions.insert(id, pending_transaction);

        TransactionInitiation {
            id: id.into(),
            pending_signature_count,
        }
    }

    // Public methods

    #[payable]
    pub fn initiate_transaction(
        &mut self,
        transaction_json: Option<TypedTransaction>,
        transaction_rlp: Option<String>,
        use_paymaster: Option<bool>,
    ) -> PromiseOrValue<TransactionInitiation> {
        let deposit = env::attached_deposit();
        require!(deposit > 0, "Deposit is required to pay for gas");

        let transaction = extract_transaction(transaction_json, transaction_rlp);

        // Guarantees invariants required in callback
        self.validate_transaction(&transaction);

        let use_paymaster = use_paymaster.unwrap_or(false);

        if use_paymaster {
            let chain_id = transaction.chain_id().unwrap();
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
                    Self::ext(env::current_account_id()).initiate_transaction_callback(
                        env::predecessor_account_id(),
                        deposit.into(),
                        transaction,
                    ),
                )
                .into()
        } else {
            let predecessor = env::predecessor_account_id();

            PromiseOrValue::Value(self.insert_pending_transaction(PendingTransaction {
                signature_requests: vec![SignatureRequest::new(predecessor.clone(), transaction)],
                sender_id: predecessor,
            }))
        }
    }

    #[private]
    pub fn initiate_transaction_callback(
        &mut self,
        predecessor: AccountId,
        deposit: near_sdk::json_types::U128,
        transaction: TypedTransaction,
        #[callback_result] result: Result<PriceData, PromiseError>,
    ) -> TransactionInitiation {
        let foreign_chain_configuration = self
            .foreign_chains
            .get_mut(&transaction.chain_id().unwrap().as_u64())
            .unwrap_or_else(|| {
                env::panic_str(&format!(
                    "Paymaster not supported for chain id {}",
                    transaction.chain_id().unwrap()
                ))
            });

        let gas_token_price = process_oracle_result(
            &self.oracle_local_asset_id,
            &foreign_chain_configuration.oracle_asset_id,
            result,
        );
        let request_tokens_for_gas = tokens_for_gas(&transaction).unwrap(); // Validation ensures gas is set.
        let fee = transaction_fee(gas_token_price, self.price_scale, request_tokens_for_gas);
        // TODO: Ensure that deposit is returned if any recoverable errors are encountered.
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
                Promise::new(predecessor.clone()).transfer(refund);
            }
        }

        let paymaster = foreign_chain_configuration
            .next_paymaster()
            .unwrap_or_else(|| env::panic_str("No paymasters found"));

        let paymaster_transaction: TypedTransaction = TransactionRequest {
            chain_id: Some(transaction.chain_id().unwrap()),
            from: Some(paymaster.foreign_address.into()),
            to: Some((*transaction.from().unwrap()).into()),
            value: Some(request_tokens_for_gas),
            ..Default::default()
        }
        .into();

        let signature_requests = vec![
            SignatureRequest::new(&paymaster.key_path, paymaster_transaction),
            SignatureRequest::new(predecessor.clone(), transaction),
        ];

        self.insert_pending_transaction(PendingTransaction {
            signature_requests,
            sender_id: predecessor,
        })
    }

    pub fn sign_next(&mut self, id: U64) -> Promise {
        let id = id.0;

        let (index, next_signature_request, key_path) = self
            .pending_transactions
            .get_mut(&id)
            .unwrap_or_else(|| {
                env::panic_str(&format!("Transaction signature request {id} not found"))
            })
            .signature_requests
            .iter_mut()
            .enumerate()
            .filter_map(|(i, r)| match r.status {
                SignatureRequestStatus::Pending {
                    ref mut in_flight,
                    ref key_path,
                } if !*in_flight => {
                    *in_flight = true;
                    Some((i as u32, &r.transaction, key_path))
                }
                _ => None,
            })
            .next()
            .unwrap_or_else(|| env::panic_str("No pending or non-in-flight signature requests"));

        ext_signer::ext(self.signer_contract_id.clone()) // TODO: Gas.
            .sign(next_signature_request.0.sighash().0, key_path)
            .then(Self::ext(env::current_account_id()).sign_next_callback(id.into(), index))
    }

    #[private]
    pub fn sign_next_callback(
        &mut self,
        id: U64,
        index: u32,
        #[callback_result] result: Result<MpcSignature, PromiseError>,
    ) -> String {
        let id = id.0;

        let pending_transaction = self
            .pending_transactions
            .get_mut(&id)
            .unwrap_or_else(|| env::panic_str(&format!("Pending transaction {id} not found")));

        let request = pending_transaction
            .signature_requests
            .get_mut(index as usize)
            .unwrap_or_else(|| {
                env::panic_str(&format!(
                    "Signature request {id}.{index} not found in transaction",
                ))
            });

        if !request.is_pending() {
            env::panic_str(&format!(
                "Signature request {id}.{index} has already been signed"
            ));
        }

        let signature = result
            .unwrap_or_else(|e| env::panic_str(&format!("Failed to produce signature: {e:?}")))
            .try_into()
            .unwrap_or_else(|e| env::panic_str(&format!("Failed to decode signature: {e:?}")));

        let transaction = &request.transaction.0;

        let rlp_signed = transaction.rlp_signed(&signature);

        request.set_signature(signature);

        // Remove transaction if all requests have been signed
        if pending_transaction.all_signed() {
            self.pending_transactions.remove(&id);
        }

        hex::encode(&rlp_signed)
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
