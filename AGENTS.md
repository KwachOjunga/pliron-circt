# AGENTS.md — Hardware Dialect Definition and Documentation Guide

## Purpose

This document defines the engineering rules for designing, implementing, reviewing, and documenting hardware-oriented IR dialects.

The goal is not to prescribe one hardware IR architecture. The goal is to ensure that every dialect has:

- a clearly defined semantic contract;
- an explicit abstraction level;
- well-defined legality and invariants;
- useful composition with other dialects;
- analyzable operations and types;
- predictable lowering behavior;
- enough information for verification, transformation, simulation, synthesis, and code generation where applicable;
- documentation that explains the *meaning* of the IR rather than merely listing its syntax.

Hardware IR is particularly sensitive to underspecified semantics. A representation that looks structurally plausible can nevertheless be wrong if it loses information about time, state, connectivity, bit width, signedness, reset behavior, clocking, latency, ordering, resource constraints, or event semantics.

---

# 1. Core Design Principle

A dialect is a semantic contract, not merely a collection of operations.

Before adding an operation, type, attribute, or region, answer:

1. What hardware concept does this construct represent?
2. At what abstraction level does it operate?
3. What are its inputs and outputs?
4. What are its temporal semantics?
5. What state does it contain?
6. What assumptions does it make about clocks, resets, events, or scheduling?
7. What values are observable?
8. What transformations preserve its meaning?
9. What invariants must always hold?
10. Which lower-level representations can faithfully implement it?
11. Which higher-level representations can legally lower into it?
12. What information must not be discarded during lowering?

If these questions cannot be answered precisely, the construct is not ready to be added to the dialect.

---

# 2. Establish the Abstraction Level

Every dialect must explicitly state what it models.

Possible levels include:

- behavioral HDL;
- process/event semantics;
- dataflow;
- RTL;
- structural netlist;
- Boolean/gate-level logic;
- arithmetic/data-path structures;
- memory systems;
- interfaces/protocols;
- physical implementation constraints;
- verification properties;
- simulation semantics;
- target-specific hardware.

Do not mix abstraction levels accidentally.

For example:

- An RTL dialect may represent registers and combinational logic without representing transistor behavior.
- An event-driven dialect may represent delayed signal updates without claiming that delay is synthesizable.
- A structural dialect may represent instances and connections without introducing procedural semantics.
- A protocol dialect may represent valid/ready behavior without pretending that a transaction is equivalent to a clock-cycle-independent function call.

If a dialect intentionally spans multiple levels, document the boundary explicitly.

## Abstraction-level test

For every operation, state:

> "This operation means X at abstraction level Y."

Then state what it does **not** mean.

This prevents accidental semantic inflation.

---

# 3. Separate Structural, Behavioral, Temporal, and Physical Semantics

Hardware constructs often combine several dimensions.

Document them separately.

### Structural semantics

Describe:

- hierarchy;
- instances;
- ports;
- connectivity;
- ownership;
- named resources;
- module composition.

### Behavioral semantics

Describe:

- input/output relationship;
- state transition function;
- combinational function;
- control flow;
- memory behavior.

### Temporal semantics

Describe:

- clocks;
- edges;
- cycles;
- delays;
- event queues;
- ordering;
- latency;
- initiation interval;
- synchronization;
- concurrent execution.

### Physical semantics

Only include physical information when the dialect is intended to represent it.

Examples:

- area;
- timing constraints;
- placement;
- routing;
- technology cells;
- voltage domains;
- physical pins.

Do not imply physical meaning from a purely logical construct unless that meaning is part of the specification.

---

# 4. Define the Semantic Model Before the Syntax

For each dialect, write a short semantic model before writing operation definitions.

A useful model should identify:

- values;
- state;
- events;
- time;
- resources;
- hierarchy;
- communication;
- control;
- memory;
- external effects.

For sequential hardware, describe the state-transition relation explicitly where practical:

```text
S(t+1) = F(S(t), I(t))
O(t)   = G(S(t), I(t))
```

For combinational hardware:

```text
O = F(I)
```

For event-driven semantics, document:

```text
(event, time, state) -> scheduled updates -> new event/state
```

For handshake/dataflow semantics, describe:

```text
availability + acceptance -> transaction
```

The notation is not mandatory. The important requirement is that the semantics be explicit.

---

# 5. Types Are Semantic Contracts

A hardware type must encode all information necessary to interpret the value correctly.

Consider explicitly:

- bit width;
- signedness;
- 2-state vs 4-state vs multi-state values;
- aggregate structure;
- packed vs unpacked representation;
- arrays;
- vectors;
- channels;
- tokens;
- references;
- clocks;
- reset signals;
- memories;
- opaque/external types;
- physical or target-specific information.

Do not use a generic integer type when width or signedness changes semantics.

For every type document:

1. What values can inhabit it?
2. What operations are legal?
3. What conversions exist?
4. Is width part of the type identity?
5. Is signedness part of the type identity?
6. Does the type represent a value, storage, a signal, a reference, or a resource?
7. Can it be materialized as an SSA value?
8. Does it represent instantaneous data or something with temporal behavior?

## Value vs signal vs storage

Do not conflate:

- a value;
- a signal carrying values over time;
- a storage element retaining state;
- an address/reference to storage;
- a connection between producers and consumers.

These have different semantics.

---

# 6. SSA Does Not Automatically Give Hardware Its Meaning

SSA is a representation mechanism, not a complete hardware semantics.

An SSA value normally represents a computed value, but hardware IR may additionally need to represent:

- persistent state;
- multiple drivers;
- event-triggered updates;
- clock domains;
- feedback;
- connectivity;
- physical identity;
- latency;
- transactions.

Never assume that ordinary SSA dataflow alone captures these properties.

If a construct has non-SSA semantics, explain how those semantics interact with SSA.

In particular, document whether:

- a value denotes a snapshot;
- a value denotes a continuously driven signal;
- a result represents a register output;
- an operation creates an identity-bearing object;
- multiple operations may refer to the same hardware object.

---

# 7. Hardware State Must Be Explicit

For every stateful operation document:

- what state exists;
- when state is read;
- when state is written;
- what causes a state transition;
- what clock/event controls it;
- reset behavior;
- initialization behavior;
- enable behavior;
- whether reset is synchronous or asynchronous;
- whether reset dominates enable;
- what happens when control inputs are unknown, if unknown states exist.

A register-like operation should not merely be documented as "stores a value."

Define its transition semantics.

Example:

```text
At an active clock edge:

if reset:
    state := reset_value
else if enable:
    state := input
else:
    state := previous_state
```

The exact priority must be part of the operation contract.

---

# 8. Clock Semantics

Clock handling deserves explicit treatment.

Document:

- clock identity;
- edge polarity;
- level sensitivity;
- clock domain;
- derived clocks;
- clock enables;
- gating;
- whether clocks are ordinary values or special resources;
- whether clock crossings are legal;
- synchronization requirements.

Avoid representing clocks as ordinary `i1` values unless the semantics make this intentional and sufficient.

A boolean waveform and a clock domain are not necessarily equivalent concepts.

---

# 9. Reset Semantics

For every reset-bearing construct specify:

- synchronous/asynchronous;
- active-high/active-low;
- reset value;
- reset priority;
- release semantics;
- interaction with enable;
- whether reset is required or optional;
- whether reset is part of the type, operation, or attribute.

Do not hide reset semantics in arbitrary attributes without a clear reason.

Two operations that look identical but differ in reset priority are semantically different.

---

# 10. Time and Events

If a dialect represents simulation or event-driven behavior, time must be explicit.

Document:

- time units;
- time precision;
- delay semantics;
- delta cycles;
- event ordering;
- scheduling priority;
- simultaneous events;
- races;
- blocking/non-blocking behavior;
- event cancellation;
- causality.

Do not use an integer attribute named `delay` without specifying its unit and scheduling semantics.

Distinguish:

- latency;
- physical propagation delay;
- simulation delay;
- cycle count;
- initiation interval.

These are not interchangeable.

---

# 11. Combinational Semantics

Combinational operations should make their mathematical behavior clear.

Document:

- output function;
- bit-width behavior;
- overflow;
- truncation;
- extension;
- signedness;
- unknown/X/Z behavior where relevant;
- whether the operation is pure;
- whether it can be speculated;
- whether it has side effects.

For example, distinguish:

```text
add: mathematical addition followed by width semantics
```

from:

```text
adder: a physical or structural adder implementation
```

The first expresses behavior; the second may express implementation.

Do not encode implementation-specific assumptions in behavioral operations unless required.

---

# 12. Sequential Semantics

Sequential operations should define:

- state elements;
- state transition function;
- output function;
- timing;
- control priority;
- initialization;
- reset;
- enable;
- latency.

If an operation is pipelined, document:

- latency;
- initiation interval;
- whether latency is fixed;
- whether bubbles are possible;
- whether backpressure exists;
- whether operations may overlap.

A "pipeline" attribute without a precise interpretation is insufficient.

---

# 13. Connectivity and Drivers

Hardware connectivity differs from ordinary SSA use-def chains.

Document:

- whether a value has exactly one driver;
- whether multiple drivers are legal;
- how multiple drivers resolve;
- whether connections create aliases or copies;
- whether a wire has identity;
- whether names are semantically significant;
- whether a connection is directional;
- whether inout/bidirectional ports are supported.

Never silently collapse distinct hardware objects into equivalent SSA values if hardware identity matters.

---

# 14. Hierarchy

Hierarchy is often semantically important.

Document:

- module definitions;
- module instances;
- instance ownership;
- port mapping;
- parameterization;
- visibility;
- external modules;
- recursive hierarchy;
- symbol identity;
- hierarchical references;
- generated instances.

For module instances, distinguish:

```text
module identity
instance identity
port identity
signal/value identity
```

Do not use names as substitutes for semantic identity.

Names can change during transformations.

---

# 15. Parameterization

Hardware designs frequently require:

- widths;
- dimensions;
- pipeline depths;
- memory sizes;
- feature flags;
- implementation choices.

For parameters document:

- allowed types;
- evaluation rules;
- scoping;
- dependencies;
- default values;
- constant requirements;
- whether expressions are compile-time;
- canonicalization rules;
- legality constraints.

Avoid arbitrary textual substitution as the semantic definition of parameters.

A parameter should have a well-defined value domain and evaluation model.

---

# 16. Memory Semantics

Memory operations must specify more than address and data types.

Document:

- number of ports;
- read/write behavior;
- synchronous/asynchronous reads;
- write latency;
- read latency;
- read-during-write behavior;
- byte enables;
- alignment;
- initialization;
- reset behavior;
- depth;
- address width;
- out-of-bounds semantics;
- multi-port conflicts;
- ordering;
- atomicity where applicable.

Important cases include:

```text
read-first
write-first
no-change
undefined
```

Do not leave read-during-write behavior implicit.

---

# 17. Protocol and Handshake Semantics

For transaction-oriented hardware, define:

- producer;
- consumer;
- transfer condition;
- valid semantics;
- ready semantics;
- backpressure;
- buffering;
- ordering;
- duplication;
- loss;
- fairness;
- deadlock conditions.

For a valid/ready protocol, a common contract is:

```text
transfer occurs when valid && ready
```

But the dialect must additionally specify:

- whether valid may depend combinationally on ready;
- whether ready may depend combinationally on valid;
- whether data must remain stable while valid && !ready;
- whether transactions are ordered;
- whether zero-latency paths are legal.

Do not assume that naming signals `valid` and `ready` automatically defines their semantics.

---

# 18. Control Flow

Hardware control flow is not necessarily software control flow.

Document whether branches represent:

- compile-time selection;
- combinational muxing;
- sequential control;
- FSM transitions;
- event-driven control;
- transaction routing.

A basic-block graph alone does not establish temporal semantics.

When representing loops, distinguish:

- combinational feedback;
- sequential loops;
- iterative controllers;
- static elaboration loops;
- runtime loops.

---

# 19. Combinational Cycles and Sequential Cycles

The dialect must explicitly state whether cycles are legal.

If combinational cycles are illegal:

- provide verification;
- define how cycles are detected;
- specify whether indirect cycles count.

If sequential feedback is legal:

- document the state boundary;
- identify what breaks the combinational cycle.

Do not rely on downstream synthesis tools to determine whether an IR cycle is meaningful.

---

# 20. Unknown, High-Impedance, and Multi-Valued Logic

If the dialect models Verilog-like semantics, decide whether it represents:

- 2-state logic;
- 4-state logic;
- symbolic unknowns;
- high impedance;
- don't-care values.

Do not casually treat:

```text
X == 0
```

or

```text
X == 1
```

as ordinary boolean semantics.

Document how optimization interacts with unknown values.

An optimization valid under 2-state semantics may be invalid under 4-state semantics.

---

# 21. Attributes

Use attributes for semantic metadata that does not deserve a standalone SSA operation or type.

Examples:

- latency;
- reset polarity;
- implementation hints;
- synthesis directives;
- physical constraints;
- protocol metadata;
- naming hints.

Distinguish:

### Semantic attributes

Changing the attribute changes program meaning.

### Optimization attributes

Changing the attribute changes optimization behavior but should preserve semantics.

### Code-generation attributes

Changing the attribute affects emitted representation.

### Naming/debug attributes

Primarily affect observability or presentation.

Document which category each attribute belongs to.

Avoid turning attributes into an unstructured escape hatch.

---

# 22. Operations

Every operation should document at minimum:

- purpose;
- semantic definition;
- operands;
- results;
- operand/result types;
- attributes;
- regions;
- termination;
- side effects;
- state;
- timing;
- legality;
- verification;
- canonicalization;
- lowering;
- examples.

Where relevant also document:

- clock;
- reset;
- latency;
- resource usage;
- hierarchy;
- protocol behavior;
- memory effects.

An operation named `foo` is not sufficiently documented by saying "performs foo."

---

# 23. Traits

Use traits for structural properties that are reusable and mechanically meaningful.

Examples include:

- single block;
- terminator;
- isolated-from-above;
- same operand/result type;
- commutativity;
- purity/speculatability;
- region properties.

A trait should express a property that is genuinely invariant.

Do not add traits simply because they make generated documentation look complete.

---

# 24. Interfaces

Interfaces should expose semantic capabilities needed by generic analyses and transformations.

Prefer interfaces when multiple unrelated operations or dialects need to answer the same question.

Examples:

- "Does this operation have a clock?"
- "What are this operation's operands?"
- "Can this operation be lowered to a memory interface?"
- "What are this operation's latency characteristics?"
- "How does this operation participate in instance hierarchy?"
- "Can this operation be scheduled?"
- "What resource does this operation consume?"

An interface should answer a reusable semantic question.

Avoid dialect-specific type checks such as:

```text
if operation is MyDialectFooOp
```

when the actual requirement is a general capability.

MLIR explicitly uses interfaces to decouple analyses and transformations from concrete operation classes.

---

# 25. Verification

Every non-trivial construct should have explicit verification rules.

Verification should check properties such as:

- width compatibility;
- type compatibility;
- clock-domain constraints;
- reset compatibility;
- parameter bounds;
- operand/result count;
- port consistency;
- hierarchy validity;
- memory configuration;
- protocol invariants;
- region structure;
- illegal cycles where statically detectable.

Separate:

### Structural validity

"The IR is well formed."

from:

### Semantic validity

"The design has the intended meaning."

from:

### Target legality

"The design can be lowered to target X."

Do not put target-specific restrictions into the generic dialect unless the restriction is intrinsic to the dialect.

---

# 26. Canonicalization and Normal Forms

Define canonical forms where they provide real value.

Consider:

- constant folding;
- redundant connection elimination;
- identity operation removal;
- width normalization;
- parameter normalization;
- commutative operand ordering;
- redundant cast elimination;
- structural deduplication.

Do not canonicalize away information that later passes require.

Especially protect:

- timing information;
- clock/reset identity;
- hardware object identity;
- debug information;
- externally visible names;
- protocol constraints.

---

# 27. Lowering Contracts

Every dialect should document its intended lowering relationships.

For each important operation specify:

```text
Source dialect
    ↓
Intermediate dialect
    ↓
Lower-level dialect
    ↓
Backend / HDL / netlist
```

For every lowering, identify:

- preserved semantics;
- deliberately discarded information;
- introduced operations;
- required analyses;
- target assumptions;
- legality preconditions.

A lowering should not be described merely as "converts X to Y."

Describe what semantic information is preserved.

---

# 28. Do Not Prematurely Lower

Do not introduce low-level details simply because they can be represented.

Examples of premature lowering:

- replacing a protocol with arbitrary muxes before protocol analysis;
- replacing a memory with gates before memory optimizations;
- replacing a pipeline with registers before scheduling;
- lowering symbolic parameters before specialization;
- lowering hierarchy before module-level transformations.

A dialect should preserve useful abstraction until the information is no longer needed.

---

# 29. Representation vs Implementation

Always distinguish:

```text
What the hardware does
```

from:

```text
How the hardware is implemented
```

For example:

- multiplication is behavior;
- a Wallace tree is implementation;
- a RAM abstraction is behavior;
- a banked SRAM implementation is implementation;
- a FIFO protocol is behavior;
- a particular FIFO cell library is implementation.

Do not encode an implementation choice in a supposedly target-independent operation.

---

# 30. Cost Models

If operations have meaningful hardware costs, document whether cost is:

- intrinsic;
- target-dependent;
- estimated;
- symbolic;
- derived from parameters.

Avoid embedding fixed area/timing numbers in a generic dialect.

For example:

```text
mul latency = 3
```

is generally target-dependent.

Prefer a representation capable of expressing the fact that latency is known, constrained, or estimated without pretending that one implementation is universal.

---

# 31. Scheduling Semantics

For scheduled hardware, distinguish:

- latency;
- initiation interval;
- absolute cycle;
- relative cycle;
- dependency;
- resource availability;
- pipeline stage;
- reservation.

Document whether schedules are:

- hard constraints;
- hints;
- derived information;
- temporary compiler state.

A schedule annotation should not silently become part of functional semantics unless explicitly intended.

---

# 32. Side Effects and Observability

For every operation determine whether it:

- reads state;
- writes state;
- changes control state;
- schedules an event;
- changes a signal;
- allocates a resource;
- communicates externally;
- affects simulation state;
- has observable naming/hierarchy effects.

Use side-effect interfaces or equivalent mechanisms where the framework provides them.

Do not mark a stateful operation as pure merely because it produces an SSA result.

---

# 33. External Modules and Black Boxes

External constructs should specify:

- interface;
- symbol identity;
- parameter interface;
- timing assumptions;
- clock/reset requirements;
- side effects;
- synthesis/simulation meaning;
- whether implementation is known.

Do not assume that an external module is behaviorally opaque in every context.

A black box can still have:

- known latency;
- known protocol;
- known resource requirements;
- known timing constraints.

---

# 34. Verification and Formal Semantics

When appropriate, document how dialect constructs map to:

- simulation;
- equivalence checking;
- property checking;
- SMT/SAT reasoning;
- temporal logic;
- cycle-accurate models.

Be explicit about semantic gaps.

For example:

```text
RTL semantics → cycle-level transition system
```

is different from:

```text
event-driven HDL semantics → event queue
```

Do not claim equivalence between models without specifying the assumptions.

---

# 35. Documentation Requirements

Every dialect should have documentation at three levels.

## Level 1 — Dialect overview

Explain:

- purpose;
- abstraction level;
- design philosophy;
- semantic model;
- major types;
- major operations;
- intended users;
- intended transformations;
- relationship to neighboring dialects;
- lowering path.

## Level 2 — Semantic reference

For each operation/type/attribute document:

- syntax;
- semantics;
- invariants;
- examples;
- legality;
- interactions with other constructs.

## Level 3 — Engineering rationale

Explain difficult design decisions:

- why a construct exists;
- why information is represented in a particular place;
- why an operation is separate from another;
- why a type is distinct;
- why a particular abstraction is retained;
- known limitations;
- alternatives considered.

Do not put all rationale into terse operation descriptions.

---

# 36. Recommended Dialect Documentation Structure

Use this structure for the main dialect document:

```text
# <name> Dialect

## Overview
## Design Goals
## Non-Goals
## Abstraction Level
## Semantic Model

## Type System
### Scalar Types
### Aggregate Types
### Resource Types
### Signal/Reference Types
### Clock/Reset Types

## Operations
### Structural Operations
### Combinational Operations
### Sequential Operations
### Memory Operations
### Control Operations
### Communication Operations

## Attributes
## Traits
## Interfaces

## Regions and Blocks
## Hierarchy
## Parameterization
## Timing Model
## State Model
## Memory Model
## Protocol Model

## Verification Rules
## Canonical Forms
## Transformation Rules
## Lowering
## Simulation
## Synthesis
## Formal Semantics

## Examples

## Known Limitations
## Design Rationale
## Future Work
```

Not every section is mandatory, but omissions should be deliberate.

---

# 37. Operation Documentation Template

Every important operation should follow a consistent template:

```text
## `<dialect>.<operation>`

### Purpose

What hardware concept does this operation represent?

### Semantics

What does the operation mean?

### Operands

For every operand:

- semantic role;
- type;
- constraints;
- timing;
- ownership.

### Results

For every result:

- semantic role;
- type;
- latency;
- state relationship.

### Attributes

For every attribute:

- meaning;
- type;
- default;
- constraints;
- semantic category.

### Regions

Describe region semantics, isolation, control flow, and termination.

### State

Describe persistent state, if any.

### Timing

Describe clock, latency, event, and scheduling behavior.

### Verification

List mandatory invariants.

### Canonicalization

List legal simplifications.

### Lowering

Describe intended lower-level representation.

### Example

Provide a minimal valid example.

### Non-Examples

Provide examples that look plausible but are illegal or semantically different.
```

---

# 38. Type Documentation Template

```text
## `!<dialect>.<type>`

### Meaning

What does a value of this type represent?

### Value Domain

What values are representable?

### Identity

Which parameters determine type identity?

### Operations

What operations can consume/produce the type?

### Conversion

What conversions are legal?

### Hardware Meaning

Does it represent:

- data;
- storage;
- signal;
- resource;
- reference;
- protocol endpoint;
- clock;
- reset;
- physical object?

### Lowering

What lower-level representation preserves its meaning?
```

---

# 39. Attribute Documentation Template

```text
## `<attribute>`

### Purpose
### Type
### Default
### Semantic Effect
### Constraints
### Verification
### Transformation Rules
### Lowering Behavior
```

Always state whether the attribute changes semantics or only affects implementation.

---

# 40. Examples Are Part of the Specification

Every non-trivial construct should have at least one valid example.

For complex semantics also provide:

- minimal example;
- realistic example;
- interaction example;
- invalid example;
- lowering example.

Examples should demonstrate semantics rather than only syntax.

A good example answers:

> "Why would an IR designer need this construct?"

---

# 41. Common Pitfalls

## 41.1 Designing from HDL syntax

Do not begin with:

> "How do we represent this SystemVerilog syntax?"

Begin with:

> "What semantic concept is being represented?"

Different HDLs can express the same hardware semantics with very different syntax.

---

## 41.2 Building a Verilog AST

An IR is not necessarily an AST.

An AST preserves source-language structure.

An IR should preserve information needed for:

- analysis;
- optimization;
- transformation;
- lowering;
- verification.

Avoid reproducing every syntactic distinction from the source HDL.

---

## 41.3 Encoding Everything as SSA

SSA is powerful, but not every hardware concept is simply a value.

Avoid pretending that:

- registers;
- wires;
- clocks;
- memories;
- channels;
- events;
- module instances

are interchangeable with ordinary SSA values.

---

## 41.4 Overusing Attributes

Attributes are not a substitute for semantic structure.

If a concept:

- participates in dataflow;
- has identity;
- has operands/results;
- has regions;
- has state;
- requires transformations;

it may deserve an operation or type rather than an opaque attribute.

---

## 41.5 Under-specifying Width

Never assume width from context when width affects semantics.

Document:

- source widths;
- result width;
- extension;
- truncation;
- overflow;
- signedness.

---

## 41.6 Confusing Latency with Delay

Latency is not necessarily physical propagation delay.

A pipeline may have:

```text
latency = 4 cycles
```

while each combinational stage has a separate timing constraint.

Do not use one concept to represent the other.

---

## 41.7 Hiding Clock Domains

A design may be logically connected but temporally unsafe.

Do not let ordinary type compatibility imply clock-domain compatibility unless that is genuinely guaranteed.

---

## 41.8 Ignoring Reset Priority

These are different:

```text
if reset:
    state := R
else if enable:
    state := X
```

and:

```text
if enable:
    if reset:
        state := R
    else:
        state := X
```

The dialect must make such differences explicit.

---

## 41.9 Leaving Memory Collision Semantics Undefined

Read/write behavior during the same cycle must be defined or explicitly marked undefined.

---

## 41.10 Treating Names as Identity

Names can be regenerated.

Use symbol/reference mechanisms for semantic identity.

Names should generally be hints unless explicitly specified otherwise.

---

## 41.11 Target-Specific Semantics in Generic Dialects

Avoid encoding:

- FPGA LUT counts;
- ASIC cell names;
- specific vendor primitives;
- fixed timing numbers;

in a target-independent abstraction unless the dialect's purpose is target-specific.

---

## 41.12 Mixing Simulation and Synthesis Semantics

A simulation construct may have no direct synthesis equivalent.

A synthesis-oriented construct may have no meaningful event-level interpretation.

Document which semantic universe the dialect belongs to.

---

## 41.13 Assuming Structural Equality Means Behavioral Equality

Two circuits can have different structure but equivalent behavior.

Conversely, structurally similar circuits can have different:

- timing;
- state;
- reset;
- event;
- X-state behavior.

Do not use structural equality as a semantic equivalence criterion.

---

## 41.14 Optimizing Across Unknown Semantics

An optimization is only valid under the dialect's semantic model.

Examples involving X/Z, multiple drivers, asynchronous behavior, or event ordering require particular care.

---

## 41.15 Making Lowering Irreversible Too Early

Once information is lowered away, later passes cannot recover it.

Keep high-level information until all transformations that depend on it have run.

---

## 41.16 Using One Giant Hardware Dialect

A single dialect containing:

- arithmetic;
- RTL;
- protocols;
- simulation;
- verification;
- physical design;
- target-specific primitives

can become difficult to reason about.

Use dialect boundaries when they correspond to meaningful semantic boundaries.

---

## 41.17 Excessive Fragmentation

The opposite problem also occurs.

Do not create a new dialect merely because a small set of operations exists.

Create a boundary when it provides:

- semantic isolation;
- reusable abstractions;
- independent transformations;
- meaningful ownership;
- interoperability benefits.

---

# 42. Review Checklist

Before accepting a new hardware dialect:

### Semantics

- [ ] Abstraction level is stated.
- [ ] Goals and non-goals are stated.
- [ ] Semantic model is written.
- [ ] State model is defined.
- [ ] Timing model is defined.
- [ ] Clock semantics are defined.
- [ ] Reset semantics are defined.
- [ ] Memory semantics are defined where applicable.
- [ ] Protocol semantics are defined where applicable.
- [ ] Unknown/X/Z semantics are defined where applicable.

### Types

- [ ] Width semantics are explicit.
- [ ] Signedness is explicit.
- [ ] Value/signal/storage/reference distinctions are clear.
- [ ] Parameterized types have defined identity.
- [ ] Conversion rules are defined.

### Operations

- [ ] Every operation has a semantic definition.
- [ ] Operand/result constraints are explicit.
- [ ] Side effects are explicit.
- [ ] State behavior is explicit.
- [ ] Timing behavior is explicit.
- [ ] Verification rules exist.
- [ ] Canonicalization rules are considered.
- [ ] Lowering is documented.

### Composition

- [ ] Interfaces are used for reusable semantic capabilities.
- [ ] Traits express true invariants.
- [ ] Hierarchy is well-defined.
- [ ] Symbol/reference semantics are explicit.
- [ ] Cross-dialect interactions are documented.

### Transformation

- [ ] Legal transformations are documented.
- [ ] Information that must be preserved is identified.
- [ ] Information that may be discarded is identified.
- [ ] Target-specific constraints are separated from generic semantics.

### Documentation

- [ ] Overview exists.
- [ ] Semantic reference exists.
- [ ] Rationale exists for non-obvious design decisions.
- [ ] Valid examples exist.
- [ ] Invalid examples exist where ambiguity is likely.
- [ ] Lowering examples exist.
- [ ] Known limitations are documented.

---

# 43. Recommended Dialect Architecture

A hardware compiler should generally distinguish semantic layers rather than forcing one dialect to represent everything.

A possible architecture is:

```text
Source / HDL / Algorithmic IR
              |
              v
       High-Level HW IR
              |
       +------+------+
       |             |
       v             v
   Dataflow       Control/FSM
       |             |
       +------+------+
              |
              v
        RTL / Structural
              |
       +------+------+
       |             |
       v             v
   Arithmetic      Memory
       |             |
       +------+------+
              |
              v
       Logic / Netlist
              |
              v
       Target Mapping
              |
              v
        HDL / Cells / Bitstream
```

This is an architectural pattern, not a mandatory pipeline.

The correct decomposition depends on the compiler's goals.

---

# 44. Relationship to MLIR

When implemented in MLIR, use MLIR's native mechanisms deliberately:

- dialects for semantic namespaces;
- operations for computations and structural constructs;
- types for value/resource domains;
- attributes for metadata and parameters;
- regions for nested control/behavior;
- traits for reusable structural properties;
- interfaces for generic semantic capabilities;
- symbols for identity;
- verifiers for invariants;
- canonicalization for normal forms;
- conversion infrastructure for lowering.

Do not recreate these mechanisms inside the dialect without a strong reason.

MLIR's dialect documentation recommends declarative definitions and generated documentation, while interfaces exist specifically to allow analyses and transformations to work across dialect boundaries without hard-coding individual operation types.

---

# 45. Relationship to CIRCT

CIRCT provides useful reference designs for hardware IR decomposition.

Relevant examples include:

- `hw` — generic hardware structure and hierarchy;
- `comb` — combinational logic;
- `seq` — sequential logic;
- `handshake` — dataflow/handshake circuits;
- `fsm` — finite-state machines;
- `pipeline` — pipelined structures;
- `llhd` — event-driven simulation semantics;
- `sv` — SystemVerilog-oriented constructs;
- `synth` — synthesis-related constructs.

Do not copy CIRCT's dialect boundaries mechanically.

Instead ask:

> What semantic boundary does each dialect establish, and is that boundary appropriate for this compiler?

The CIRCT `hw` dialect is deliberately generic and serves as a substrate for higher-level dialects rather than attempting to model all SystemVerilog semantics itself.

---

# 46. Dialect Documentation Should Explain the Semantic Graph

The most useful documentation is not merely:

```text
Op A
Op B
Op C
```

It should explain:

```text
Source concept
      |
      v
Dialect concept
      |
      +--> analysis
      |
      +--> transformation
      |
      +--> lowering
      |
      v
Target concept
```

For every important construct, document where it comes from and where it goes.

This allows developers to understand the role of the construct in the compiler rather than learning isolated operations.

---

# 47. Required Documentation for a New Hardware Dialect

A new dialect should normally ship with:

```text
docs/
  Dialect.md
  Rationale.md
  Semantics.md
  Lowering.md
  Examples.md
```

If the project is small, these may initially be combined.

The minimum complete documentation should still cover:

1. motivation;
2. abstraction level;
3. semantic model;
4. type system;
5. operations;
6. attributes;
7. invariants;
8. timing/state model;
9. transformation rules;
10. lowering;
11. examples;
12. limitations;
13. rationale.

---

# 48. Final Engineering Rule

When uncertain, prefer the representation that preserves the most **useful semantic information** while remaining analyzable and transformable.

Do not optimize the IR for:

- ease of printing;
- resemblance to an HDL;
- ease of implementing the first lowering;
- minimal number of operations.

Optimize it for the compiler's complete lifecycle:

```text
express
    ↓
verify
    ↓
analyze
    ↓
transform
    ↓
schedule
    ↓
lower
    ↓
simulate / verify
    ↓
synthesize / emit
```

A good hardware dialect makes illegal states difficult to represent, important semantic distinctions explicit, and generic compiler transformations possible without requiring every pass to understand every operation individually.
