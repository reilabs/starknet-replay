# Starknet Replay

`starknet-replay` is a CLI application to replay Starknet transactions locally
using data from the Pathfinder database. It reports the frequency with which
each `libfunc` has been called when replaying the transactions.

It's also possible to export the histogram of the most frequently used libfuncs
by number of calls. The data plotted in the histogram is filtered to only
include the libfuncs that amount to 80% of the total calls in the replay. This
helps readability and visual analysis.

Only `INVOKE` transactions of Sierra contracts are used for this report because
only Sierra contracts use libfuncs and only `INVOKE` transactions execute Sierra
code. Rejected transactions are included because they are still useful to
indicate which `libfunc` users need.

Gathering this data allows actions to be taken based on libfunc usage, examples
of which include designating certain functions for extra scrutiny based on their
popularity and allowing deprecation of less-used libfuncs. This information
allows allows analysis of how libfunc usage changes over time, and how new
functions are adopted by the community.

In the future, this tool is likely to evolve to support:

- Use of the `papyrus` database as an alternative to pathfinder.
- Analysis of changes to the storage layer during the transaction replay.

## How to Use

```bash
cargo run --release -- --db-path <PATHFINDER_DB> --start-block <BLOCK_NUM> --end-block <BLOCK_NUM>
```

`PATHFINDER_DB` is the path of the Pathfinder sqlite database. The Pathfinder
database is generated from running a `pathfinder` node. Further information is
available
[here](https://github.com/eqlabs/pathfinder/tree/v0.11.6?tab=readme-ov-file#database-snapshots).

This tool makes use of `tracing` library for log purposes. For this reason set
`RUST_LOG` at least at `info` level to see the raw output of libfunc statistics.

## Example

```bash
cargo run -- --db-path ../pathfinder/mainnet.sqlite --start-block 632917 --end-block 632917 --svg-out "histogram.svg"
```

The command above replays all transactions of block
[632917](https://starkscan.co/block/632917#transactions) and saves the libfunc
histogram in the file named `"histogram.svg"`.

## Limitations

- Libfunc frequency results haven't been checked yet
- Whole blocks replay is slow

## Requirements

Tested only on `pathfinder-v0.11.x`. More recent version of Pathfinder use a
size optimised database which may require some changes. For latest info check
issue #17.

## Useful links

- [Pathfinder](https://github.com/eqlabs/pathfinder)
- [Papyrus](https://github.com/starkware-libs/papyrus)
- [Starknet](https://docs.starknet.io/documentation/)
- [Libfunc](https://github.com/lambdaclass/cairo_native?tab=readme-ov-file#implemented-library-functions)
- [Starknet Transactions](https://docs.starknet.io/documentation/architecture_and_concepts/Network_Architecture/transactions/)
- [`tracing`](https://github.com/tokio-rs/tracing)
