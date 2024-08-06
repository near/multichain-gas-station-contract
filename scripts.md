# Scripts

## Initialization sequence

NOTE: ensure `network-config testnet` is set to appropriate value for all calls (either mainnet or testnet)

1. Deploy 
- ensure `canhazgas.testnet` field is updated to the appropriate values (name of your gas station contract)
  
   ```sh
   near contract deploy canhazgas.testnet use-file ./target/near/gas_station/gas_station.wasm with-init-call new_debug json-args '{"oracle_id":"pyth-oracle.testnet","signer_contract_id":"v2.nft.kagi.testnet"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' network-config testnet sign-with-legacy-keychain send
   ```

2. Add supported deposit assets 
- ensure the `hatchet.testnet`, `oracle_asset_id`, and `canhazgas.testnet` fields are updated to the appropriate values
  
   **Native NEAR**

   ```sh
   near contract call-function as-transaction canhazgas.testnet add_accepted_local_asset json-args '{"asset_id":"Native","decimals":24,"oracle_asset_id":"3gnSbT7bhoTdGkFVZc1dW1PvjreWzpUNUD5ppXwv1N59"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' sign-as hatchet.testnet network-config testnet sign-with-legacy-keychain send
   ```

3. Add foreign chain  
- ensure the `chain_id`, `oracle_asset_id`, and `canhazgas.testnet` fields are updated to the appropriate values
  
   ```sh
   near contract call-function as-transaction canhazgas.testnet add_foreign_chain json-args '{"chain_id":"97","oracle_asset_id":"EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw","transfer_gas":"21000","fee_rate":["120","100"],"decimals":18}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' sign-as canhazgas.testnet network-config testnet sign-with-legacy-keychain send
   ```

   **Note**: This script currently uses the ETH/USD price identifier despite the chain ID being that of BSC. This is because the Pyth price feed for BNB/USD on NEAR testnet is currently not working.

4. Add paymaster

   **Add administrator if necessary**
   - ensure the `hatchet.testnet`, and `canhazgas.testnet` fields are updated to the appropriate values
  
   ```sh
   near contract call-function as-transaction canhazgas.testnet add_administrator json-args '{"account_id":"hatchet.testnet"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' sign-as canhazgas.testnet network-config testnet sign-with-legacy-keychain send
   ```

   **Transfer chain key NFT - if NFT already minted**
     - NOTE: to mint NFT see [./nft_key#creating-new-key-tokens](https://github.com/near/multichain-gas-station-contract/tree/pyth-client/nft_key#creating-new-key-tokens)
     - ensure the `token_id`, `account_id`, and `hatchet.testnet` fields are updated to the appropriate values

   ```sh
   near contract call-function as-transaction v2.nft.kagi.testnet ckt_approve_call json-args '{"token_id":"1","account_id":"canhazgas.testnet","msg":"{\"is_paymaster\":true}"}' prepaid-gas '100.0 Tgas' attached-deposit '1 yoctoNEAR' sign-as hatchet.testnet network-config testnet sign-with-legacy-keychain send
   ```

   **Mark key for use as paymaster**
   - NOTE: call this once per `chain_id` you wish to add.
   - change the `balance` to the balance of the account on a chain at the time it is added as a paymaster for that chain. The balance must be specified in units of the smallest indivisible unit of gas token (i.e. wei on Ethereum mainnet). To find the account associated with the NFT from the previous step, call [nft_key.near->ckt_public_key_for(token_id)](https://github.com/near/multichain-gas-station-contract/blob/0ad3dd68d1f53129b482eaae865bca1a2daedbb8/nft_key/src/lib.rs#L168) and then convert it into an address using something like [ethers_core::utils::raw_public_key_to_address](https://docs.rs/ethers-core/latest/ethers_core/utils/fn.raw_public_key_to_address.html). For example: https://github.com/near/multichain-gas-station-contract/blob/0ad3dd68d1f53129b482eaae865bca1a2daedbb8/lib/src/foreign_address.rs#L20-L22
   - ensure the `chain_id`, `token_id`, and `canhazgas.testnet` fields are updated to the appropriate values

   ```sh
   near contract call-function as-transaction canhazgas.testnet add_paymaster json-args '{"chain_id":"97","balance":"100000000000000000000","nonce":0,"token_id":"1"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' sign-as canhazgas.testnet network-config testnet sign-with-legacy-keychain send
   ```

Selected [Pyth price identifiers](https://pyth.network/price-feeds?cluster=pythtest-crosschain):

- NEAR/USD: `3gnSbT7bhoTdGkFVZc1dW1PvjreWzpUNUD5ppXwv1N59`
- ETH/USD: `EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw`
- BNB/USD: `GwzBgrXb4PG59zjce24SF2b9JXbLEjJJTBkmytuEZj1b`

## Signing sequence

1. Create transaction sequence

   ```sh
   near contract call-function as-transaction canhazgas.testnet create_transaction json-args '{"transaction_rlp_hex":"0xe7618222628204d28204d2825208940f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f8204d280c0","use_paymaster":true,"token_id":"0"}' prepaid-gas '100.0 Tgas' attached-deposit '0.5 NEAR' sign-as hatchet.testnet network-config testnet sign-with-legacy-keychain send
   ```

2. Sign each transaction (perform 1+ times)

   ```sh
   near contract call-function as-transaction canhazgas.testnet sign_next json-args '{"id":"0"}' prepaid-gas '300.0 Tgas' attached-deposit '0 NEAR' sign-as hatchet.testnet network-config testnet sign-with-legacy-keychain send
   ```

## Maintenance

### Re-sync paymaster nonce

The nonce should be set to _the number of transactions that have already been sent **from** this account_. This means that the nonce should be set to `0` if the paymaster account has not yet sent any transactions, to `1` if the paymaster has already sent one transaction, etc.

```sh
near contract call-function as-transaction canhazgas.testnet set_paymaster_nonce json-args '{"chain_id":"97","index":0,"nonce":16}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' sign-as canhazgas.testnet network-config testnet sign-with-legacy-keychain send
```

### Set paymaster balance

This number will only decrease on the contract's side unless it is regularly "topped-up" by an authorized entity. Set this value to the maximum amount of foreign tokens the gas station contract can send to fund gas for user transactions.

```sh
near contract call-function as-transaction canhazgas.testnet set_paymaster_balance json-args '{"chain_id":"97","index":0,"balance":"134800000000000000"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' sign-as canhazgas.testnet network-config testnet sign-with-legacy-keychain send
```
