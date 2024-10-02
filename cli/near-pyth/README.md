# NEAR-Pyth Price Pusher

This is a simple CLI program for interacting with the Pyth price API and pushing selected prices to the oracles on the NEAR blockchain.

## Usage

Note that wherever a price ID is required, it can be substituted for a query (e.g. "btc/usd") that resolves to the desired ID. Otherwise, IDs can be specified in either base58 or hex. The application runs the resolver on all IDs before executing the specified operation.

Since the application of this program is quite limited in scope, it is able to make many intelligent assumptions. For example, merely specifying the desired NEAR network via the `-n/--network` flag or the `NEAR_ENV` environment variable is usually sufficient information for the program to guess the correct Pyth HTTP endpoint, contract ID, etc. Of course, all of these defaults can be easily overridden with CLI flags.

### Sample executions

#### Get prices from the testnet Pyth HTTP endpoint

```sh
near-pyth http-get near/usd usdc/usd usdt/usd
```

Sample output:

```text
Feed ID: 27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4
4.95 ± 0.00 @ 2024-07-04T14:32:15+09:00 (now)

Feed ID: 1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588
1.00 ± 0.00 @ 2024-07-04T14:32:15+09:00 (now)

Feed ID: 41f3625971ca2ed2263e78573fe5ce23e13d2558ed3f2e47ab0f84fb9e7ae722
1.00 ± 0.00 @ 2024-07-04T14:32:13+09:00 (now)
```

#### Get prices from the mainnet Pyth HTTP endpoint

```sh
NEAR_ENV=mainnet near-pyth http-get near/usd usdc/usd usdt/usd
```

OR

```sh
near-pyth -n mainnet http-get near/usd usdc/usd usdt/usd
```

OR

```sh
near-pyth -e https://hermes.pyth.network http-get near/usd usdc/usd usdt/usd
```

Sample output:

```text
Feed ID: c415de8d2eba7db216527dff4b60e8f3a5311c740dadb233e13e12547e226750
4.95 ± 0.01 @ 2024-07-04T14:32:40+09:00 (now)

Feed ID: 2b89b9dc8fdf9f34709a5b106b472f0f39bb6ca9ce04b0fd7f2e971688e2e53b
1.00 ± 0.00 @ 2024-07-04T14:32:40+09:00 (now)

Feed ID: eaa020c61cc479712813461ce153894a96a6c00b21ed0cfc2798d1f9a9e9c94a
1.00 ± 0.00 @ 2024-07-04T14:32:40+09:00 (now)
```

#### Push a single price update

This program reads key files from the legacy NEAR CLI. To be precise, it reads a JSON file that contains the keys `accountId` and `privateKey`.

```sh
near-pyth update near/usd --key-file ~/.near-credentials/testnet/<account>.json
```

Sample output:

```text
Acting account: <account>
TXID: 2JfB9i11qa7B6H2T8kKDm6371gZufgRy8vx5ohUiHG45
```

#### Continuously push price updates

```sh
near-pyth stream-update dot/usd sui/usd -k ~/.near-credentials/testnet/<account>.json
```

Sample output:

```text
Acting account: <account>
36032e522b810babd8e3148e9f0d588af9e95e93b97ffb58566b837fdbd31f7f: 5.92 ± 0.00 @ 2024-07-04T15:16:24+09:00 (now)
50c67b3fd225db8912a424dd4baed60ffdde625ed2feaaf283724f9608fea266: 0.75 ± 0.00 @ 2024-07-04T15:16:24+09:00 (now)
Skipping 0, pushing newest data only
36032e522b810babd8e3148e9f0d588af9e95e93b97ffb58566b837fdbd31f7f: 5.92 ± 0.00 @ 2024-07-04T15:16:24+09:00 (now)
50c67b3fd225db8912a424dd4baed60ffdde625ed2feaaf283724f9608fea266: 0.75 ± 0.00 @ 2024-07-04T15:16:24+09:00 (now)
36032e522b810babd8e3148e9f0d588af9e95e93b97ffb58566b837fdbd31f7f: 5.92 ± 0.00 @ 2024-07-04T15:16:26+09:00 (now)
50c67b3fd225db8912a424dd4baed60ffdde625ed2feaaf283724f9608fea266: 0.75 ± 0.00 @ 2024-07-04T15:16:26+09:00 (now)
36032e522b810babd8e3148e9f0d588af9e95e93b97ffb58566b837fdbd31f7f: 5.92 ± 0.00 @ 2024-07-04T15:16:26+09:00 (now)
50c67b3fd225db8912a424dd4baed60ffdde625ed2feaaf283724f9608fea266: 0.75 ± 0.00 @ 2024-07-04T15:16:26+09:00 (now)
36032e522b810babd8e3148e9f0d588af9e95e93b97ffb58566b837fdbd31f7f: 5.92 ± 0.00 @ 2024-07-04T15:16:27+09:00 (now)
50c67b3fd225db8912a424dd4baed60ffdde625ed2feaaf283724f9608fea266: 0.75 ± 0.00 @ 2024-07-04T15:16:26+09:00 (now)
36032e522b810babd8e3148e9f0d588af9e95e93b97ffb58566b837fdbd31f7f: 5.92 ± 0.00 @ 2024-07-04T15:16:27+09:00 (now)
50c67b3fd225db8912a424dd4baed60ffdde625ed2feaaf283724f9608fea266: 0.75 ± 0.00 @ 2024-07-04T15:16:26+09:00 (now)
36032e522b810babd8e3148e9f0d588af9e95e93b97ffb58566b837fdbd31f7f: 5.92 ± 0.00 @ 2024-07-04T15:16:27+09:00 (now)
50c67b3fd225db8912a424dd4baed60ffdde625ed2feaaf283724f9608fea266: 0.75 ± 0.00 @ 2024-07-04T15:16:26+09:00 (now)
36032e522b810babd8e3148e9f0d588af9e95e93b97ffb58566b837fdbd31f7f: 5.92 ± 0.00 @ 2024-07-04T15:16:27+09:00 (now)
50c67b3fd225db8912a424dd4baed60ffdde625ed2feaaf283724f9608fea266: 0.75 ± 0.00 @ 2024-07-04T15:16:26+09:00 (now)
36032e522b810babd8e3148e9f0d588af9e95e93b97ffb58566b837fdbd31f7f: 5.92 ± 0.00 @ 2024-07-04T15:16:29+09:00 (now)
50c67b3fd225db8912a424dd4baed60ffdde625ed2feaaf283724f9608fea266: 0.75 ± 0.00 @ 2024-07-04T15:16:26+09:00 (now)
36032e522b810babd8e3148e9f0d588af9e95e93b97ffb58566b837fdbd31f7f: 5.92 ± 0.00 @ 2024-07-04T15:16:29+09:00 (now)
50c67b3fd225db8912a424dd4baed60ffdde625ed2feaaf283724f9608fea266: 0.75 ± 0.00 @ 2024-07-04T15:16:30+09:00 (now)
36032e522b810babd8e3148e9f0d588af9e95e93b97ffb58566b837fdbd31f7f: 5.92 ± 0.00 @ 2024-07-04T15:16:29+09:00 (now)
50c67b3fd225db8912a424dd4baed60ffdde625ed2feaaf283724f9608fea266: 0.75 ± 0.00 @ 2024-07-04T15:16:31+09:00 (now)
TXID: 2UeXd6exhDgiEhjCLLmHkE2NJ17ZraaXUF49bHMEMnRh
36032e522b810babd8e3148e9f0d588af9e95e93b97ffb58566b837fdbd31f7f: 5.92 ± 0.00 @ 2024-07-04T15:16:29+09:00 (now)
50c67b3fd225db8912a424dd4baed60ffdde625ed2feaaf283724f9608fea266: 0.75 ± 0.00 @ 2024-07-04T15:16:31+09:00 (now)
Skipping 10, pushing newest data only
[snip]
```

## Authors

- Jacob Lindahl <jacob.lindahl@near.org> [@sudo_build](https://twitter.com/sudo_build)
