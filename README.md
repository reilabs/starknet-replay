## About

`cairo-replay` is a CLI application to replay Cairo transactions from Pathfinder
database and extract `libfunc` usage statistics.

In the near future it will support `papyrus` database.

## How to Use

```bash
cargo run --release -- --db-path <PATHFINDER_DB> --start-block <BLOCK_NUM> --end-block <BLOCK_NUM>
```

`PATHFINDER_DB` is obtained from running a `pathfinder` node. Further
information is available
[here](https://github.com/eqlabs/pathfinder/tree/v0.11.6?tab=readme-ov-file#database-snapshots).

Currently tested only on `pathfinder-v0.11.x`.
