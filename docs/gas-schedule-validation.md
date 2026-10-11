# Gas Schedule 1 validation

The 2026-10-11 decision uses complete operation base fees and virtual memory/
stack capacity fees. These are deterministic game-language resource prices,
not measurements of CPU time, allocator bytes, or Ethereum fee equivalence.

The following independent arithmetic was compared with production native CLI
results. Explicit Entry and Halt have zero Gas. Size still counts both as
nonempty static cells; they do not share Gas weights.

| Program work | Execution Gas | Memory Gas | Stack Gas | Total |
| --- | ---: | ---: | ---: | ---: |
| READ, OUTPUT | 10 | 0 | 0 | 10 |
| READ, ADD, OUTPUT | 13 | 0 | 0 | 13 |
| SUB, OUTPUT | 8 | 0 | 0 | 8 |
| ADD twice through Repeat | 6 | 0 | 0 | 6 |
| Direction, register-pointer movement | 3 | 0 | 0 | 3 |
| PUSH, POPADD | 6 | 0 | 1 | 7 |
| Empty READ | 5 | 0 | 0 | 5 |
| LOAD, LOAD, STORE same address | 18 | 10 | 2 | 30 |

The exact-limit and rollback fixture uses ADD then OUTPUT. Limit 8 accepts the
complete program; limit 7 rejects the OUTPUT Tick with attempted total 8,
retaining the previously committed ADD and publishing no output. Repeated
terminal requests do not add Gas. Shared-address fixtures charge one cold fee
across repeated accesses; per-test VMs reset the cold and capacity ledgers.

Focused tests also cover visible-case Gas/breakdown sums, hidden hard limits,
static Size once, explicit Entry, unused Functions, Folded reference/body cells,
step/run equivalence, work interruption, checked overflow, and disabled Custom
through ordinary source and verified IR. The shared Full suite includes exact
Gas breakdown expectations for every case, with dedicated Gas limit vectors.

These checks support internal coherence of Schedule 1. They do not establish
balanced difficulty across a published level catalog or an empirical physical
resource model. All existing author constraints/targets now refer to the
current Gas/Size contract, without treating former Cost targets as equivalent.
