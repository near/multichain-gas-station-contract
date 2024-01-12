use ethers::{
    types::{transaction::eip2718::TypedTransaction, NameOrAddress, U256},
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
    pub xchain_chain_id: U64,
    pub signer_contract_id: AccountId,
    pub sender_whitelist: UnorderedSet<XChainAddress>,
    pub receiver_whitelist: UnorderedSet<XChainAddress>,
    pub flags: Flags,
    pub price_per_xchain_gas_token: (u128, u128),
    pub price_scale: (u128, u128),
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(xchain_id: String, xchain_chain_id: U64, signer_contract_id: AccountId) -> Self {
        let mut contract = Self {
            xchain_id,
            xchain_chain_id,
            signer_contract_id,
            sender_whitelist: UnorderedSet::new(StorageKey::SenderWhitelist),
            receiver_whitelist: UnorderedSet::new(StorageKey::ReceiverWhitelist),
            flags: Flags::default(),
            // this value is approximately the number of yoctoNEAR per 1wei of ETH.
            price_per_xchain_gas_token: (10u128.pow(24) * 2500, 10u128.pow(18) * 350 / 100), // TODO: Make dynamic / configurable
            price_scale: (120, 100), // +20% on top of market price
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
            transaction.gas().is_some() && transaction.gas_price().is_some(),
            "Gas must be explicitly specified",
        );

        if let Some(chain_id) = transaction.chain_id() {
            require!(
                chain_id.as_u64() == self.xchain_chain_id.0,
                "Chain ID mismatch"
            );
        }

        transaction.set_chain_id(self.xchain_chain_id.0);

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

    fn xchain_relay_inner(&mut self, transaction: TypedTransaction) -> Promise {
        ext_signer::ext(self.signer_contract_id.clone()) // TODO: Gas.
            .sign(transaction.sighash().0, self.xchain_id.clone())
            .then(Self::ext(env::current_account_id()).xchain_relay_callback(transaction))
    }

    fn price_of_gas(&self, request_tokens_for_gas: XChainTokenAmount) -> u128 {
        // calculate fee based on currently known price, and include scaling factor
        let a = request_tokens_for_gas
            * U256::from(self.price_per_xchain_gas_token.0)
            * U256::from(self.price_scale.0);
        let (b, rem) = a.div_mod(
            U256::from(self.price_per_xchain_gas_token.1) * U256::from(self.price_scale.1),
        );
        // round up
        if !rem.is_zero() { b + 1 } else { b }.as_u128()
    }

    #[payable]
    pub fn xchain_relay(
        &mut self,
        transaction_json: Option<TypedTransaction>,
        transaction_rlp: Option<String>,
    ) -> Promise {
        // Steps:
        // 1. Filter & validate payload.
        // 2. Request signature from MPC contract.
        // 3. Emit signature as event.

        let mut transaction = extract_transaction(transaction_json, transaction_rlp);

        self.validate_transaction(&mut transaction);

        let request_tokens_for_gas = tokens_for_gas(&transaction).unwrap(); // Validation ensures gas is set.

        let fee = self.price_of_gas(request_tokens_for_gas);

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
            xchain_id: self.xchain_id.clone(),
            sender_address: transaction.from().map(Into::into),
            unsigned_transaction: hex::encode(&transaction.rlp()),
            request_tokens_for_gas: Some(request_tokens_for_gas),
        }
        .emit();

        self.xchain_relay_inner(transaction)
    }

    #[private]
    pub fn xchain_relay_callback(
        &mut self,
        transaction: TypedTransaction,
        #[callback_result] result: Result<ethers::types::Signature, PromiseError>, // NOTE: The exact format of the result is uncertain, but it should contain the same information regardless.
    ) -> String {
        let signature = result
            .unwrap_or_else(|e| env::panic_str(&format!("Failed to produce signature: {e:?}")));

        let rlp_signed = transaction.rlp_signed(&signature);
        let rlp_signed_hex = hex::encode(&rlp_signed);

        let request_tokens_for_gas = tokens_for_gas(&transaction);

        ContractEvent::FinalizeTransactionSignature {
            xchain_id: self.xchain_id.clone(),
            sender_address: transaction.from().map(Into::into),
            signed_transaction: rlp_signed_hex,
            request_tokens_for_gas,
        }
        .emit();

        hex::encode(rlp_signed)
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
