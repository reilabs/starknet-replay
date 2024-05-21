## About

`cairo-replay` is a CLI application to replay Cairo transactions locally using
data from Pathfinder database. It reports the frequency each `libfunc` has been
called when replaying the transactions.

Next features to be added:

- Support OF `papyrus` database
- Replay transactions with changes to the storage layer.

## How to Use

```bash
cargo run --release -- --db-path <PATHFINDER_DB> --start-block <BLOCK_NUM> --end-block <BLOCK_NUM>
```

`PATHFINDER_DB` is obtained from running a `pathfinder` node. Further
information is available
[here](https://github.com/eqlabs/pathfinder/tree/v0.11.6?tab=readme-ov-file#database-snapshots).

Tested only on `pathfinder-v0.11.x`.

## Useful links

- [Pathfinder](https://github.com/eqlabs/pathfinder)
- [Papyrus](https://github.com/starkware-libs/papyrus)
- [Starknet](https://docs.starknet.io/documentation/)
- [Libfunc](https://github.com/lambdaclass/cairo_native?tab=readme-ov-file#implemented-library-functions)
