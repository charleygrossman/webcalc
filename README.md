# webcalc
Simple calculator web service and client to learn Rust.

## Usage
Build the `client` and `server` binaries with `cargo`.

`server` takes three arguments, `host`, `port` and `size`, where `host` and `port` form the `host:port` server address and `size` represents the number of threads to use in the server's worker pool (bounded by the constants `WORKER_POOL_MIN_SIZE` and `WORKER_POOL_MAX_SIZE`).

`client` takes a single `address` argument that is the full `host:port` server address. The `INPUT_FILEPATH` and `OUTPUT_FILEPATH` environment variables must also be set when running `client`; see the `testdata` folder for examples. Each line of the input file represents a single calculation to be processed by the server, and each line of the output file is the result.