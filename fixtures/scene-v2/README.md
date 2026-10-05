# Scene API 2 Execution Fixtures

These `format_version: 1` author levels and Full `.cg` programs exercise all six
scenes through Level API 2. API 2 is the host protocol version; it is separate
from the author JSON format version. The manifest supplies explicit seeds,
boundaries, and expected terminal status. MechanicalArm includes Inspection
and Packing before output; Elevator boards and delivers a passenger; Robot
reaches its required patrol; Baudot emits both five-bit boundary values used by
the author example. Robot's static terrain, colors, starts, and objects live in
`scene_config`; each test contains only `visible`.

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
