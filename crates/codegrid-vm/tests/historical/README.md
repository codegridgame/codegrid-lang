# Historical VM Tests

`state_tests_full_v2.rs` contains Full execution cases for multi-threading, functions, Custom instructions, Folded Blocks, memory, stacks, and Attachments. The VM crate includes this file in its unit-test module, so the cases run with `cargo test -p codegrid-vm`. Treat the historical cases as regression evidence and extend them with direct conformance tests required by the Full VM specification.
