# `hw` Dialect: Current Shortcomings and Remediation

This document records what the current `hw` dialect can and cannot express
today. It is intentionally separate from [`hw_dialect.md`](hw_dialect.md),
which describes the intended semantic contract.

## 1. The abstraction boundary is structural, not yet executable

The dialect provides useful structural vocabulary: modules, instances, wires,
aggregate types, aliases, enums, parameters, and hierarchical paths. It does
not yet provide enough executable semantics to serve as a complete RTL or
netlist interchange format.

In particular, the IR does not currently model:

- clock and reset domains;
- registers, latches, or explicit state transitions;
- combinational process boundaries or scheduling;
- port direction and output signatures on `hw.module`;
- resolved multi-driver nets and tri-state behavior for `hw.inout`;
- instance interface lookup and port compatibility;
- target-independent timing, latency, or resource constraints; or
- a formal distinction between a signal, a storage location, and an ordinary
  SSA value beyond documentation.

This limits the design to a structural substrate. A compiler can describe
topology and some type shape, but cannot yet express a complete synchronous
hardware component without relying on future `seq` operations and external
conventions.

### How to address it

Add explicit semantic layers rather than placing all behavior in attributes:

1. Introduce clock/reset and state operations in `seq`, with transition,
   enable, reset priority, and edge semantics.
2. Add a module signature model that records named input/output/inout ports and
   validates instances against referenced module symbols.
3. Define a connectivity model for drivers, aliases, inout resolution, and
   wire identity.
4. Add interfaces for width, latency, clocking, side effects, and hierarchy so
   generic passes do not special-case every operation.

## 2. Most semantic invariants are documentation-only

The operation declarations predominantly use the generated `succ` verifier.
Structural interfaces enforce counts where declared, but dialect-specific
relationships are not generally checked. Examples include:

- `hw.bitcast` equal-width requirements;
- `hw.concat` result width and operand legality;
- `hw.slice` bounds and result width;
- array element and index type compatibility;
- struct field existence and field type compatibility;
- union tag existence and variant type compatibility;
- enum encodings fitting the underlying width and having unique names; and
- parameter references resolving to declarations of compatible type.

As a result, constructors can create IR that looks structurally plausible but
does not satisfy the semantic contract in the main specification.

### How to address it

Implement operation and type verifiers in increasing dependency order:

1. Validate local shape and type identity.
2. Validate aggregate field/index/tag relationships.
3. Validate module signatures and symbol references.
4. Add whole-module checks for driver uniqueness, output termination, cycles,
   and hierarchy resolution.

Each verifier should have a corresponding negative test. Constructors should
remain convenient, but verification must be the authority for legality.

## 3. Aggregate layout is under-specified and not queryable uniformly

Arrays, structs, and unions exist as types, but the dialect does not expose a
common layout interface. The documentation describes packed widths and field
ordering, yet generic code cannot reliably ask every hardware type for total
bitwidth, alignment, field offset, or flattening order.

This limits lowering, serialization, ABI generation, and equivalence checking:
each pass must know the concrete type class and duplicate layout rules.

### How to address it

Define a layout interface with operations such as:

- total packed width;
- element or field count;
- field/element type;
- field offset and width;
- packed ordering convention; and
- whether the type is a value, aggregate, resource, or reference.

Then make `hw.bitcast`, aggregate operations, and lowerings consume that
interface. Keep layout policy explicit and test it with golden bit-position
examples.

## 4. Hierarchy and symbols are not connected to interfaces

`hw.instance` stores module and instance names as attributes, and
`hw.module_extern` stores a symbol name, but the current API does not resolve
those names to module signatures. The IR can therefore represent an instance
whose input count, output count, or types disagree with its target.

Hierarchical paths are also stored as strings. They are useful for preserving
intent, but they are not yet checked against the actual instance graph and are
not robust under renaming or hierarchy transformations.

### How to address it

Use symbol references for module identity and typed port mappings for instance
connections. Add a symbol-table verification pass that resolves internal and
external module declarations. Represent hierarchy paths as structured symbol
references, retaining a printable form only as a derived view.

## 5. Inout and multiple-driver behavior is missing

`hw.inout<T>` identifies a bidirectional port shape, but there is no operation
for driving, reading, enabling, or resolving it. The dialect therefore cannot
distinguish a legal tri-state connection from an accidental multiple driver.

### How to address it

Define explicit driver/reader operations or a connectivity interface with
driver ownership and resolution semantics. State whether resolution is
four-state, wired-OR, wired-AND, target-defined, or illegal. Add verification
for driver count and direction, and keep technology-specific cells in a lower
dialect.

## 6. Parameters are textual rather than semantic

`hw.param_decl` and `hw.param_value` retain parameter type and value as string
attributes. The current design does not parse expressions, establish scopes,
evaluate dependencies, or validate that a use refers to a declaration.

This limits specialization, constant propagation, width inference, and
reproducible lowering. Text substitution is also insufficient for expressions
whose syntax or meaning varies by target.

### How to address it

Introduce a typed parameter expression model with literal, reference, unary,
binary, and conditional forms. Define evaluation scope, constant requirements,
overflow behavior, and diagnostics. Specialization should produce explicit
parameter bindings rather than rewriting arbitrary strings.

## 7. No transformation or lowering contract is implemented

The dialect has constructors and printers but no verified lowering pipeline for
flattening hierarchy, resolving aggregates, mapping instances, or emitting an
HDL/netlist representation. There are also no canonicalization rules for
redundant wires, identity bitcasts, or aggregate round trips.

This limits the practical capacity of the IR: it can record a design, but
there is no implementation-defined path that preserves its semantics into a
backend.

### How to address it

Document and implement staged conversions:

```text
hw structural IR -> typed RTL/seq+comb IR -> flattened structural IR
                  -> Boolean/netlist IR -> HDL, cells, or target backend
```

For every conversion, list preserved identity, layout, timing, reset, and
protocol information. Add round-trip or golden-output tests at each boundary.

## 8. Testing currently favors construction over rejection and behavior

The existing tests verify that constructors return requested types and that
well-formed operations can be inserted into modules. They do not yet establish
that malformed widths, field names, symbol references, driver graphs, or
parameter expressions are rejected. There is also no evaluator or backend
against which bit-level behavior can be compared.

The expanded hardware tests should therefore be understood as API and IR
shape coverage, not proof that all semantic contracts are enforced.

### How to address it

Grow testing in four layers:

1. Unit tests for type accessors, layout, attributes, and printed syntax.
2. Negative verifier tests for every illegal relationship.
3. Module-level tests for symbols, instance compatibility, drivers, and graph
   cycles.
4. Golden lowering and simulation/equivalence tests once those facilities
   exist.

## 9. Recommended implementation order

1. Add module signatures and symbol resolution.
2. Add shared layout and hardware-value interfaces.
3. Implement local type and operation verifiers.
4. Define inout and driver semantics.
5. Replace textual parameters with typed expressions.
6. Add `seq` state/clock/reset semantics and connect them to `hw` modules.
7. Implement the first typed lowering and its preservation tests.
