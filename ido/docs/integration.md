# Contract Integration

## Instructions

### InitMarket

Initializes new market account and sets up its owner account.

Positional account params:

0. (Writable) Market account to initialize
1. (Read-only, Signer) Account which will be the owner of this market and will sign initialization transactions for individual pools
2. (Read-only) System Rent account, used to check if market account has enough SOL on it to be rent-exempt

Typed params: None

### InitPool

Initializes new pool, attaches it to the market, creates all necessary accounts.

Positional account params:

0. (Writable) Pool account to initialize
1. (Read-only) Market account this pool will belong to
2. (Read-only, Signer) Market owner account, has to sign this transaction. Also pays for accounts creation
3. (Read-only) Mint for the tokens to be collected (users will be paying with these tokens)
4. (Read-only) Mint for the distributed tokens (the one sold through the pool)
5. (Writable) Account to store collected tokens, should be a program account, will be created by the program
6. (Writable) Account to store distributed tokens, should be a program account, will be created by the program
7. (Writable) Account for the pool mint, should be a program account, will be created by the program
8. (Read-only) Pool authority account, will be the owner of all new accounts
9. (Read-only) System Rent account, used to verify rent balances for all the accounts involved
10. (Read-only) System Clock account, used to verify pool start and finish time
11. (Read-only) Token program ID, used to call token program for token account and mint initialization
12. (Writable, Optional) Account for the pool whitelist mint, should be a program account, will be created by the program

Typed params:
- `price_numerator` and `price_denominator` is the price for the distributed token in collected tokens (multiply by numerator and then divide by denominator).
- `goal_max` and `goal_min` are the maximum and minimum amounts in collected tokens for the pool. If the collected amount is less than `goal_min` the pool should refund all the collected tokens.
- `amount_min` and `amount_max` are the minimum and maximum amount of one single investment transaction.
- `time_start` and `time_finish` are the times when the pool starts (can accept collected tokens) and finishes (allows claiming purchased distributed tokens).

### Participate

Issued by the user participating in the pool tokensale. Only allowed for the pool after their start time, but before the finish time.

Positional account params:

0. (Writable) Initialized and currently active pool account
1. (Read-only) Pool authority account
2. (Writable) Account sending collected token from the user to the pool, you should approve spending on this account by the transaction signer before issuing this instruction
3. (Read-only, Signer) Single-use authority which can spend tokens from the previous account
4. (Writable) Account to receive collected tokens, should be pool's collected token's account
5. (Writable) Token account to receive back pool tokens (which can be later exchanged for the distributed tokens)
6. (Writable) Pool mint account, will mint new tokens to the previous account
7. (Read-only) Token program ID, used to call transfer and mint for the collected and pool tokens
8. (Read-only) System Clock account, used to check if pool is currently active
9. (Writable, Optional) Token account holding whitelist tokens, if the pool is whitelist-only a single token will be burned by this instruction. You need to issue approval for the signing authority to burn this 1 token
10. (Writable, Optional) Again, only for whitelist pools, the mint which will be burning user's whitelist tokens (the same as the pool's whitelist mint)

Typed params:
- single `u64` value holding the amount of collected tokens to transfer to the pool.

### Claim

Claims purchased distribution tokens after the pool finish time (if `goal_min` is reached) or refunds collected tokens (if not).

Positional account params:

0. (Read-only) Finished pool account to collect funds from
1. (Read-only) Pool authority, used to control pool token accounts and mints
2. (Writable) User token account holding pool tokens (received after pool participation), will be burned by this action
3. (Read-only, Signer) Single-use user authority approved for burning tokens from the previous account
4. (Writable) Pool mint which will be burning pool tokens
5. (Writable) Pool token account to claim funds from. If the pool was successful then it is the distribution account. Otherwise collection pool account needs to be specified to refund tokens to the user
6. (Writable) User account to receive claimed tokens (just as with the previous account can either be collected or distributed token account)
7. (Read-only) Token program ID, used for burning pool tokens and transfers
8. (Read-only) System Clock account, used to check if the pool is finished collecting funds

Typed params: None

### AddToWhitelist

Called by the pool owner before the pool starts to add particular users to the pool whitelist.

Positional account params:

0. (Read-only) Reference to the pool, which is not yet in the running state, just preparing
1. (Read-only) Pool authority account controlling whitelist mint account
2. (Read-only, Signer) Pool owner account, should sign this instruction
3. (Writable) User account to receive a new minted whitelist token
4. (Writable) Pool whitelist mint account, which will mint the whitelist token to the account above
5. (Read-only) Token program ID, used to mint new token
6. (Read-only) System Clock account, used to check if the pool is still inactive

Typed params: None

### Withdraw

Called by the pool owner after the pool is over to collect the user investments (in collected tokens) and leftover distributed tokens. Or if the pool failed to reach its `goal_min` returns all of the distribution tokens.

Positional account params:

0. (Read-only) Pool account after the sale is over
1. (Read-only, Signer) Pool owner account, should sign this instruction
2. (Writable) Account to collect funds from. Should be pool's collection or distribution token account
3. (Writable) Pool owner's token account to receive tokens from the previous account (either collected or distributed token)
4. (Read-only) Token program ID, used to transfer tokens
5. (Read-only) System Clock account, used to check if pool sale is over

Typed params: None

## Generating Account Addresses

`InitializePool` instruction creates all the required accounts, you just need to supply account public keys as parameters. Below are instructions for each of the accounts:

```rust
Pubkey::find_program_address(
    &[
        &market.to_bytes()[..32],
        &pool.to_bytes()[..32],
        CONSTANT,
    ],
    &id(),
);
```

Where `market` is the market public key, `pool` is the pool public key. And `CONSTANT` is one of the constants defined in `processor.rs`:

- `collection` for the collected token account
- `distribution` for the distributed token account
- `mint` for the pool mint
- `whitelist` for the whitelist mint
- `authority` for the pool authority account, the owner of all tokens and mints above