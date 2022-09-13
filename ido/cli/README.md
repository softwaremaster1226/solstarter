# SolStarter CLI

## Instruction

To create market you need first create new stake token mint.

For this use [`spl-token` program CLI](https://spl.solana.com/token) and install it using `cargo install spl-token-cli`

Then to create an new token mint run `spl-token create-token`.

You will receive new mint key. Use this key in `create-market` command.

Example: 

```rust
cargo run create-market --stake-token DuaZdvzGSp1YysfTyUsV7Pni4qeKHuo3HtHvtHr2AHqq --tier-1 50 --tier-2 100 --tier-3 150 --tier-4 200 --lock-in 100 --lock-out 100
````

This will give you the following output:

```sh
IDO market account: 54XXruAsqkWvrEom2VR6BrEieVsGJt3ccv3rZYDyfqVp
Stake pool account: 9VwAJxSM9EbCLrYRLMQMEmRVMyP9qdjqB6hvrkn2GxMs
Stake pool mint: 65xkvqwBisxPsq4SzBxa7AyNqUfzwAWq4gvRtsF4uhzh
Stake pool token account: 3LCtRcWxogxzhQPhrtotgcSa9eJKU5Z72vjBcSZ5Kw9n
Signature: 4Vo8atTNTQLVEbUF7oVehg6dWeRhRoLHmXbxLvfJC3hD3uURfCkcgUxvFHst48Sn94YmzKJDht1m5uArR3m4ntfD
```

When you have created market you can create new pool but before you also need mint collection and mint distribution keys.

You can create it with command `spl-token create-token` in spl-token CLI as in first example.

In `pool-owner` parameter is better to set your wallet address. If you don't know it you can run `solana address` in your terminal.

Now you have all the necessary parameters to create new pool.

Example:

```rust
cargo run create-pool --market 54XXruAsqkWvrEom2VR6BrEieVsGJt3ccv3rZYDyfqVp --mint-collection So11111111111111111111111111111111111111112 --mint-distribution GJhJytbuuHzxjQw2YW8QBK6S5T9pR3jc7Yme9qGq6GHy --pool-owner C2EohykNinLTZvF1desCxLa7CpWEesqZAxCUgnGJiz99 --is-whitelist false --is-kyc false --price 10 --goal-max 50 --goal-min 10 --amount-min 1 --amount-max 5 --time-start 1624213500 --time-finish 1624214100 --stage-1 0 --stage-2 0
```

This will give you the following output:

```
IDO pool account: 7wqR22gwef7dWnmgSvEhTjLQzzX2TzJDsQyEQNsnK5E8
Token collection account: 376qHvUTTBkvRAwRvXBn1yR8cCwRgfUNNQgTbZbrzPfT
Token distribution account: 54kBkiz2vxxLY2RxWMEnHk4WA3cNLa6ArrNnQVZ6iSz3
Pool mint account: 8C2qnSwGscKscpBg8AYyjzbQwzzxqASYTTgdQihWwKg2
Tx hash of preparation signature with accounts creation: 37M2xzGnC1JHNsCRNvXpvgcZGoovQZqegTk6L6fvz7gs9juw1ubN4GQsbVR3j58Bxcg9NdPgmCo1oQkdYNXjzjvS
Signature: 4FKRzKtRggzLcCxNA9K6gx4kzRqt7uqFouHuT9cNirVAEfWpqxPCFhp2w9dbvM7jc4GYBcJC53maCR5SCSZ6mwxC
```

Now you can mint whitelist tokens for users. In this directory you can see file **user_accs.csv**. You have to write list of user's wallet keys and whitelist token accounts.
If user doesn't have whitelist token account you can write only user's wallet key and CLI will create associated token account.

Below you can see example of filled **user_accs.csv** file.

```
wallet,whitelist_token_acc
BmyPUuNukKDVdtytcpeR1YmxLHEKN4mcjYTRbRckNsoa,
5awWeGRCri85XM48YwRWXVW2DT2A4hbBkUosAa86d1Ky,FUd2uwpkkiUQ6SiAKPPUKWw6oXgiCjejmgFTpHJ3LntU
3VvLDXqJbw3heyRwFxv8MmurPznmDVUJS9gPMX2BDqfM,
```

To mint whitelist tokens to users run this command: 

```rust
cargo run add-to-whitelist --pool 3Dpc94xY24jG2TbEoLMXNtbGbmPYzCvDdWmdGWP2nDyD --whitelist-accs ./ido/cli/user_accs.csv
```

To add tokens to the accout distribution follow these steps:

```
// Create account for the AccountDistribution Mint
spl-token create-account GJhJytbuuHzxjQw2YW8QBK6S5T9pR3jc7Yme9qGq6GHy

// Mint tokens for the AccountDistribution Mint
spl-token mint GJhJytbuuHzxjQw2YW8QBK6S5T9pR3jc7Yme9qGq6GHy 50

// Transfer Mint Tokens into the Pool AccountDistribution account
spl-token transfer GJhJytbuuHzxjQw2YW8QBK6S5T9pR3jc7Yme9qGq6GHy 50 54kBkiz2vxxLY2RxWMEnHk4WA3cNLa6ArrNnQVZ6iSz3
```

If you would like to add fake tokens to the collection, pool or distributions accounts follow these steps:

```
spl-token create-account 6LFM6GrxDqVoL6P7NytVmHz2p3wUTyHvDh3zCrkVdcTc (this is the token address of the collected tokens)
spl-token mint 6LFM6GrxDqVoL6P7NytVmHz2p3wUTyHvDh3zCrkVdcTc 1000
spl-token balance 6LFM6GrxDqVoL6P7NytVmHz2p3wUTyHvDh3zCrkVdcTc
spl-token accounts (should show 1000)
spl-token transfer 6LFM6GrxDqVoL6P7NytVmHz2p3wUTyHvDh3zCrkVdcTc 50 GkyqVnjiVfpErPECAsaqqwGLDfpo5afop1EniV73Egwe (the second account is the wallet public key that can hold the collected token)
```