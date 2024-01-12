use ethers::{
    types::{transaction::eip2718::TypedTransaction, NameOrAddress},
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

mod signer_contract;
use signer_contract::ext_signer;

mod xchain_address;
use xchain_address::XChainAddress;

mod utils;
use utils::*;

type XChainTokenAmount = ethers::types::U256;

/// A successful request will emit two events, one for the request and one for
/// the finalized transaction, in that order. The `id` field will be the same
/// for both events.
///
/// IDs are arbitrarily chosen by the contract. An ID is guaranteed to be unique
/// within the contract.
#[event(version = "0.1.0", standard = "x-multichain-sig")]
pub enum ContractEvent {
    Request {
        xchain_id: String,
        sender: Option<XChainAddress>,
        unsigned_transaction: String,
        request_tokens_for_gas: Option<XChainTokenAmount>,
    },
    Finalize {
        xchain_id: String,
        sender: Option<XChainAddress>,
        signed_transaction: String,
        request_tokens_for_gas: Option<XChainTokenAmount>,
    },
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Debug, Default)]
#[serde(crate = "near_sdk::serde")]
pub struct Flags {
    is_deployment_allowed: bool,
    is_sender_whitelist_enabled: bool,
    is_receiver_whitelist_enabled: bool,
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
    pub xchain_id: String,
    /// The identifier that the xchain uses to identify itself.
    /// For example, 1 for ETH mainnet, 97 for BSC mainnet...
    pub chain_id: U64,
    pub signer_contract_id: AccountId,
    pub sender_whitelist: UnorderedSet<XChainAddress>,
    pub receiver_whitelist: UnorderedSet<XChainAddress>,
    pub flags: Flags,
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(xchain_id: String, chain_id: U64, signer_contract_id: AccountId) -> Self {
        let mut contract = Self {
            xchain_id,
            chain_id,
            signer_contract_id,
            sender_whitelist: UnorderedSet::new(StorageKey::SenderWhitelist),
            receiver_whitelist: UnorderedSet::new(StorageKey::ReceiverWhitelist),
            flags: Flags::default(),
        };

        Owner::init(&mut contract, &env::predecessor_account_id());

        contract
    }

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

    fn validate_transaction(&self, transaction: &mut TypedTransaction) {
        require!(
            transaction.gas().is_some(),
            "Gas must be explicitly specified",
        );

        if let Some(chain_id) = transaction.chain_id() {
            require!(chain_id.as_u64() == self.chain_id.0, "Chain ID mismatch");
        }

        transaction.set_chain_id(self.chain_id.0);

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
            // No receiver == contract deployment
            require!(
                self.flags.is_deployment_allowed,
                "Deployment is not allowed"
            );
        }

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

    fn xchain_relay_inner(&mut self, mut transaction: TypedTransaction) -> Promise {
        self.validate_transaction(&mut transaction);

        ContractEvent::Request {
            xchain_id: self.xchain_id.clone(),
            sender: transaction.from().map(Into::into),
            unsigned_transaction: hex::encode(&transaction.rlp()),
            request_tokens_for_gas: tokens_for_gas(&transaction),
        }
        .emit();

        let sighash_bytes = transaction.sighash().0;

        ext_signer::ext(self.signer_contract_id.clone()) // TODO: Gas.
            .sign(sighash_bytes, self.xchain_id.clone())
            .then(Self::ext(env::current_account_id()).xchain_relay_callback(transaction))
    }

    pub fn xchain_relay(
        &mut self,
        transaction_json: Option<TypedTransaction>,
        transaction_rlp: Option<String>,
    ) -> Promise {
        // TODO: Charge NEAR (or other FT) for gas fee.
        // This may require a price oracle as well?

        // Steps:
        // 1. Filter & validate payload.
        // 2. Request signature from MPC contract.
        // 3. Emit signature as event.

        let transaction = transaction_json
            .or_else(|| {
                transaction_rlp.map(|rlp_hex| {
                    let rlp_bytes = hex::decode(rlp_hex).unwrap_or_else(|_| {
                        env::panic_str("Error decoding `transaction_rlp` as hex")
                    });
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
            });

        self.xchain_relay_inner(transaction)
    }

    #[private]
    pub fn xchain_relay_callback(
        &mut self,
        transaction: TypedTransaction,
        #[callback_result] result: Result<ethers::types::Signature, PromiseError>,
    ) -> String {
        let signature = result
            .unwrap_or_else(|e| env::panic_str(&format!("Failed to produce signature: {e:?}")));

        let rlp_signed = transaction.rlp_signed(&signature);
        let rlp_signed_hex = hex::encode(&rlp_signed);

        let request_tokens_for_gas = tokens_for_gas(&transaction);

        ContractEvent::Finalize {
            xchain_id: self.xchain_id.clone(),
            sender: transaction.from().map(Into::into),
            signed_transaction: rlp_signed_hex,
            request_tokens_for_gas,
        }
        .emit();

        hex::encode(rlp_signed)
    }
}
