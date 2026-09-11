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

The current implementation provides emission operations and local verifiers,
but not yet a conversion pass, canonicalization patterns, or a SystemVerilog
printer. Those are the next implementation boundary.

## Verification boundary

Local SV verification rejects malformed operation contracts. It cannot prove:

- that target names are unique across a complete module;
- that a target is declared as a legal `logic`, `wire`, or variable;
- that an `always_ff` target has no conflicting procedural drivers;
- that a clock domain crossing is synchronized;
- that emitted SystemVerilog is accepted by every simulator or synthesis tool.

Those checks belong to symbol-table, driver, clock-domain, and emitted-source
validation passes.