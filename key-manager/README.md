# Chain Key Governor

The goal of the Chain Key Governor standard is to separate the control of a chain key (chain signatures, MPC signing) from the smart contract that is using the key.

The standard defines two contract interfaces: a key manager interface and a governor interface.

All of the methods are prefixed with `ck_` for "**c**hain **k**ey."

## Example workflow

`alice.near` wishes to use `gas-station.near`, a smart contract that uses MPC keys to fund gas for transactions on foreign blockchains. `alice.near` will use the Chain Key Governor (Manager)-compliant contract `key-manager.near` to delegate an MPC key owned by `alice.near` to `gas-station.near`.

1. `alice.near` calls `key-manager.near::ck_transfer_governorship(path=eth, new_governor_id=gas-station.near)`. Note that there is no "create key" call needed: all unassigned key paths are implicitly governed by the owner.
    1. As part of this function call, `key-manager.near` calls `gas-station.near::ck_accept_governorship(owner_id=alice.near, path=eth)`.
    2. `gas-station.near` decides to accept the governorship, and so returns `true`.
    3. Receiving the `true` response from `gas-station.near`, `key-manager.near` assigns `gas-station.near` as the governor of `alice.near`'s key `"eth"`.
2. `alice.near` now wishes to use the gas funding functionality of `gas-station.near`, so she calls `gas-station.near::fund_foreign_transaction()` (example method, not part of this standard).
    1. `gas-station.near` makes the cross-contract call to `key-manager.near::ck_sign_prehashed(owner_id=alice.near, path=eth, payload=[...])`.
    2. `key-manager.near` sees that `gas-station.near` holds the governorship for the key `"eth"` owned by `alice.near`, so it initiates the signing process and returns the signature.
    3. `gas-station.near` can use the returned signature to fulfill `alice.near`'s request.
3. `alice.near` wishes to reclaim control of her `"eth"` key. She calls `gas-station.near::request_release_key(path=eth)` (example method, not part of this standard).
    1. `gas-station.near` calls `key-manager.near::ck_transfer_governorship(owner_id=alice.near, path=eth, new_governor_id=None)`.
    2. `key-manager.near` sees that `gas-station.near` holds the governorship for the key `"eth"` owned by `alice.near`, so it removes `gas-station.near` from the governorship of the specified key.
    3. `alice.near` can once again independently initiate signatures for her `"eth"` key.

## Notes and concerns

- If a key is assigned to a governor, the governor is the _only_ entity that is allowed to initiate signatures for that key.
- It may be desirable to remove the concept of
