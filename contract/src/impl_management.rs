use std::str::FromStr;

use ethers_core::types::U256;
use near_sdk::{
    env,
    json_types::{U128, U64},
    near_bindgen, require,
    store::Vector,
    AccountId, Promise, PromiseError, PromiseOrValue,
};
use near_sdk_contract_tools::owner::{Owner, OwnerExternal};

use crate::{
    asset::AssetId,
    chain_configuration::{ChainConfiguration, PaymasterConfiguration, ViewPaymasterConfiguration},
    contract_event::TransactionSequenceSigned,
    decode_transaction_request,
    valid_transaction_request::ValidTransactionRequest,
    Contract, ContractExt, Flags, GetForeignChain, PendingTransactionSequence, StorageKey,
};
use lib::{
    foreign_address::ForeignAddress, kdf::get_mpc_address, oracle::PriceData, signer::ext_signer,
    Rejectable,
};

#[allow(clippy::needless_pass_by_value)]
#[near_bindgen]
impl Contract {
    pub fn get_expire_sequence_after_blocks(&self) -> U64 {
        self.expire_sequence_after_blocks.into()
    }

    pub fn set_expire_sequence_after_blocks(&mut self, expire_sequence_after_blocks: U64) {
        self.assert_owner();
        self.expire_sequence_after_blocks = expire_sequence_after_blocks.into();
    }

    pub fn get_signer_contract_id(&self) -> &AccountId {
        &self.signer_contract_id
    }

    /// Set the signer contract ID. Automatically refreshes the public key
    /// unless `refresh` is `false`, in which case it requires a call to
    /// [`Contract::refresh_signer_public_key`] afterwards.
    pub fn set_signer_contract_id(
        &mut self,
        account_id: AccountId,
        refresh: Option<bool>,
    ) -> PromiseOrValue<()> {
        self.assert_owner();

        if self.signer_contract_id != account_id {
            self.signer_contract_id = account_id;
            self.signer_contract_public_key = None;

            if refresh.unwrap_or(true) {
                return PromiseOrValue::Promise(
                    ext_signer::ext(self.signer_contract_id.clone())
                        .public_key()
                        .then(
                            Self::ext(env::current_account_id())
                                .refresh_signer_public_key_callback(),
                        ),
                );
            }
        }

        PromiseOrValue::Value(())
    }

    /// Refresh the public key from the signer contract.
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
        let public_key = public_key
            .ok()
            .expect_or_reject("Failed to load signer public key from the signer contract");
        self.signer_contract_public_key = Some(public_key);
    }

    pub fn get_signer_public_key(&self) -> Option<&near_sdk::PublicKey> {
        self.signer_contract_public_key.as_ref()
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
            ChainConfiguration {
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

        let config = self.get_chain_mut(chain_id.0).unwrap_or_reject();
        config.oracle_asset_id = oracle_asset_id;
    }

    pub fn set_foreign_chain_transfer_gas(&mut self, chain_id: U64, transfer_gas: U128) {
        self.assert_owner();

        let config = self.get_chain_mut(chain_id.0).unwrap_or_reject();
        config.transfer_gas = U256::from(transfer_gas.0).0;
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
        nonce: u32,
        key_path: String,
        balance: Option<near_sdk::json_types::U128>,
    ) -> u32 {
        self.assert_owner();

        require!(
            AccountId::from_str(&key_path).is_err(),
            "Paymaster key path must not be a valid account id",
        );

        let chain = self.get_chain_mut(chain_id.0).unwrap_or_reject();

        let index = chain.paymasters.len();

        chain.paymasters.push(PaymasterConfiguration {
            nonce,
            key_path,
            minimum_available_balance: U256::from(balance.map_or(0, |v| v.0)).0,
        });

        index
    }

    pub fn set_paymaster_balance(&mut self, chain_id: U64, index: u32, balance: U128) {
        #[cfg(not(feature = "debug"))]
        self.assert_owner();

        let chain = self.get_chain_mut(chain_id.0).unwrap_or_reject();
        let paymaster = chain.get_paymaster_mut(index).unwrap_or_reject();

        paymaster.minimum_available_balance = U256::from(balance.0).0;
    }

    pub fn increase_paymaster_balance(&mut self, chain_id: U64, index: u32, balance: U128) {
        #[cfg(not(feature = "debug"))]
        self.assert_owner();

        let chain = self.get_chain_mut(chain_id.0).unwrap_or_reject();
        let paymaster = chain.get_paymaster_mut(index).unwrap_or_reject();

        paymaster.minimum_available_balance =
            (U256(paymaster.minimum_available_balance) + U256::from(balance.0)).0;
    }

    pub fn set_paymaster_nonce(&mut self, chain_id: U64, index: u32, nonce: u32) {
        #[cfg(not(feature = "debug"))]
        self.assert_owner();

        let chain = self.get_chain_mut(chain_id.0).unwrap_or_reject();
        let paymaster = chain.get_paymaster_mut(index).unwrap_or_reject();

        paymaster.nonce = nonce;
    }

    /// Note: If a transaction sequence is _already_ pending signatures with
    /// the paymaster getting removed, this method will not prevent those
    /// payloads from getting signed.
    pub fn remove_paymaster(&mut self, chain_id: U64, index: u32) {
        self.assert_owner();
        let chain = self.get_chain_mut(chain_id.0).unwrap_or_reject();

        if index < chain.paymasters.len() {
            chain.paymasters.swap_remove(index);
            // resetting chain.next_paymaster is not necessary, since overflow is handled in [`ForeignChainConfiguration::next_paymaster`] function.
        } else {
            env::panic_str("Invalid index");
        }
    }

    pub fn get_paymasters(&self, chain_id: U64) -> Vec<ViewPaymasterConfiguration> {
        self.get_chain(chain_id.0)
            .unwrap_or_reject()
            .paymasters
            .iter()
            .map(|p| ViewPaymasterConfiguration {
                nonce: p.nonce,
                key_path: p.key_path.clone(),
                foreign_address: get_mpc_address(
                    self.signer_contract_public_key.clone().unwrap(),
                    &env::current_account_id(),
                    &p.key_path,
                )
                .unwrap(),
                minimum_available_balance: U256(p.minimum_available_balance).as_u128().into(),
            })
            .collect()
    }

    pub fn list_pending_transaction_sequences(
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

    pub fn get_pending_transaction_sequence(&self, id: U64) -> Option<&PendingTransactionSequence> {
        self.pending_transaction_sequences.get(&id.0)
    }

    pub fn list_signed_transaction_sequences_after(
        &self,
        block_height: U64,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Vec<&TransactionSequenceSigned> {
        self.signed_transaction_sequences
            .iter()
            .skip_while(|s| s.block_height < block_height.0)
            .skip(offset.map_or(0, |o| o as usize))
            .take(limit.map_or(usize::MAX, |l| l as usize))
            .map(|s| &s.event)
            .collect()
    }

    pub fn withdraw_collected_fees(
        &mut self,
        asset_id: AssetId,
        amount: Option<U128>,
        receiver_id: Option<AccountId>, // TODO: Pull method instead of push (danger of typos/locked accounts)
    ) -> Promise {
        near_sdk::assert_one_yocto();
        self.assert_owner();
        let fees = self
            .collected_fees
            .get_mut(&asset_id)
            .expect_or_reject("No fee entry for provided asset ID");

        let amount = amount.unwrap_or(U128(fees.0));

        fees.0 = fees
            .0
            .checked_sub(amount.0)
            .expect_or_reject("Not enough fees to withdraw");

        asset_id.transfer(
            receiver_id.unwrap_or_else(|| self.own_get_owner().unwrap()),
            amount,
        )
    }

    pub fn get_collected_fees(&self) -> std::collections::HashMap<&AssetId, &U128> {
        self.collected_fees.iter().collect()
    }

    pub fn get_foreign_address_for(&self, account_id: AccountId) -> ForeignAddress {
        get_mpc_address(
            self.signer_contract_public_key.clone().unwrap(),
            &env::current_account_id(),
            account_id.as_str(),
        )
        .unwrap()
    }

    pub fn estimate_gas_cost(&self, transaction_rlp_hex: String, price_data: PriceData) -> U128 {
        let transaction =
            ValidTransactionRequest::try_from(decode_transaction_request(&transaction_rlp_hex))
                .expect_or_reject("Invalid transaction request");

        let foreign_chain_configuration = self.get_chain(transaction.chain_id).unwrap_or_reject();

        let paymaster_transaction_gas = foreign_chain_configuration.transfer_gas();
        let request_tokens_for_gas =
            (transaction.gas() + paymaster_transaction_gas) * transaction.max_fee_per_gas();

        foreign_chain_configuration
            .foreign_token_price(
                &self.oracle_local_asset_id,
                &price_data,
                request_tokens_for_gas,
            )
            .into()
    }
}
