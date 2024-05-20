# Gas Station

This smart contract ties everything in this repository together. It uses NFT keys to produce MPC signatures for paymaster accounts on foreign chains, as well as facilitates the signing of user transactions.

## About

### Roles

There are two roles in this smart contract:

- **Administrator**s can manage role assignments, pause and unpause the contract, manage whitelists, flags, etc.
- **Market maker**s can update the paymaster account balances stored internally, synchronize paymaster account nonces, and withdraw collected fees.

### NFT chain keys

Users must `ckt_approve_call` their NFT chain keys to this contract before using. No `msg` is required.

#### Paymaster chain keys

An **Administrator** can add an NFT chain key as a paymaster when executing `ckt_approve_call` by specifying a `msg` with:

```json
{
    "is_paymaster": true
}
```

### Gas calculation

The gas station automatically sends the user's foreign account enough of the gas token to cover the gas limit of the user transaction. This means that if the transaction does not use the entire gas limit, there may be some "dust" left over.

## Build

Compiling the contract with the `debug` flag enabled will disable some checks (like permissioning functions to synchronize paymaster nonces) to make the contract easier to use on testnet. When compiling for mainnet, the `debug` flag must be disabled.
