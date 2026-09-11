# Pliron Constraints and Hardware IR Design

## Purpose

This document records the practical constraints discovered while defining the
`hw`, `comb`, `seq`, and `sv` dialects. These are not merely Rust API details.
Each constraint affects what can be represented faithfully, where a semantic
invariant belongs, and which transformations are safe.

The central rule is:

> Treat pliron's generic IR mechanisms as representation machinery, not as a
> substitute for the hardware dialect's semantic contract.

A constructor can build an operation. Only verification establishes whether
that operation is legal hardware IR.

## 1. Dialect registration is part of the semantic environment

Operations and types must be registered in the `Context` before they can be
constructed, parsed, or verified. The repository therefore has one explicit
registration path:

```rust
register_all(&mut ctx);
```

That call installs `hw`, `comb`, `seq`, and `sv`. A test or transformation
that creates a fresh context but omits registration is not exercising the same
IR environment as the compiler.

### Hardware consequence

Type identity and operation identity are context-owned. Do not compare or
serialize a hardware handle as if its meaning were independent of the context.
Pass the context through builders, verifiers, printers, and transformation
passes.

## 2. Operation names have identifier rules

Pliron operation names are identifiers with a dialect separator. Names such as
`sv.assign` and `sv.always_ff` are valid. A nested spelling such as
`seq.hlmem.read` is rejected by the identifier parser in the current pliron
version, so the implemented memory operations use `seq.hlmem_read` and
`seq.hlmem_write`.

### Hardware consequence

The textual spelling of an operation is constrained by the host IR. Do not
allow HDL-inspired names to drive the semantic decomposition. If a concept
needs a hierarchy, represent that hierarchy with operations, attributes, or
regions and choose a legal operation name. The name must not be mistaken for
the semantic boundary.

## 3. Attribute dictionary keys are globally unique

The current context rejects duplicate attribute dictionary keys across
operation definitions. Two operations cannot independently declare a generated
attribute named `target`, `is_async_reset`, or `reset_polarity` without causing
a registration panic. The SV dialect therefore uses:

- `assign_target` on `sv.assign`;
- `ff_target` on `sv.always_ff`;
- `ff_async_reset` on `sv.always_ff`; and
- `ff_reset_polarity` on `sv.always_ff`.

These names are implementation keys. Their semantic roles remain target name,
asynchronous-reset mode, and reset polarity.

### Hardware consequence

Attribute namespaces are not free-form HDL metadata. Attribute identity is
part of the dialect implementation. Use operation-specific prefixes when the
same concept appears on multiple operations, and document the semantic alias
explicitly. Never solve a key collision by silently dropping an attribute:
that would discard timing or reset information.

## 4. Derived interfaces and custom verifiers are complementary

Interfaces such as `NOpdsInterface`, `OneResultInterface`, and
`NRegionsInterface` verify structural facts:

- operand count;
- result count;
- region count;
- standard reusable properties.

A manual `Verify` implementation checks dialect semantics. For example,
`seq.compreg` additionally verifies that its first operand is `!seq.clock` and
that its input and result types match. `sv.always_ff` checks clock/reset types,
value compatibility, target presence, and reset polarity.

The operation verification path is therefore conceptually:

```text
attributes and values
        -> structural interfaces
        -> operation-specific verifier
        -> dominance and enclosing-IR checks
```

### Hardware consequence

Do not encode semantic legality only in constructors. A caller can construct
an intentionally malformed operation for diagnostics or transformation tests.
`verify_op` is the authority that determines whether the IR is legal.
Every semantic verifier should have at least one negative test.

## 5. `TypeHandle` is a typed identity handle, not a concrete Rust value

A `TypeHandle` points to a trait-object type stored in the context. Generic
comparisons are appropriate for type identity:

```rust
value.get_type(ctx) == expected_type
```

Concrete parameters require an explicit downcast:

```rust
let ty = handle.deref(ctx);
let memory = ty.downcast_ref::<MemoryType>().unwrap();
```

The dereferenced borrow must remain alive while the concrete reference is
used. Temporary chains often fail Rust's lifetime rules.

### Hardware consequence

Use type equality for compatibility and downcasting only for attributes that
are intrinsic to a concrete type, such as memory depth and element type. Do
not infer hardware width, signedness, clock identity, or storage semantics
from display strings.

## 6. Context borrowing shapes builder APIs

Most pliron builders need `&mut Context`, while values and type queries need
`&Context`. Rust therefore rejects expressions such as:

```rust
Op::new(&mut ctx, previous.result(&ctx), ty);
```

Materialize values before the mutable borrow:

```rust
let previous_value = previous.result(&ctx);
let op = Op::new(&mut ctx, previous_value, ty);
```

### Hardware consequence

This encourages a useful transformation discipline: resolve all source values,
types, and metadata before mutating the IR. A pass should collect its inputs,
validate preconditions, then create and insert new operations. Do not mutate a
module while still discovering the types that govern the mutation.

## 7. SSA values do not represent every hardware identity

A `Value` represents a typed SSA result or block argument. Hardware semantics
may also require identity for:

- a clock domain;
- a memory resource;
- a named wire;
- a module instance;
- an externally visible target;
- a signal with multiple-driver rules.

The `hw.wire`, `!seq.clock`, `!seq.mem`, symbol metadata, and SV target
attributes preserve distinctions that ordinary SSA use-def edges cannot carry.

### Hardware consequence

Never replace an identity-bearing object with an arbitrary equivalent SSA value
until the transformation has proved that identity is not observable. A
combinational value and a clock waveform may both be one-bit values in an HDL,
but they are not interchangeable in this IR.

## 8. Graph regions and ordinary SSA dominance differ

`hw.module` uses a graph region to model concurrent hardware. The module body
is not a software control-flow region. Operation order is useful for printing
and insertion, but it is not an execution schedule. Hardware meaning comes
from connectivity, clocks, state boundaries, and interfaces.

### Hardware consequence

Do not lower a graph-region order directly into sequential source order. A
transformation may topologically order combinational dependencies for emission,
but it must preserve concurrent meaning. `seq` operations are temporal
barriers; `sv.always_ff` is an emission form, not a license to infer software
control flow.

## 9. Attributes are metadata until the dialect gives them meaning

A `StringAttr` or `BoolAttr` is only a container. It does not automatically
validate polarity, latency, collision behavior, or target names. The seq and SV
verifiers explicitly restrict attribute vocabularies such as:

- `active_high` / `active_low`;
- `read-first` / `write-first` / `undefined`.

### Hardware consequence

Every semantic attribute needs a documented domain, default or required status,
verification rule, and lowering behavior. Free-form attributes are especially
dangerous for reset and memory semantics because a backend may silently choose
a different interpretation.

## 10. No automatic lowering or canonicalization is implied

Defining operations and registering them does not create a complete conversion
pass, canonicalizer, simulator, or SystemVerilog printer. The repository now
has typed SV lowering helpers and one rewriter-based canonicalization pattern,
but it still does not have a complete lowering driver or source emitter.

### Hardware consequence

A dialect must preserve enough information for future passes rather than
claiming that its current syntax is executable. In particular:

- `seq.firreg` retains reset mode and polarity until an SV lowering chooses an
  event-control spelling;
- `seq.hlmem` retains memory identity and collision policy until memory
  lowering;
- `sv` retains emission intent but is not itself emitted source.

The implemented helpers are intentionally narrow: they lower individual
verified values or register operations, and the canonicalizer removes only a
provably redundant assignment. A module-level pass must still decide naming,
ordering, declarations, memory legalization, and source emission.

## 11. What the current rules permit

The current design can express and verify:

- structural modules, ports, wires, instances, and aggregates;
- pure combinational operations with typed SSA values;
- clock and reset identities;
- reset-bearing state boundaries;
- abstract synchronous memories and local collision policies;
- named SV continuous assignments and reset-aware `always_ff` intent.

It can safely support local transformations that preserve these contracts,
such as replacing a combinational producer while retaining its result type or
lowering a verified resettable register to an SV operation with the same reset
attributes.

## 12. What the current rules do not prove

The current design does not yet provide complete proofs for:

- unique SV target names across a module;
- multiple-driver legality and resolution;
- clock-domain crossing safety;
- address range and memory-port conflict analysis;
- combinational-cycle rejection across all dialects;
- canonicalization equivalence;
- generated SystemVerilog syntax accepted by a simulator or synthesizer;
- timing closure, setup/hold margins, or physical clock-tree legality.

These require module-level analyses, transformation infrastructure, or an
external HDL tool. They should be added as explicit passes rather than hidden
inside local operation constructors.
