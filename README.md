## About

`cairo-replay` is a CLI application to replay Starknet transactions locally using
data from Pathfinder database. It reports the frequency each `libfunc` has been
called when replaying the transactions.

Only `INVOKE` transactions of Sierra contracts are used for this report because
only Sierra contracts use libfuncs and only `INVOKE` transactions execute Sierra
code. Rejected transactions are included because they are still useful to indicate which `libfunc` users need.

Gathering of these data can have many benefits among which:

- Knowing which libfuncs require top scrutinity because most frequent
- Knowing if any libfunc can be deprecated because of little use
- Identifying potential new libfuncs due to recurrent patterns
- How usage of libfuncs changes over time
- Rate of adoption of new libfuncs

Next features to be added:

- Support of `papyrus` database
- Replay transactions with changes to the storage layer.

## How to Use

```bash
cargo run --release -- --db-path <PATHFINDER_DB> --start-block <BLOCK_NUM> --end-block <BLOCK_NUM>
```

`PATHFINDER_DB` is the path of the Pathfinder sqlite database. The Pathfinder
database is generated from running a `pathfinder` node. Further information is
available
[here](https://github.com/eqlabs/pathfinder/tree/v0.11.6?tab=readme-ov-file#database-snapshots).

## Example

```bash
cargo run -- --db-path ../pathfinder/mainnet.sqlite --start-block 632917 --end-block 632917
```

The command above replays all transactions of block [632917](https://starkscan.co/block/632917#transactions)

## Limitations

- Some transactions trigger `TransactionExecutionError::ExecutionError`
- Libfunc frequency results haven't been checked yet

## Requirements

Tested only on `pathfinder-v0.11.x`. More recent version of Pathfinder use a
size optimised database which may require some changes. For latest info check issue #17.

## Useful links

- [Pathfinder](https://github.com/eqlabs/pathfinder)
- [Papyrus](https://github.com/starkware-libs/papyrus)
- [Starknet](https://docs.starknet.io/documentation/)
- [Libfunc](https://github.com/lambdaclass/cairo_native?tab=readme-ov-file#implemented-library-functions)
- [Starknet Transactions](https://docs.starknet.io/documentation/architecture_and_concepts/Network_Architecture/transactions/)
