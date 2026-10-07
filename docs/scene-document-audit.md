# Scene Document Audit

## Current contract (2026-10-07)

The user removed Baudot, QualityControl, and Elevator from Rust. The executable
catalog is ExactIO, Robot, MechanicalArm. Paper-tape and quality authoring
precompile input and expected bytes into ExactIO; their presentation metadata
stays in the application. Elevator drafts are preserved only for recovery.

The author contract retains format_version 1 and host API/ABI 2. Old removed
scene identifiers return UnsupportedSceneType. ExactIO accepts arbitrary u8
input and output vectors, without five-bit, quality item, or batch restrictions.
Published error identities remain reserved rather than being reassigned.

Robot map/state rules, MechanicalArm transformations, deterministic work
accounting, hidden-case privacy, and feedback ownership remain normative.
Current executable fixtures cover only the registered catalog.
