# `yuv-cli`

CLI tool for managing YUV transactions.

## Features

- Create a YUV transaction (`transfer`, `issue`, `freeze`):
  - Issue an asset from your pair of keys;
  - Transfer issued tokens;
  - Freeze, unfreeze YUV outputs;
- Communicate with a YUV node (`node` subcommand):
  - Provide pixel proofs to the YUV node;
  - Get YUV transactions from the YUV node;
- Validate proofs locally (`validate` subcommand);
- Generate YUV addresses, key-pairs, pixel hashes (`generate` subcommand);
- Convert instances between each other (`convert` subcommand).

## Build and install

Clone git repository:

```sh
git clone https://github.com/akitamiabtc/yuv.git
```

Install using cargo:

```sh
cargo install --path ./apps/cli
```

From now, if you've added `$HOME/.cargo/bin` to your `$PATH`, `yuv-cli`
should be available from your terminal session.

## Usage

Setup configuration file:

```toml
# config.toml
private_key = "your_private_key"

storage = "path/to/storage"

[bitcoin_provider]
type = "bitcoin_rpc"
url = "http://127.0.0.1:18443" # bitcoin node RPC url
network = "regtest"
auth = { username = "admin1", password = "123" }
# Start syncing the blockchain history from the certain timestamp
start_time = 0

# Or if you want to use Esplora:
# [bitcoint-provider]
# type = "esplora"
# url = "http://127.0.0.1:30000"
# network = "regtest"
# # stop gap - It is a setting that determines when to stop fetching transactions for a set of
# # addresses by indicating a gap of unused addresses. For example, if set to 20, the syncing
# # mechanism would stop if it encounters 20 consecutive unused addresses.
# stop_gap = 20


[yuv_rpc]
url = "http://127.0.0.1:18333"

# The fee rate strategy. Possible values:
# - { type = "estimate", target_blocks: 2 } The fee rate is fetched from Bitcoin RPC. If an error
#   occurs, the tx building process is interrupted.
# - { type = "manual", fee_rate = 1.0 } Default fee rate is used.
# - { type = "try_estimate", fee_rate = 1.0, target_blocks: 2 } The fee rate is fetched
#   automatically from Bitcoin RPC. If an error occurs, the default fee rate is used.
# NOTE: fee_rate is measured in sat/vb.
# https://developer.bitcoin.org/reference/rpc/estimatesmartfee.html
[fee_rate_strategy]
type = "manual"
fee_rate = 1.2
```

### Simple scenario

Let's go through some of the scenarios:

1. Synchronize all the wallet history (see [step 1]);
2. Create **USD Issuer** and **EUR Issuer** accounts which will issue tokens to
   users (see [step 2]);
3. Generate two key pairs of keys that will transfer YUV-coins between each other
   (let's name them **Alice** and **Bob**, see [step 3]);
4. Issue **USD** and **EUR** tokens to **Alice** (see [step 4]);
   - Check **Alice**'s balances and UTXO.
5. Transfer issued tokens from **Alice** to **Bob** (see [step 5]);
   - Perform a monochromatic transfer.
   - Perform a multichromatic transfer.
6. Using **USD Issuer**'s keys create a freeze transaction for **Bob**'s output
   (see [step 6]);
7. Using **USD Issuer**'s keys create an unfreeze transaction for **Bob**'s output (see [step 7]);

> We will use [Nigiri] for this demo to setup configured Regtest Bitcoin node and fund our freshly created users with Bitcoins.

[Nigiri]: https://nigiri.vulpem.com/

> When you've installed `nigiri`, start the node using `nigiri start` with some
> helpful daemons like explorer and webapp.

#### 1. Synchronize the wallet history
Use the following command to synchronize your wallet: 

> NOTE: replace the `config.toml` with a path to your configuration file.

``` sh
yuv-cli --config ./config.toml wallet sync
```

It could take some time, so be calm and make a cup of coffee for yourself. Also you can change
`start_time` field in the `[bitcoin_provider]` section to cut down on synchronizing time. If you want to
interrupt the syncing process, use the following command:

``` sh
yuv-cli --config ./config.toml wallet abort-rescan
```

This command will be done in case when you are using `bitcoin_rpc` configuration for
`[bitcoin_provider]` (see  [usage]);

#### 2. Generate **USD Issuer** and **EUR Issuer** key pairs

Generate **EUR Issuer** key pair:

```sh
yuv-cli generate keypair --network regtest
```

RESULT:

```text
Private key: cUK2ZdLQWWpKeFcrrD7BBjiUsEns9M3MFBTkmLTXyzs66TQN72eX
P2TR address: bcrt1phynjv46lc4vsgdyu8qzna4rkx0m6d2s48cjmx8mtcqkey5r23t2swjhv5n
P2WPKH address: bcrt1qplal8wyn20chw4jfdamkk5vnfkpwdm3vyd46ew
```

<details>
<summary>Configuration file for <b>EUR Issuer</b> </summary>

```toml
# eur.toml
private_key = "cUK2ZdLQWWpKeFcrrD7BBjiUsEns9M3MFBTkmLTXyzs66TQN72eX"

storage = ".users/eur"

[bitcoin_provider]
type = "bitcoin_rpc"
url = "http://127.0.0.1:18443"
auth = { username = "admin1", password = "123" }
network = "regtest"
fee_rate_strategy = { type = "manual", fee_rate = 1.0, target = 2 }
start_time = 0

[yuv_rpc]
url = "http://127.0.0.1:18333"

[fee_rate_strategy]
type = "manual"
fee_rate = 1.2
```

</details>

**USD Issuer** keypair:

```text
Private key: cNMMXcLoM65N5GaULU7ct2vexmQnJ5i5j3Sjc6iNnEF18vY7gzn9
P2TR address: bcrt1p4v5dxtlzrrfuk57nxr3d6gwmtved47ulc55kcsk30h93e43ma2eqvrek30
P2WPKH address: bcrt1qycd9xdayguzayn40ua56slsdm0a9ckn3n34tv0
```

<details>
<summary>Configuration file for <b>USD Issuer</b> </summary>

```toml
# usd.toml
private_key = "cNMMXcLoM65N5GaULU7ct2vexmQnJ5i5j3Sjc6iNnEF18vY7gzn9"

storage = ".users/usd"

[bitcoin_provider]
type = "bitcoin_rpc"
url = "http://127.0.0.1:18443"
auth = { username = "admin1", password = "123" }
network = "regtest"
fee_rate_strategy = { type = "manual", fee_rate = 1.0, target = 2 }
start_time = 0

[yuv_rpc]
url = "http://127.0.0.1:18333"

[fee_rate_strategy]
type = "manual"
fee_rate = 1.2
```

</details>

Also, lets fund issuers with one Bitcoin:

```sh
nigiri faucet bcrt1qplal8wyn20chw4jfdamkk5vnfkpwdm3vyd46ew 1
nigiri faucet bcrt1qycd9xdayguzayn40ua56slsdm0a9ckn3n34tv0 1
```

#### 3. Generate **Alice** and **Bob** key pairs

Generate a key pair for **Alice**:

```text
Private key: cQb7JarJTBoeu6eLvyDnHYNr6Hz4AuAnELutxcY478ySZy2i29FA
P2TR address: bcrt1phhfvq20ysdh6ht8fhtp7e8xfemva23lr703mtyrnuv7fkdggayvsz8x8gd
P2WPKH address: bcrt1q69j54cjd44wuvaqv4lmnyrw89ve4ufq3cx37mr
```

<details>
<summary>Configuration file for <b>Alice</b></summary>

```toml
# alice.toml
private_key = "cQb7JarJTBoeu6eLvyDnHYNr6Hz4AuAnELutxcY478ySZy2i29FA"

storage = ".users/alice"

[bitcoin_provider]
type = "bitcoin_rpc"
url = "http://127.0.0.1:18443"
auth = { username = "admin1", password = "123" }
network = "regtest"
fee_rate_strategy = { type = "manual", fee_rate = 1.0, target = 2 }
start_time = 0

[yuv_rpc]
url = "http://127.0.0.1:18333"

[fee_rate_strategy]
type = "manual"
fee_rate = 1.2
```

</details>

and **Bob**:

```text
Private key: cUrMc62nnFeQuzXb26KPizCJQPp7449fsPsqn5NCHTwahSvqqRkV
P2TR address: bcrt1p03egc6nv2ardypk2qpwru20sv7pfsxrn43wv7ts785rq5s8a8tmqjhunh7
P2WPKH address: bcrt1q732vnwgml595glrucr00rt8584x58mjp6xtnmf
```

<details>
<summary>Configuration file for <b>Bob</b></summary>

```toml
# bob.toml
private_key = "cUrMc62nnFeQuzXb26KPizCJQPp7449fsPsqn5NCHTwahSvqqRkV"

storage = ".users/bob"

[bitcoin_provider]
type = "bitcoin_rpc"
url = "http://127.0.0.1:18443"
auth = { username = "admin1", password = "123" }
network = "regtest"
fee_rate_strategy = { type = "manual", fee_rate = 1.0, target = 2 }
start_time = 0

[yuv_rpc]
url = "http://127.0.0.1:18333"

[fee_rate_strategy]
type = "manual"
fee_rate = 1.2
```

</details>

Also, lets copy their keys to environmental variables:

```sh
export ALICE="bcrt1phhfvq20ysdh6ht8fhtp7e8xfemva23lr703mtyrnuv7fkdggayvsz8x8gd"
export BOB="bcrt1p03egc6nv2ardypk2qpwru20sv7pfsxrn43wv7ts785rq5s8a8tmqjhunh7"
export USD="bcrt1p4v5dxtlzrrfuk57nxr3d6gwmtved47ulc55kcsk30h93e43ma2eqvrek30"
export EUR="bcrt1phynjv46lc4vsgdyu8qzna4rkx0m6d2s48cjmx8mtcqkey5r23t2swjhv5n"
```

#### 4. Create issuances for **Alice**

Now we are ready to create issuance of 10000 **USD** tokens for **Alice**:

```sh
yuv-cli --config ./usd.toml issue --amount 10000 --recipient $ALICE
```

Where `amount` is issuance amount, `recipient` - **Alice**'s public key (read
from environment variable added in [step 2]).

RESULT:

```text
tx id: 2a92796ba4d385caf7fbc392d2793fb3ffd3cf53bcb17c884fa1840100eb29f5
type: Issue
data:
  output_proofs:
    0:
      pixel:
        luma:
          amount: 10000
        chroma: ab28d32fe218d3cb53d330e2dd21db5b32dafb9fc5296c42d17dcb1cd63beab2
      inner_key: 02bdd2c029e4836fabace9bac3ec9cc9ced9d547e3f3e3b59073e33c9b3508e919
```

As the result, you will get the transaction ID and structure of the issuance
proof of the YUV transaction. By parameters obtained from configuration file,
`yuv-cli` will send it for broadcasting to YUV node with created proofs, where
the node will wait until the tranasction is mined to check it before accepting.

Using `nigiri` let's mine the next block:

```sh
nigiri rpc --generate 1
```

Check that the transaction has been accepted by the node:

```sh
yuv-cli --config ./usd.toml get --txid 2a92796ba4d385caf7fbc392d2793fb3ffd3cf53bcb17c884fa1840100eb29f5
```

As a sign of acceptance, you would receive a YUV transaction in JSON format.

Also, we can check current **Alice**'s balances:

```sh
yuv-cli --config ./alice.toml balances
```

RESULT:

```text
ab28d32fe218d3cb53d330e2dd21db5b32dafb9fc5296c42d17dcb1cd63beab2: 10000
```

Let's do the same with **EUR Issuer**:

```sh
yuv-cli --config ./eur.toml issue --amount 10000 --recipient $ALICE
nigiri rpc --generate 1
```

And check balances again:

```sh
yuv-cli --config ./alice.toml balances
```

RESULT:

```text
bc1p4v5dxtlzrrfuk57nxr3d6gwmtved47ulc55kcsk30h93e43ma2eqkj9l76: 10000
bc1phynjv46lc4vsgdyu8qzna4rkx0m6d2s48cjmx8mtcqkey5r23t2s5rt9mx: 10000
```

#### 5. Transfer from **Alice** to **Bob**

Now, let's move on to the transfer. Fund **Alice** with one Bitcoin:

```sh
nigiri faucet bcrt1qm5wu5zjyswyw877kq8dup6k02nef29wwc2tcwu 1
```

We are ready for transfer of 1000 **USD** tokens from **Alice** to **Bob**:

```sh
yuv-cli --config ./alice.toml transfer \
    --chroma $USD \
    --amount 1000 \
    --recipient $BOB
```

RESULT:

```text
tx id: 477df4cb007a46fe9efd7de75ffa7012846d9babea3f31bbb50c9b93f12ff7f5
type: Transfer
data:
  input_proofs:
    0:
      type: Sig
      data:
        pixel:
          luma:
            amount: 10000
          chroma: ab28d32fe218d3cb53d330e2dd21db5b32dafb9fc5296c42d17dcb1cd63beab2
        inner_key: 02bdd2c029e4836fabace9bac3ec9cc9ced9d547e3f3e3b59073e33c9b3508e919
  output_proofs:
    0:
      type: Sig
      data:
        pixel:
          luma:
            amount: 1000
          chroma: ab28d32fe218d3cb53d330e2dd21db5b32dafb9fc5296c42d17dcb1cd63beab2
        inner_key: 027c728c6a6c5746d206ca005c3e29f06782981873ac5ccf2e1e3d060a40fd3af6
    1:
      type: Sig
      data:
        pixel:
          luma:
            amount: 9000
          chroma: ab28d32fe218d3cb53d330e2dd21db5b32dafb9fc5296c42d17dcb1cd63beab2
        inner_key: 02bdd2c029e4836fabace9bac3ec9cc9ced9d547e3f3e3b59073e33c9b3508e919
```

Generate block using `nigiri`:

```sh
nigiri rpc --generate 1
```

And check balances of both users:

```sh
yuv-cli --config ./alice.toml balances
```

RESULT:

```text
bc1p4v5dxtlzrrfuk57nxr3d6gwmtved47ulc55kcsk30h93e43ma2eqkj9l76: 9000
bc1phynjv46lc4vsgdyu8qzna4rkx0m6d2s48cjmx8mtcqkey5r23t2s5rt9mx: 10000
```

```sh
yuv-cli --config ./bob.toml balances
```

RESULT:

```text
bc1p4v5dxtlzrrfuk57nxr3d6gwmtved47ulc55kcsk30h93e43ma2eqkj9l76: 1000
```

##### Multichromatic transfers

We covered monochromatic transfers above (i.e. each transfer contained a single chroma).
Now, let's try to perform a multichromatic transfer and send both **EUR** and **USD** from **Alice** to **Bob** in a single transfer.

As Alice's balance is already filled with some **EUR** and **USD**, we are ready to make a transfer:

```sh
yuv-cli --config ./alice.toml transfer \
    --chroma $USD \
    --amount 500 \
    --recipient $BOB \
    --chroma $EUR \
    --amount 1000 \
    --recipient $BOB
```

RESULT:

```text
tx id: 6936880d51e5fd92b6dd3c754905b538f146f69942080c4f3dca8b99d5f1f086
type: Transfer
data:
  input_proofs:
    0:
      type: Sig
      data:
        pixel:
          luma:
            amount: 9000
          chroma: ab28d32fe218d3cb53d330e2dd21db5b32dafb9fc5296c42d17dcb1cd63beab2
        inner_key: 02bdd2c029e4836fabace9bac3ec9cc9ced9d547e3f3e3b59073e33c9b3508e919
    1:
      type: Sig
      data:
        pixel:
          luma:
            amount: 10000
          chroma: b92726575fc55904349c38053ed47633f7a6aa153e25b31f6bc02d92506a8ad5
        inner_key: 02bdd2c029e4836fabace9bac3ec9cc9ced9d547e3f3e3b59073e33c9b3508e919
  output_proofs:
    0:
      type: Sig
      data:
        pixel:
          luma:
            amount: 500
          chroma: ab28d32fe218d3cb53d330e2dd21db5b32dafb9fc5296c42d17dcb1cd63beab2
        inner_key: 027c728c6a6c5746d206ca005c3e29f06782981873ac5ccf2e1e3d060a40fd3af6
    1:
      type: Sig
      data:
        pixel:
          luma:
            amount: 1000
          chroma: b92726575fc55904349c38053ed47633f7a6aa153e25b31f6bc02d92506a8ad5
        inner_key: 027c728c6a6c5746d206ca005c3e29f06782981873ac5ccf2e1e3d060a40fd3af6
    2:
      type: Sig
      data:
        pixel:
          luma:
            amount: 8500
          chroma: ab28d32fe218d3cb53d330e2dd21db5b32dafb9fc5296c42d17dcb1cd63beab2
        inner_key: 02bdd2c029e4836fabace9bac3ec9cc9ced9d547e3f3e3b59073e33c9b3508e919
    3:
      type: Sig
      data:
        pixel:
          luma:
            amount: 9000
          chroma: b92726575fc55904349c38053ed47633f7a6aa153e25b31f6bc02d92506a8ad5
        inner_key: 02bdd2c029e4836fabace9bac3ec9cc9ced9d547e3f3e3b59073e33c9b3508e919
```

Generate a block using `nigiri`:

```sh
nigiri rpc --generate 1
```

And check balances of both users:

```sh
yuv-cli --config ./alice.toml balances
```

RESULT:

```text
bc1p4v5dxtlzrrfuk57nxr3d6gwmtved47ulc55kcsk30h93e43ma2eqkj9l76: 8500
bc1phynjv46lc4vsgdyu8qzna4rkx0m6d2s48cjmx8mtcqkey5r23t2s5rt9mx: 9000
```

```sh
yuv-cli --config ./bob.toml balances
```

RESULT:

```text
bc1p4v5dxtlzrrfuk57nxr3d6gwmtved47ulc55kcsk30h93e43ma2eqkj9l76: 1500
bc1phynjv46lc4vsgdyu8qzna4rkx0m6d2s48cjmx8mtcqkey5r23t2s5rt9mx: 1000
```

**NOTE:** it's also acceptable to specify different recipients in a multichromatic transfer.

#### 6. Freeze Bob's output

Let's see **Bob**'s YUV UTXOS:

```sh
yuv-cli --config ./bob.toml utxos --chroma $USD
```

RESULT:

```text
477df4cb007a46fe9efd7de75ffa7012846d9babea3f31bbb50c9b93f12ff7f5:0 1000
6936880d51e5fd92b6dd3c754905b538f146f69942080c4f3dca8b99d5f1f086:0 500
```

Using **USD Issuer**'s keys create a freeze transaction for **Bob**'s output:

In case of using `esplora` as a `bitcoin_provider` you should pass `--rpc-url` and `--rpc-auth` where
format of `--rpc-auth` is `[login:paasword]`. 
If your node doesn't require auth credentials, just use `--rpc-url`. The example below: 

``` sh
# In case when the Bitcoin RPC node requires auth credentials.
yuv-cli --config ./usd.toml freeze 477df4cb007a46fe9efd7de75ffa7012846d9babea3f31bbb50c9b93f12ff7f5 0 --rpc-url http://127.0.0.1:18443 --rpc-auth admin1:123

# In case when the Bitcoin RPC node doesn't require auth credentials.
yuv-cli --config ./usd.toml freeze 477df4cb007a46fe9efd7de75ffa7012846d9babea3f31bbb50c9b93f12ff7f5 0 --rpc-url http://127.0.0.1:18443
```

When you use `bitcoin_rpc` as a `bitcoin_provider` the following command will freeze the tokens:

```sh
yuv-cli --config ./usd.toml freeze 477df4cb007a46fe9efd7de75ffa7012846d9babea3f31bbb50c9b93f12ff7f5 0
```

RESULT:

```text
Transaction broadcasted: abf54fedcdd13158b425f2841587f6874c5cc25935c3f2bd0b863ab7bac8e854
```

Generate block using `nigiri`:

```text
nigiri rpc --generate 1
```

> Also, you can check if that transaction was indexed by node:

```sh
yuv-cli --config ./usd.toml get --txid e8891f004680eefdd8faf149073796d1b189e39454ebd8a68a112fed2b135aae
```

And check **Bob**s UTXOS after that:

```sh
yuv-cli --config ./bob.toml utxos $USD
```

Now **Bob** has one less UTXO:

```text
6936880d51e5fd92b6dd3c754905b538f146f69942080c4f3dca8b99d5f1f086:0 500
```

#### 7. Create unfreeze transaction for **Bob**'s output

Using **Issuer**'s keys create an unfreeze transaction for **Bob**'s output:
``` sh
# (Esplora) In case when the Bitcoin RPC node requires auth credentials.
yuv-cli --config ./usd.toml unfreeze 477df4cb007a46fe9efd7de75ffa7012846d9babea3f31bbb50c9b93f12ff7f5 0 --rpc-url http://127.0.0.1:18443 --rpc-auth admin1:123

# (Esplora) In case when the Bitcoin RPC node doesn't require auth credentials.
yuv-cli --config ./usd.toml unfreeze 477df4cb007a46fe9efd7de75ffa7012846d9babea3f31bbb50c9b93f12ff7f5 0 --rpc-url http://127.0.0.1:18443

# (BitcoinRpc)
yuv-cli --config ./usd.toml unfreeze 477df4cb007a46fe9efd7de75ffa7012846d9babea3f31bbb50c9b93f12ff7f5 0
```

RESULT:

```text
Transaction broadcasted: 5faeae04cd7b4d853866eb427896a3a6fff89f2e2f320def1950cd30e0c43b8f
```

Generate block:

```sh
nigiri rpc --generate 1
```

Also, you may check if that transaction was indexed by node:

```sh
yuv-cli --config ./usd.toml get --txid 5faeae04cd7b4d853866eb427896a3a6fff89f2e2f320def1950cd30e0c43b8f
```

And finally, check **Bob**'s YUV UTXOS:

```sh
yuv-cli --config ./bob.toml utxos $USD
```

RESULT:

```text
477df4cb007a46fe9efd7de75ffa7012846d9babea3f31bbb50c9b93f12ff7f5:0 1000
6936880d51e5fd92b6dd3c754905b538f146f69942080c4f3dca8b99d5f1f086:0 500
```

#### 8. Bulletproofs

Bulletproofs are used to prove that the value is in some range without revealing it

Let's start with the issuance of 10000 **USD** tokens for **Alice**:

```sh
export ISSUANCE_TX_ID=$(yuv-cli --config ./usd.toml bulletproof issue --satoshis 10000 --value 10000 --recipient $ALICE)
```

Generate block using `nigiri`:

```sh
nigiri rpc --generate 1
```

Let's check that Pedersen's commitment to the issuance bulletproof that we received is valid:

```sh
yuv-cli --config ./alice.toml bulletproof check --value 10000 --tx-id $ISSUANCE_TX_ID --tx-vout 0 --sender $USD
```

Now, let's transfer 1000 **USD** tokens from **Alice** to **Bob**:

```sh
export TRANSFER_TX_ID=$(yuv-cli --config alice.dev.toml bulletproof transfer --value 1000 --residual 9000 --satoshis 2000 --residual-satoshis 7000 --chroma $USD --recipient $BOB --input-tx-id $ISSUANCE_TX_ID --input-tx-vout 0)
```

Generate block using `nigiri`:

```sh
nigiri rpc --generate 1
```

Finally check that Pedersen's commitment to the transfer bulletproof that we received is valid:

```sh
yuv-cli --config ./bob.toml bulletproof check --value 1000 --tx-id $TRANSFER_TX_ID --tx-vout 0 --sender $ALICE
```

[step 1]: #1-synchronize-the-wallet-history
[step 2]: #2-generate-usd-issuer-and-eur-issuer-key-pairs
[step 3]: #3-generate-alice-and-bob-key-pairs
[step 4]: #4-create-issuances-for-alice
[step 5]: #5-transfer-from-alice-to-bob
[step 6]: #6-freeze-bobs-output
[step 7]: #7-create-unfreeze-transaction-for-bobs-output
[step 8]: #8-bulletproofs
[usage]: #usage