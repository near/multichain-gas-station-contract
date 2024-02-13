# Multichain Gas Station Contract

This smart contract accepts payments in NEAR tokens in exchange for gas funding on non-NEAR foreign chains. Part of the NEAR Multichain effort, it works in conjunction with the [MPC recovery service](https://github.com/near/mpc-recovery) to generate on-chain signatures.

## Requirements

- Rust & Cargo
- [`cargo-make`](https://github.com/sagiegurari/cargo-make)

## Build

```bash
cargo make build
```

The WASM binary will be generated in `target/wasm32-unknown-unknown/release/`.

## Contract Interactions

### Setup and Administration

1. Initialize the contract with a call to `new`. [The owner](https://github.com/near/near-sdk-contract-tools/blob/develop/src/owner.rs) is initialized as the predecessor of this transaction. All of the following transactions must be called by the owner.
2. Refresh the MPC contract public key by calling `refresh_signer_public_key`.
3. Set up foreign chain configurations with `add_foreign_chain`.
4. Add paymasters to each foreign chain with `add_paymaster`.

### Usage

Users who wish to get transactions signed and relayed by this contract and its accompanying infrastructure should perform the following steps:

1. Construct an unsigned transaction payload for the foreign chain they wish to interact with, e.g. Ethereum.
2. Call `create_transaction` on this contract, passing in that payload and activating the `use_paymaster` toggle in the case that the user wishes to use a paymaster. If the user uses a paymaster, he must attach a sufficient quantity of NEAR tokens to this transaction to pay for the gas + service fee. This function call returns an `id` and a `pending_transactions_count`.
3. Call `sign_next`, passing in the `id` value obtained in the previous step. This transaction should be executed with the maximum allowable quantity of gas (i.e. 300 TGas). This transaction will return a signed payload, part of the sequence of transactions necessary to send the user's transaction to the foreign chain. Repeat `pending_transactions_count` times.
4. Relay each signed payload to the foreign chain RPC in the order they were requested.

## Authors

- Jacob Lindahl <jacob.lindahl@near.org> [@sudo_build](https://twitter.com/sudo_build)
