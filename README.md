## About

`cairo-replay` is a CLI application to replay Cairo transactions from Pathfinder database and extract `libfunc` usage statistics.

In the near future it will support `papyrus` database.

## How to use

```bash
cargo run --release -- --db-path <PATHFINDER_DB> --start-block <BLOCK_NUM> --end-block <BLOCK_NUM>
```
