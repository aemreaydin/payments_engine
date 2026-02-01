# Payments Engine

A streaming CSV payments engine that processes transactions (deposits, withdrawals, disputes, resolves, chargebacks) and outputs client account states.

## Usage

```
cargo run -- transactions.csv > accounts.csv
```

Input CSV format:

```csv
type,client,tx,amount
deposit,1,1,1.0
deposit,2,2,2.0
withdrawal,1,3,1.5
dispute,1,1,
resolve,1,1,
```

Output CSV format:

```csv
client,available,held,total,locked
1,0.5000,0.0000,0.5000,false
2,2.0000,0.0000,2.0000,false
```

## Running Tests

```
cargo test
```

## Design

The engine processes transactions row-by-row in a single pass. Only deposit transactions are stored in memory (for dispute lookups), so memory usage scales with the number of unique deposits rather than total transaction count.

### Module Structure

| Module | Responsibility |
|---|---|
| `transaction.rs` | Input types and CSV deserialization |
| `account.rs` | Account state and output formatting |
| `engine.rs` | Transaction processing logic |
| `io.rs` | CSV reading/writing |
| `error.rs` | Typed error enum |
| `main.rs` | CLI entry point |

### Precision

All monetary values use `rust_decimal::Decimal` for exact decimal arithmetic. Output values are formatted to 4 decimal places. This avoids IEEE 754 floating-point rounding issues (e.g. `0.1 + 0.2 != 0.3`).
