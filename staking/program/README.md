
https://solstarter.org/


# Overview

- Generic token sale - `take money and sell tokens`
- Instead of wait list calculate allocation for those who staked
- The more staked the more allocation
- Staked token must be locked to be considered staked for some tier
- Considered to be staked after X days of being so
- Staking tier need to know all users
- can be used as ticket(staging) to participate in `IDO`

# Example

You stake 1000 SOS(SOlStarter), they are in transfer for 30 days, then in 3 days you can execute `StakeFinish` and receive 100 xSOS, then in 10 minutes another 0.24 xSOS (for example).
For `UnstakeFinish` can have other period of 7 days to swap xSOS to SOS.

## Instructions

### Build and test for program compiled natively
```
$ cargo build
$ cargo test
```

### Build and test the program compiled for BPF
```
$ cargo build-bpf
$ cargo test-bpf
```