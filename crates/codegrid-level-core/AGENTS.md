# Level Core Rules

This crate owns logical level decoding, structural restrictions, deterministic
evaluation, metrics, constraints, and rating above the language core.

Dependencies point to IR, model, and VM only; serde is transport-neutral JSON
support. Accept only verified IR. Never compile source or access files, clocks,
network, host randomness, editor APIs, or platform bindings. Keep scene policy
out of language crates. Hidden test details must not enter public results.
The level specifications and recorded decisions define behavior; report genuine
gaps before choosing semantics. Validate generated instructions after commit.
