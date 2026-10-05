# Level Core Rules

This crate owns logical level decoding, structural restrictions, deterministic
evaluation, metrics, constraints, and rating above the language core.

Dependencies point to IR, model, and VM only; serde is transport-neutral JSON
support. Accept only verified IR. Never compile source or access files, clocks,
network, host randomness, editor APIs, or platform bindings. Keep scene policy
out of language crates. Hidden test details must not enter public results.
The level specifications and recorded decisions define behavior; report genuine
gaps before choosing semantics. Validate generated instructions after commit.

The `scenes` module owns the selected product scene catalog and evaluation
family metadata. Planned catalog entries are not executable registrations and
must not be advertised as supported host capabilities.

The `scene_protocol` module owns validated scene construction, actor framing,
action ordering, shared outcomes, and scene counters. It delegates
scene-specific actions through the private object-safe `scene_runtime` boundary;
`scene_world` owns the built-in worlds and their transition details. This trait
object is an in-process implementation seam only: it is not a package loader,
guest ABI, or custom-scene execution capability. These modules do not advance
the VM or apply host budgets. Evaluators must stage successful world/input
candidates before checking and publishing resource-bound transitions. Host
adapters must not reproduce these rules, and protocol transitions alone do not
establish executable capabilities.

The `scene_session` module owns one continuous VM per validated test case,
committed output buffering, generated-code guards, independent VM/scene work
limits, and atomic scene/input publication. Reuse `validate_program_rules` and
existing metric helpers; do not duplicate structural capability policy. Result
data in this internal Rust case API is trusted and must pass the evaluation
layer's visible/hidden projection before reaching a host response.

The `scene_evaluate` module owns selected-case order, per-evaluation work
ceilings, visible-only metric aggregation/scoring, public case projection, and
hidden coarse failures. The `scene_feedback` module defines typed allowlisted
events, one shared event serialization, atomic staging, and acknowledgement
cursors. Host adapters may wrap these payloads but must not reimplement their
scene content, error identity, or cursor semantics. Actual event production must
be reserved together with scene/input publication before host registration.

Retain each published event's immutable encoded body and bill its staging work.
The shared API copies these bodies when paging; it must not re-encode scene
payloads on replay. Prepared typed page copies reserve their peak scene units
before cloning and change cursors only on successful commit.
