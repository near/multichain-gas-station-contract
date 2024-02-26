# Multichain Gas Station Contract

> This is still early software.

This smart contract accepts payments in NEAR tokens in exchange for gas funding on non-NEAR foreign chains. Part of the NEAR Multichain effort, it works in conjunction with the [MPC recovery service](https://github.com/near/mpc-recovery) to generate on-chain signatures.

## What is it?

This smart contract is a piece of the NEAR Multichain project, which makes NEAR Protocol an effortlessly cross-chain network. This contract accepts EVM transaction request payloads and facilitates the signing, gas funding, and relaying of the signed transactions to their destination chains. It works in conjunction with a few different services, including:

- The [MPC recovery service](https://github.com/near/mpc-recovery), also called the "MPC signer service", includes a network of trusted MPC signers, which hold keyshares and cooperatively sign transactions on behalf of the MPC network. It also includes an on-chain component, called the "MPC signer contract," which accepts on-chain signature requests and returns signatures computed by the MPC network.
- The [multichain relayer server](https://github.com/near/multichain-relayer-server) scans _this_ smart contract for signed transaction payloads and emits them to foreign chain RPCs.

## How does it work?

Currently, relaying one transaction to a foreign chain requires three transactions. However, [NEP-516 (delayed receipts / runtime triggers)](https://github.com/near/NEPs/issues/516) will reduce this number to one.

Transaction breakdown:

1. The first transaction is a call to the `create_transaction` function. This function accepts an EVM transaction request payload and a deposit amount (to pay for gas on the foreign chain) and returns an `id` and a `pending_transactions_count`.
2. The second transaction is a call to the `sign_next` function. This function accepts the `id` returned in step 1 and returns a signed payload. This payload is the gas funding transaction, transferring funds from a paymaster account on the foreign chain to the user's account on the foreign chain. It must be submitted to the foreign chain before the second signed payload.
3. The third transaction is another call to the `sign_next` function, identical to the one before. This function accepts an `id` and returns a signed payload. This payload is the signed user transaction.

Three transactions are required because of the gas restrictions imposed by the protocol. Currently (pre-NEP-516), the MPC signing function requires a _lot_ of gas, so dividing up the signing process into three parts is required to maximize the amount of gas available to each signing call.

Once this service and its supporting services are live, the multichain relayer server will be monitoring this gas station contract and relaying the signed transactions in the proper order as they become available, so it will not be strictly necessary for the users of this contract to ensure that the transactions are properly relayed, unless the user wishes to relay the transactions using their own RPC (e.g. to minimize latency).

### `sign_next` call trace explanation

Let's say `alice.near` has already called `create_transaction(..., use_paymaster=true)` on the gas station contract `gas-station.near`, and has obtained a transaction sequence `id` as a result of that function call.

Next, `alice.near` calls `gas-station.near::sign_next(id)`. Because this is the first `sign_next` call, the contract first generates a paymaster gas funding transaction. However, this is payload is unsigned at first. It is unwise[^unwise] to keep private keys on-chain (they would cease to be private), so the contract invokes another service, the MPC signing service.

[^unwise]: The debug/mock version of this contract _does_ store private keys on-chain (**big no-no**), making it _only suitable for testing_.

This service allows us to request signatures from a particular private key that the gas station contract controls. The MPC service allows the gas station contract to request a key by "path," which is simply a string. The signing service then uses a combination of the predecessor account ID (in this case, `gas-station.near`), the path string provided as a parameter to the signature request, and a few other pieces of information to derive a recoverable signature for the payload that recovers to a stable public key.

In the case of the paymaster transaction, the gas station uses a special set of predetermined path strings that map to known addresses on the foreign chain. These addresses are pre-funded with the native (gas) token for that foreign chain. Thus, when the gas station contract requests signatures for the paymaster transaction payload, the signed transactions are able to manipulate the funds in that foreign account.

In the case of the second, user-provided transaction, the gas station passes the user's account ID as the path string to the MPC signer service. This means that each transaction requested by `alice.near` will receive a signature that recovers to the same public key (foreign address) every time.

Therefore, the call trace for the two `sign_next` transactions looks something like this:

1. `alice.near` &rarr; `gas-station.near::sign_next(id) -> SignedTransaction`
   - `gas-station.near` &rarr; `mpc-signer.near::sign(payload=..., path=$0) -> Signature`
2. `alice.near` &rarr; `gas-station.near::sign_next(id) -> SignedTransaction`
   - `gas-station.near` &rarr; `mpc-signer.near::sign(payload=..., path=alice.near) -> Signature`

## Requirements

- Rust & Cargo
- [`cargo-make`](https://github.com/sagiegurari/cargo-make)
- [`near-cli-rs`](https://github.com/near/near-cli-rs)

## Build

```bash
cargo make build
```

The WASM binary will be generated in `target/wasm32-unknown-unknown/release/`.

The debug build can be generated with the command:

```bash
cargo make build-debug
```

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
