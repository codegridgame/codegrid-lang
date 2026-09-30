# Browser Level Adapter Rules

Depend only on codegrid-level-api among workspace crates. Own JavaScript string
conversion and lifecycle glue; never duplicate compilation, evaluation, scoring,
validation, or hidden-result policy. Check payload types and UTF-8 byte ceilings
before copying JS strings. Preserve wide values through original JSON strings.
Require CODEGRID_WASM_MAX_MEMORY_BYTES for wasm32 builds and verify the encoded
linear-memory maximum in actual-host tests. This ceiling does not cap browser
process or JavaScript heap memory. Test actual browser worker execution before
claiming browser parity. Native tests and Node are supplemental evidence.
