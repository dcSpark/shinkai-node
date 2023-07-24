# Shinkai Node

## Tests

Note: You must run tests from the root directory of this repo.

- Use `cargo test -- --test-threads=1` to ensure all tests pass. This runs tests in sequence rather than in parallel.

- Use `cargo test tcp_node_test -- --nocapture --test-threads=1` to run one test and send output to console. Useful for debugging.
