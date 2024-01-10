use ethers::{
    types::{transaction::eip2718::TypedTransaction, NameOrAddress, U256},
    utils::to_checksum,
};
use near_sdk::{
    borsh::{BorshDeserialize, BorshSerialize},
    collections::LazyOption,
    env,
    json_types::{Base64VecU8, U64},
    near_bindgen, require,
    serde::{Deserialize, Serialize},
    store::UnorderedSet,
    AccountId, BorshStorageKey, PanicOnDefault, Promise, PromiseError,
};
use near_sdk_contract_tools::{event, standard::nep297::Event};

use crate::signer_contract::ext_signer;

mod signer_contract;
mod xchain_address;
use xchain_address::XChainAddress;

#[event(version = "0.1.0", standard = "x-multichain-sig")]
pub enum ContractEvent {
    Relay {
        xchain_id: String,
        sender: Option<String>,
        payload_id: Option<String>,
        payload: String,
        request_tokens_for_gas: Option<U256>,
    },
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Debug)]
#[borsh(crate = "near_sdk::borsh")]
#[serde(crate = "near_sdk::serde")]
pub struct Flags {
    allow_deployment: bool,
}

#[derive(BorshSerialize, BorshDeserialize, BorshStorageKey, Hash, Clone, Debug, PartialEq, Eq)]
#[borsh(crate = "near_sdk::borsh")]
pub enum StorageKey {
    SenderWhitelist,
    ReceiverWhitelist,
    Flags,
}

#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault, Debug)]
#[borsh(crate = "near_sdk::borsh")]
#[near_bindgen]
pub struct Contract {
    pub xchain_id: String,
    pub chain_id: U64,
    pub signer_contract_id: AccountId,
    pub sender_whitelist: Option<UnorderedSet<XChainAddress>>,
    pub receiver_whitelist: Option<UnorderedSet<XChainAddress>>,
    pub flags: LazyOption<Flags>,
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(xchain_id: String, chain_id: U64, signer_contract_id: AccountId) -> Self {
        Self {
            xchain_id,
            chain_id,
            signer_contract_id,
            sender_whitelist: None,
            receiver_whitelist: None,
            flags: LazyOption::new(StorageKey::Flags, None),
        }
    }

    pub fn xchain_relay(
        &mut self,
        #[allow(unused_mut)] mut transaction: TypedTransaction,
    ) -> Promise {
        // Steps:
        // 1. Filter & validate payload.
        // 2. Request signature from MPC contract.
        // 3. Emit signature as event.

        require!(
            transaction.gas().is_some(),
            "Gas must be explicitly specified",
        );

        if let Some(chain_id) = transaction.chain_id() {
            require!(chain_id.as_u64() == self.chain_id.0, "Chain ID mismatch");
        }

        transaction.set_chain_id(self.chain_id.0);

        let receiver: Option<XChainAddress> = match transaction.to() {
            Some(NameOrAddress::Name(_)) => {
                env::panic_str("ENS names are not supported");
            }
            Some(NameOrAddress::Address(address)) => Some(address.into()),
            None => None,
        };

        if let Some(ref sender_whitelist) = self.sender_whitelist {
            require!(
                sender_whitelist.contains(
                    &transaction
                        .from()
                        .unwrap_or_else(|| env::panic_str("Sender whitelist is enabled"))
                        .into()
                ),
                "Sender is not whitelisted",
            );
        }

        let flags = self.flags.get();

        match receiver {
            Some(ref receiver) => {
                if let Some(ref receiver_whitelist) = self.receiver_whitelist {
                    require!(
                        receiver_whitelist.contains(receiver),
                        "Receiver is not whitelisted",
                    );
                }
            }
            None => {
                if let Some(ref flags) = flags {
                    require!(flags.allow_deployment, "Deployment is not allowed");
                }
            }
        }

        let sighash_bytes = transaction.sighash().0;

        ext_signer::ext(self.signer_contract_id.clone())
            .sign(sighash_bytes, self.xchain_id.clone())
            .then(Self::ext(env::current_account_id()).xchain_relay_callback(transaction))
    }

    #[private]
    pub fn xchain_relay_callback(
        &mut self,
        transaction: TypedTransaction,
        #[callback_result] result: Result<ethers::types::Signature, PromiseError>,
    ) -> Base64VecU8 {
        let signature = result
            .unwrap_or_else(|e| env::panic_str(&format!("Failed to produce signature: {e:?}")));

        let txid = transaction.hash(&signature);
        let rlp_signed = transaction.rlp_signed(&signature);
        let rlp_signed_hex = hex::encode(&rlp_signed);

        let request_tokens_for_gas = transaction
            .gas()
            .zip(transaction.gas_price())
            .map(|(gas, gas_price)| gas * gas_price);

        ContractEvent::Relay {
            xchain_id: self.xchain_id.clone(),
            sender: transaction.from().map(|a| to_checksum(a, None)),
            payload_id: Some(hex::encode(txid.0)),
            payload: rlp_signed_hex,
            request_tokens_for_gas,
        }
        .emit();

        rlp_signed.to_vec().into()
    }
}
