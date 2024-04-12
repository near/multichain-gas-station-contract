# Scripts

## Initialization sequence

1. Deploy

   ```sh
   near contract deploy canhazgas.testnet use-file ./target/wasm32-unknown-unknown/release/gas_station.wasm with-init-call new_debug json-args '{"oracle_id":"pyth-oracle.testnet","signer_contract_id":"multichain-testnet-2.testnet","supported_assets_oracle_asset_ids":[["Native","3gnSbT7bhoTdGkFVZc1dW1PvjreWzpUNUD5ppXwv1N59"]]}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' network-config testnet sign-with-legacy-keychain send
   ```

2. Add foreign chain

   ```sh
   near contract call-function as-transaction canhazgas.testnet add_foreign_chain json-args '{"chain_id":"97","oracle_asset_id":"GwzBgrXb4PG59zjce24SF2b9JXbLEjJJTBkmytuEZj1b","transfer_gas":"21000","fee_rate":["120","100"]}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' sign-as canhazgas.testnet network-config testnet sign-with-legacy-keychain send
   ```

3. Add paymaster

   ```sh
   near contract call-function as-transaction canhazgas.testnet add_paymaster json-args '{"chain_id":"97","balance":"100000000000000000000","nonce":0,"key_path":"$0"}' prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' sign-as canhazgas.testnet network-config testnet sign-with-legacy-keychain send
   ```

Selected [Pyth price identifiers](https://pyth.network/price-feeds?cluster=pythtest-crosschain):

- NEAR/USD: `3gnSbT7bhoTdGkFVZc1dW1PvjreWzpUNUD5ppXwv1N59`
- ETH/USD: `EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw`
- BNB/USD: `GwzBgrXb4PG59zjce24SF2b9JXbLEjJJTBkmytuEZj1b`

## Signing sequence

1. Create transaction sequence

   ```sh
   near contract call-function as-transaction canhazgas.testnet create_transaction json-args '{"transaction_rlp_hex":"0xe7618222bb8204d28204d2825208940f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f8204d280c0","use_paymaster":true}' prepaid-gas '100.0 Tgas' attached-deposit '0.5 NEAR' sign-as hatchet.testnet network-config testnet sign-with-legacy-keychain send
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
