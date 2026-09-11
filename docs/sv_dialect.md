# SV Dialect

## Purpose

The `sv` dialect is the target-facing emission layer for SystemVerilog intent.
It is deliberately separate from `hw`, `comb`, and `seq`:

- `hw` preserves hierarchy, ports, wires, and structural identity.
- `comb` preserves pure zero-cycle computation.
- `seq` preserves clocks, state, reset priority, latency, and memory behavior.
- `sv` records how those verified meanings should be emitted as SystemVerilog.

This separation prevents a backend from using textual SystemVerilog syntax as
the source of truth for hardware semantics. SV operations should be created
after semantic analysis and lowering decisions have established what the
circuit means.

## Implemented operations

### `sv.logic_decl`

Declares a named `logic` object. Use it when a target variable must exist
independently of a particular assignment process. Its result carries the
declared type and `logic_target` carries the emitted name.

### `sv.always_comb`

Represents a procedural combinational assignment with a `comb_target` and one
source value. Use it when the target is intentionally emitted as an
`always_comb` process rather than a continuous assignment:

```systemverilog
always_comb begin
  result = expression;
end
```

It does not introduce state and cannot replace `seq` operations.

### `sv.instance`

Represents a SystemVerilog module instance. `sv_instance_module` identifies
the referenced module, `sv_instance_name` identifies the instance, and
operands preserve input connection order:

```systemverilog
child_module u_child (input_a, input_b);
```

Hierarchy and symbol resolution still originate in `hw`; this operation is
the emission form after those structural contracts have been checked.

### `sv.mem_decl`

Declares a `!seq.mem` resource as a SystemVerilog unpacked memory array. The
`memory_target` is the emitted name, while depth and element type remain in
the `!seq.mem` type:

```systemverilog
logic [31:0] storage [0:255];
```

Read/write timing and collision behavior remain the responsibility of the
sequential memory operations and their lowering pass.

### `sv.assign`

`sv.assign` represents a named continuous assignment:

```text
next_value = sv.assign input {target = "next_value"}
```

Use it when a value is combinational and should be emitted as a named net or
continuous assignment. The operation has one operand, one same-typed result,
and a non-empty `target` attribute.

Typical emission:

```systemverilog
assign next_value = input;
```

The operation does not introduce state, scheduling, or a clock. Stateful
behavior belongs in `seq` and is represented by `sv.always_ff` only after the
state contract is known.

### `sv.always_ff`

`sv.always_ff` represents a reset-aware nonblocking sequential assignment. Its
operands are `(clock, input, reset, reset_value)`, and its attributes are:

- `target`: non-empty emitted variable name;
- `is_async_reset`: whether reset is part of the event control;
- `reset_polarity`: `active_high` or `active_low`.

Use it to lower `seq.firreg` while preserving reset mode and polarity:

```systemverilog
always_ff @(posedge clk) begin
  if (reset)
    state <= reset_value;
  else
    state <= input;
end
```

For asynchronous active-low reset, the target shape is:

```systemverilog
always_ff @(posedge clk or negedge reset_n) begin
  if (!reset_n)
    state <= reset_value;
  else
    state <= input;
end
```

The SV verifier requires a `!seq.clock`, a `!seq.reset`, matching input and
reset-value types, a non-empty target, and a supported reset polarity. It does
not decide whether a reset is electrically safe or whether a clock domain is
properly synchronized; those are design-level analyses.

## Lowering relationship

The intended direction is:

```text
hw + comb + seq
          |
          v
         sv
          |
          v
    SystemVerilog source
```

`sv` is not a replacement for the semantic dialects. A lowering must preserve:

- register cycle latency;
- reset priority and polarity;
- synchronous versus asynchronous reset behavior;
- continuous versus sequential assignment kind;
- target names and structural identity where externally observable.

The current implementation provides emission operations, local verifiers, a
deterministic printer for the implemented operation surface, lowering helpers,
an executable `lower_module_registers` conversion pass for sequential
registers, and a redundant-assignment canonicalization helper. A complete
conversion pass for every hardware operation, broader canonicalization
framework, and complete SystemVerilog AST remain future extensions.

Detailed transformation rules are documented in
[`sv_transformations.md`](sv_transformations.md). The practical constraints
that shape these operations, including pliron's global attribute-key and
operation-name rules, are recorded in
[`pliron_hardware_ir_constraints.md`](pliron_hardware_ir_constraints.md).

## Verification boundary

Local SV verification rejects malformed operation contracts. It cannot prove:

- that target names are unique across a complete module;
- that a target is declared as a legal `logic`, `wire`, or variable;
- that an `always_ff` target has no conflicting procedural drivers;
- that a clock domain crossing is synchronized;
- that emitted SystemVerilog is accepted by every simulator or synthesis tool.

Those checks belong to symbol-table, driver, clock-domain, and emitted-source
validation passes.

## Implementation attribute keys

The semantic target name is represented by operation-specific keys because the
current pliron context requires attribute dictionary keys to be globally
unique:

- `sv.assign` uses `assign_target`;
- `sv.always_ff` uses `ff_target`, `ff_async_reset`, and
  `ff_reset_polarity`.

These names are storage keys, not a change in the conceptual meaning of the
attributes described above.