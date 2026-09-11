# SV Transformations and Dialect Boundaries

## 1. Transformation model

The intended compilation direction is:

```text
source / algorithm
        |
        v
hw hierarchy + comb dataflow + seq state
        |
        v
verified SV emission intent
        |
        v
SystemVerilog source
```

`sv` is a target-facing representation. It should be introduced after
semantic operations have been verified and after transformations that depend
on higher-level information have completed. Lowering to SV is therefore not a
mere rename of operations.

A legal transformation must state:

1. which semantic information it consumes;
2. which information it preserves;
3. which information becomes target-specific;
4. which invariants it proves before creating SV operations; and
5. whether the transformation is reversible.

## 2. `comb` to `sv.assign`

A pure combinational value may be emitted as a continuous assignment when it
has a stable target name and its type is legal for the target backend.
Conceptually:

```text
%next = comb.add %a, %b : i8
%named = sv.assign %next {assign_target = "next_value"} : i8
```

The transformation must preserve:

- the operand/result type;
- zero-cycle combinational semantics;
- bit width, extension, truncation, and signedness behavior;
- data dependencies.

It must not turn a stateful or clock-bearing value into an assignment. For
example, a `!seq.clock` value is not ordinary combinational data even if its
carrier representation is one bit.

Typical output:

```systemverilog
assign next_value = a + b;
```

The current `sv.assign` operation records one named right-hand-side value. It
does not yet encode a full SV expression tree or emit source text itself. An
emitter must either retain the source operation that produced the value or
have a separate expression printer.

## 3. `seq.firreg` to `sv.always_ff`

A verified resettable register lowers to `sv.always_ff` by transferring its
clock, input, reset, reset value, target identity, asynchronous mode, and reset
polarity:

```text
seq.firreg
  clock        -> sv.always_ff clock
  input        -> sv.always_ff input
  reset        -> sv.always_ff reset
  reset_value  -> sv.always_ff reset_value
  async mode   -> sv.ff_async_reset
  polarity     -> sv.ff_reset_polarity
  result name  -> sv.ff_target
```

For synchronous active-high reset:

```systemverilog
always_ff @(posedge clk) begin
  if (reset)
    state <= reset_value;
  else
    state <= next_value;
end
```

For asynchronous active-low reset:

```systemverilog
always_ff @(posedge clk or negedge reset_n) begin
  if (!reset_n)
    state <= reset_value;
  else
    state <= next_value;
end
```

The lowering must not:

- invert reset without changing the polarity contract;
- move reset below an enable condition;
- replace nonblocking state update with a continuous assignment;
- erase the one-cycle state boundary;
- select an event control inconsistent with `ff_async_reset`.

The current SV operation requires reset operands even when a source operation
such as `seq.compreg` has no reset. A future lowering may add an explicit
no-reset SV form or lower `seq.compreg` through a separate operation. It must
not invent a reset signal merely to satisfy an interface.

## 4. `seq.compreg` to SV

`seq.compreg` semantically represents a plain D register:

```systemverilog
always_ff @(posedge clk) begin
  state <= next_value;
end
```

The implementation provides `sv.always_ff_no_reset` for this exact case. The
lowering helper `sv::lowering::lower_compreg` creates that operation and keeps
the source clock and input values intact. It does not add a reset signal.

## 5. Memory transformations

`seq.hlmem` preserves a memory resource, depth, element type, and
read-during-write policy. A future SV memory lowering should produce a
SystemVerilog declaration plus one or more read/write processes:

```systemverilog
logic [7:0] mem [0:15];

always_ff @(posedge clk) begin
  if (write_enable)
    mem[write_address] <= write_data;
  read_data <= mem[read_address];
end
```

The transformation must preserve:

- depth and addressability;
- element width and aggregate representation;
- synchronous read latency;
- write enable timing;
- `read-first`, `write-first`, or `undefined` collision behavior;
- port count and ordering.

A raw array declaration is not enough to preserve these semantics. The emitter
must choose process ordering or explicit bypass logic appropriate to the
collision policy. If the target cannot implement the requested policy, the
conversion must fail or require an explicit target-specific legalization pass.

## 6. `hw` hierarchy to SV modules and instances

`hw.module` is the source of structural hierarchy. A module lowering should
map:

- module symbol -> SystemVerilog module name;
- input block arguments -> input ports;
- output terminator operands -> output ports;
- `hw.instance` -> module instance;
- `hw.wire` -> named net or variable declaration;
- `hw.module_extern` -> external module declaration or black-box reference.

The graph region must be topologically ordered for emitted declarations and
assignments, but that ordering is presentation, not sequential execution.

A module-level lowering pass must also build a symbol table. The current
operation-local verifiers do not prove that SV target names are unique or that
all instance references resolve.

## 7. Transformations that are not semantics-preserving by default

### SV back to `seq` or `comb`

Lowering arbitrary SystemVerilog back into semantic IR is not generally
lossless. Source may contain:

- blocking and nonblocking assignments;
- multiple procedural drivers;
- event controls and delays;
- four-state values;
- unsynthesizable constructs;
- implicit nets and elaboration-time behavior;
- tool-specific extensions.

A reverse transformation needs a restricted SV subset and explicit legality
checks. `sv.always_ff` can map back to `seq.firreg` when its clock, reset,
polarity, target, and assignment shape meet the seq contract. A general
`always` block cannot be treated as a register automatically.

### `seq` to `comb`

This is illegal for stateful operations unless the transformation is an
explicit state-elimination or formal abstraction pass. Removing a register
changes latency and recurrence semantics.

### `comb` across `seq`

Combinational rewrites may occur on either side of a register boundary, but
hoisting or sinking logic across that boundary changes cycle latency unless
retiming is explicitly enabled and proven equivalent.

## 8. Canonicalization rules for SV intent

Canonicalization must preserve observable emission semantics:

- Do not merge assignments with different target names if names are externally
  observable.
- Do not merge sequential assignments with different clocks or reset policy.
- Do not reorder `always_ff` blocks when driver legality or source ordering is
  part of the target contract.
- Remove redundant `sv.assign` only when its result has no uses and its target
  is not externally required.
- Normalize reset polarity only together with the reset signal and emitted
  event-control polarity.

The implementation provides `sv::canonicalization::eliminate_redundant_assign`.
It recognizes an assignment whose target is already the source value name,
replaces all result uses with the source value through pliron's `Rewriter`,
and erases the redundant operation. Other rules in this section remain
specification-level constraints for future canonicalizers.

## 9. Verification after transformation

Every lowering should run at least these checks:

1. Verify the source operation before conversion.
2. Check the destination operation's local verifier.
3. Verify the enclosing module after insertion.
4. Run module-level name, driver, hierarchy, and clock-domain analyses.
5. Compare preserved properties such as type, latency, reset mode, and memory
   collision policy.

A successful constructor call is not evidence that a conversion is legal.
The conversion must call `verify_op` on the resulting IR and report a
conversion error when a target contract cannot be represented.

## 10. Current implementation boundary

Implemented today:

- local `sv.assign` and `sv.always_ff` operation construction;
- reset-free `sv.always_ff_no_reset` for plain D-register emission;
- local type, target-name, and reset-policy verification;
- deterministic source rendering for the implemented SV operations;
- typed lowering helpers for `comb` values, `seq.compreg`, and `seq.firreg`;
- redundant-assignment canonicalization through `Rewriter`;
- module-local validation for SV targets, instance names, clock/reset
  assignment boundaries, and same-address memory write conflicts;
- documentation of semantic source-to-target mappings.

Not implemented yet:

- a module-wide conversion driver from `hw`/`comb`/`seq` to `sv` that walks
  every source operation and inserts the generated operations;
- full SV expression, declaration, module, and instance operations;
- memory lowering with collision-policy legalization;
- cross-module symbol resolution and complete driver analysis;
- clock-domain crossing analysis and full memory-port conflict analysis;
- broader canonicalization patterns and equivalence tests.

These omissions are intentional and should be tracked as compiler passes, not
hidden by adding more free-form attributes to the current operations.
