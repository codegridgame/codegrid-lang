# Scene API 2 Execution Fixtures

These `format_version: 1` author levels and Full `.cg` programs exercise
ExactIO, Robot, and MechanicalArm through Level API 2. The API version is
separate from the author JSON format version. The manifest supplies explicit
seeds, boundaries, and expected terminal status. MechanicalArm inspects and
packs an input robot; Robot reaches its required patrol. Additional ExactIO
fixtures cover byte boundaries and a fixed classification mapping. Robot
configuration lives in `scene_config`; each test contains only `visible`.

Use the shared Rust compiler/evaluator for acceptance and execution. Host
harnesses transport original files and compare complete permitted results;
they do not implement scene semantics. Passing this initial manifest does not
prove all protocol vectors or production integration.

The Robot observation fixture consumes initial input, performs a nonterminal
move, then reads the newly appended position to choose the next action. Its
small-slice variant must preserve the full result and event trace.

The Robot halt/constraint fixture emits WAIT and HALT from separate threads in
the same committed tick. With an unfinished patrol and a breached tick
constraint, ConstraintExceeded must take precedence over IncompleteGoal.
