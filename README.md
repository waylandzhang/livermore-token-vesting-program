# Livermore Token Vesting Program

A secure, Solana-based token vesting solution for the Livermore Project, built to facilitate controlled token distribution according to predefined vesting schedules.

## Overview

The Livermore Token Vesting Program is a Solana smart contract that manages token vesting for the Livermore Project. It enables the creation of vesting schedules for different stakeholder groups, enforces issuance limits, and provides a secure mechanism for releasing tokens according to the planned schedule.

This program is a critical component of the [Livermore Project](https://www.investagentpro.com/solana_token_launch), which aims to integrate AI and Web3 technologies through the AI-driven Invest Agent Pro platform.

## Features

- **Multiple Vesting Types**:
  - **Market**: 1-month lockup
  - **Data Purchase**: Split lockup (50% for 6 months, 50% for 1 year)
  - **Team**: 3-year lockup

- **Supply Control Mechanisms**:
  - Enforces yearly issuance limits for each vesting type
  - Tracks annual issuance across all vesting types
  - Project planned to issue tokens over 5 years

- **Security Features**:
  - Uses Program Derived Addresses (PDAs) for secure token custody
  - Implements comprehensive validation and security checks
  - Compatible with SPL Token 2022 standard

## Token Allocation

The Livermore Project plans to issue 210 million tokens over 5 years with the following annual allocation:

| Vesting Type   | Annual Allocation | Total (5 years) | Lockup Period            |
|----------------|-------------------|-----------------|--------------------------| 
| Market         | 12.6M tokens      | 63M tokens      | 1 month                  |
| Data Purchase  | 21M tokens        | 105M tokens     | 50% for 6m, 50% for 1y  |
| Team           | 8.4M tokens       | 42M tokens      | 3 years                  |
| **Total**      | **42M tokens**    | **210M tokens** | -                        |

## Technical Usage

### Creating a Vesting Schedule

To create a vesting schedule, you need to provide:
- Token mint address
- Destination wallet address
- Vesting type (Market, DataPurchase, or Team)
- Project year (1-5)
- Start time (timestamp)
- Amount of tokens to vest

The program validates that the request is within allowable limits and creates the appropriate vesting schedule.

### Unlocking Tokens

When tokens are ready for release according to the vesting schedule, either:
- The creator of the vesting schedule, or
- The token recipient

Can initiate the unlock process. The program verifies that the tokens' lock period has expired before releasing them to the recipient's wallet.

## Building and Deployment

```bash
# Build the program
cargo build-sbf

# Deploy to Solana devnet for testing
solana program deploy --program-id <PROGRAM_ID_KEYPAIR> target/deploy/livermore_vesting_2022.so

# Deploy to Solana mainnet for production
solana program deploy --program-id <PROGRAM_ID_KEYPAIR> target/deploy/livermore_vesting_2022.so --url mainnet-beta
```

## Project Timeline

The Livermore Project is scheduled to launch on March 10, 2025, with token distribution continuing over a 5-year period according to the vesting schedules defined in this program.

## Learn More

For more information about the Livermore Project, its tokenomics, and the underlying AI-driven investment platform, visit the [official whitepaper](https://www.investagentpro.com/solana_token_launch).

## License

This program is licensed under the MIT License - see the [LICENSE](./LICENSE) file for details.

---

© 2025 CitaBlock Technology Corporation. All rights reserved. 