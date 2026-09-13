# Dialect Relationships, Semantic Gaps & Implementation Architecture

## 1. Executive Summary & Purpose

The `pliron-hw` compiler framework is architected around the core principle established in `AGENTS.md`:

> **"A dialect is a semantic contract, not merely a collection of operations."**

The clean division between `hw`, `comb`, `seq`, and `sv` provides a foundation for representing synthesizable hardware. However, dialect boundaries alone are insufficient to guarantee semantic correctness. Hardware descriptions contain several classes of semantics that must be explicitly represented or intentionally excluded:
- Structural connectivity and net identity.
- Procedural control flow and variable update ordering.
- Blocking (`=`) and nonblocking (`<=`) assignment semantics.
- Four-state logic (`0`, `1`, `X`, `Z`) vs. two-state synthesizable execution.
- Compile-time elaboration, parameterization, and generate blocks.
- Registers vs. level-sensitive latches.
- Clock domains and clock-domain crossings (CDC).
- Memory port behavior, bounds, and read-during-write collision hazards.
- Multiple drivers, `inout`, and tri-state structures.
- Packed vs. unpacked aggregate layouts.
- Sizing rules, sign extension, and truncation.
- Protocol-level interfaces.
- Source provenance and diagnostic traceability.

This document establishes the **authoritative architectural decisions, semantic contracts, and lowering guardrails** for `pliron-hw`.

The central architectural principle governing the framework is:

> **Each semantic question must have one authoritative owner, and lowering must never silently discard information owned by another dialect.**

---

## 2. Core Architectural Model: Cooperative Semantic System

### 2.1 Not a Strictly Sequential Lowering Pipeline
The core dialects do **not** form a linear lowering pipeline like `hw → comb → seq`. Instead, they form a **cooperative semantic system** where each dialect governs an orthogonal dimension of hardware representation:

```text
                          ┌─────────────────────────┐
                          │       hw Dialect        │
                          │   Structural Netlist    │
                          │   Module & Hierarchy    │
                          │   Aggregates & Layouts  │
                          └────────────┬────────────┘
                                       │
                         Container Context & Graphs
                                       │
                 ┌─────────────────────┴─────────────────────┐
                 │                                           │
                 ▼                                           ▼
   ┌───────────────────────────┐               ┌───────────────────────────┐
   │       comb Dialect        │               │        seq Dialect        │
   │  Pure Zero-Cycle Dataflow │◄──────────────│  State Transition Systems │
   │   Mathematical Functions  │   Registers   │    Clock Domains & CDC    │
   │         O = F(I)          │──────────────►│    Reset Dominance        │
   │     Strictly Acyclic      │  Next-States  │    S(t+1) = F(S(t), I(t)) │
   └───────────────────────────┘               └───────────────────────────┘
```

- **`hw.module`** provides the structural container (graph region) within which `comb`, `seq`, and structural operations seamlessly coexist.
- **`comb` operations** consume values produced by `seq` state registers, external module inputs, or structural instances.
- **`seq` operations** consume next-state values, enable gates, and address calculations computed via `comb` operations.
- **Sequential feedback** is natural and legal: state register outputs feed combinational logic which feeds back into register inputs, broken by the clock boundary.
- **No lowering is implied** merely by these mutual dependencies; they coexist as peers within an `hw.module`.

---

## 3. Structural Representation Decisions

### 3.1 SSA Values as the Primary Internal Connectivity Representation
- The core IR represents internal dataflow through **Pliron SSA values** rather than independent net objects.
  ```text
  %0 = comb.add %a, %b
  %1 = comb.mux %sel, %0, %c
  %2 = seq.compreg %1, %clk
  ```
- **Benefits**: Explicit use-def relationships, linear dead-code elimination, natural dominance analysis, and tight memory representation.
- **Exception for Structural Nets**: Explicit structural nets (`hw.wire`) are reserved strictly for semantics that SSA cannot express:
  - Multi-driver nets and tri-state busses.
  - Bidirectional `inout` ports.
  - Forward-referencing net cycles requiring named physical identities.

### 3.2 `hw.instance` Produces SSA Results for Output Ports
- Module instances are structural operations whose output ports produce SSA values:
  ```text
  %out0, %out1 = hw.instance @alu_block(%in0, %in1) : (i8, i8) -> (i8, i1)
  ```
- `hw.instance` captures: (1) target module symbol, (2) compile-time parameter bindings, and (3) input port SSA bindings.
- It does **not** create a nested inline module definition; the instantiated module remains an independent `hw.module` symbol.

### 3.3 Symbol-Based Module Identity
- Modules have unique symbol identities in the Pliron context.
- Verification checks:
  1. The target module symbol exists in the symbol table.
  2. The target is an `hw.module` or `hw.extern_module`.
  3. Parameter specializations are valid and satisfy bounds.
  4. Port counts, port directions, and port types match the callee signature exactly.

---

## 4. Type System Decisions

### 4.1 Explicitly Sized Hardware Integer Types
- The core IR has **no implicit or generic unsized integer type**.
- Every integer value carries an explicit bit width and signedness (`Signless`, `Signed`, or `Unsigned`):
  ```text
  !hw.int<8>          // 8-bit signless
  !hw.int<32, signed> // 32-bit signed
  ```
- Operations that compute with integers require explicit width contracts.

### 4.2 Frontend Elimination of Implicit SystemVerilog Sizing
- SystemVerilog's complex implicit width extension and context-determined expression sizing are **eliminated at the frontend ingestion boundary**.
- Implicit zero-extensions become explicit `comb.zext`.
- Implicit sign-extensions become explicit `comb.sext`.
- Implicit truncations become explicit `comb.trunc`.
- Core optimization passes operate on explicit, unambiguous bit widths.

### 4.3 Distinct Width Conversion Operations
The `comb` dialect provides dedicated operations for width adjustments:
- `comb.zext`: Zero-extends an unsigned or signless integer to a larger bit width.
- `comb.sext`: Sign-extends a signed integer (copying the MSB) to a larger bit width.
- `comb.trunc`: Truncates an integer to a strictly smaller bit width.
- `comb.concat`: Concatenates multiple integer vectors into a unified vector.
- `comb.extract`: Extracts a contiguous bit-slice from an integer vector.
- `hw.bitcast`: Strictly reserved for **reinterpreting identically sized types** (e.g. `!hw.array<4 x i8>` to `i32`, or `!hw.struct` to `iN`). It is **forbidden** to use `hw.bitcast` for width conversion.

---

## 5. Four-State Logic & Tri-State Decisions

### 5.1 Two-State Synthesizable Core Semantic Model
- The core `comb` and `seq` dialects operate on **two-state logic (`0`, `1`)**.
- The core IR models **synthesizable hardware behavior**, not full SystemVerilog simulation semantics.
- SystemVerilog simulation constructs involving `X` (unknown) or `Z` (high impedance), such as `casex`, `casez`, `===`, and `!==`, must be:
  1. Resolved or lowered to deterministic two-state logic during frontend elaboration, or
  2. Rejected with a diagnostic if the construct cannot be faithfully synthesized.
- Unknown values must never silently turn into `0` or `1` without explicit lowering rules.

### 5.2 Tri-State and `inout` Handling
- Internal tri-state nets (busses driven by multiple tri-state buffers) are not part of ordinary SSA dataflow.
- Top-level chip pins with `inout` or tri-state drivers are represented using explicit structural operations in `hw` (e.g. `hw.inout_type`).

---

## 6. Aggregate Type Decisions

### 6.1 Strict Distinction Between Packed and Unpacked Aggregates
The type system strictly distinguishes:
- `hw.array` (packed bit-vector arrays vs. unpacked arrays).
- `hw.struct` (packed contiguous bit-fields vs. unpacked records).
- `hw.union` (overlapping bit representations).
- `hw.enum` (symbolic enumerated values with explicit base encoding).

### 6.2 Deterministic Physical Layout
Every aggregate type defines:
- Element ordering (LSB-first vs. MSB-first).
- Field bit offsets and packing rules.
- Total serialized bit width.
- This ensures bitcast and memory layout compatibility across compilation stages.

---

## 7. Combinational & Sequential Semantics Decisions

### 7.1 Pure Combinational Operations (`comb`)
- Semantics are mathematical: $O = F(I)$.
- `comb` operations are stateless, instantaneous, pure, and strictly acyclic.
- No `comb` operation may accept a clock, define a state element, or specify simulation delays.

### 7.2 Persistent State Exclusively in `seq`
- Only `seq` operations may create persistent hardware state across time.
- Core state primitives:
  - `seq.reg` / `seq.compreg` / `seq.firreg`: Edge-triggered flip-flops.
  - `seq.latch`: Level-sensitive storage.
  - `seq.mem` / `seq.hlmem`: Dedicated multi-port RAM/ROM resources.
- Future primitives: `seq.fifo`, `seq.synchronizer`.

### 7.3 Clear Distinction: Registers vs. Latches
- Registers are **edge-triggered** (`posedge` or `negedge`).
- Latches are **level-sensitive** (`enable` active high or low).
- They are modeled as distinct operations to prevent unintended latch inference from masquerading as registered state.

---

## 8. Clock & Reset Decisions

### 8.1 Clocks as First-Class Types
- Clocks are typed as `!seq.clock`, not generic `i1` booleans.
- Clocks cannot participate in arbitrary integer arithmetic in `comb`.
- Derived clocks and clock enables must pass through explicit operations (e.g. `seq.clock_gate`).

### 8.2 Clock Edge as Explicit Register Attribute
- Registers explicitly encode trigger edge (`posedge` or `negedge`) via an attribute, not by checking boolean edge transitions.

### 8.3 Clock Domains & CDC (Clock-Domain Crossing)
- Clocks carry domain metadata (e.g. `domain = "clk_core"`).
- Two clocks are not in the same domain merely because they share `!seq.clock`.
- Implicit data transfers between distinct clock domains are prohibited; crossings must pass through explicit CDC primitives (e.g. `seq.synchronizer`, `seq.async_fifo`).

### 8.4 Reset Ownership & Priority
- Reset is owned exclusively by sequential state operations (`seq.firreg`, `sv.always_ff`).
- Canonical register contract:
  ```text
  At active clock edge (or asynchronous reset edge):
  if (reset == active_level) {
      state <= reset_value;
  } else if (enable) {
      state <= next_state;
  } else {
      state <= state;
  }
  ```
- **Reset Dominance**: Reset evaluation strictly dominates enable. Any alternative priority must be explicitly encoded in attributes.

---

## 9. Procedural SystemVerilog & Assignment Semantics

### 9.1 Intermediate Semantic Normalization
SystemVerilog source text is not lowered directly from raw AST into core IR. It passes through an intermediate procedural normalization phase:

```text
SV Source Text ──► AST ──► Elaboration ──► Procedural CFG/SSA ──► Core (hw + comb + seq)
```

### 9.2 Nonblocking Assignment (NBA) Resolution
- Nonblocking assignments (`<=`) and blocking assignments (`=`) are resolved during procedural analysis.
- For sequential blocks:
  ```verilog
  always_ff @(posedge clk) begin
      a <= b;
      b <= a;
  end
  ```
  The compiler constructs separate next-state expressions (`a_next = b`, `b_next = a`) before instantiating `seq.compreg` operations.
- The core `seq` dialect does not model SystemVerilog's simulation event queue regions (Active, Inactive, NBA, Observed); it models the resulting synchronous state transition system.

---

## 10. Memory Resource Decisions

### 10.1 Memory as a First-Class Stateful Resource
- `seq.mem` represents an addressable RAM/ROM array, not an unpacked collection of individual registers.
- Memory ports are explicit sub-constructs or operations defining:
  - Address width ($W \ge \lceil \log_2(\text{depth}) \rceil$) and data width.
  - Clock and enable signals.
  - Write masks and byte enables.
  - Read latency (0 for asynchronous, $\ge 1$ for synchronous).

### 10.2 Read-During-Write Hazard Contracts
Every memory operation explicitly specifies its collision contract:
- `read_first`: Read returns old stored data before the concurrent write takes effect.
- `write_first`: Read returns the newly written data immediately.
- `undefined`: Concurrent access to the same address yields undefined results ($X$).

### 10.3 Out-of-Bounds Address Semantics
- Addresses outside $0 \le \text{addr} < \text{depth}$ are defined as returning `undefined`. The compiler will not silently inject bounds-clamping hardware unless explicitly requested.

---

## 11. Source SV vs. Emission SV Dialects

### 11.1 Separation of Concerns
```text
SystemVerilog Source Text
          │
          ▼ (Ingestion)
    Source SV AST
          │
          ▼ (Elaboration & Lowering)
     Core Hardware IR (`hw` + `comb` + `seq`)
          │
          ▼ (Target Synthesis / Emission Lowering)
    Emission SV AST (`sv`)
          │
          ▼ (Serialization)
Clean Synthesizable SystemVerilog Text
```

- **Source SV**: Captures the rich syntax of incoming HDL for analysis, AST representation, and procedural normalization.
- **Core HW IR**: Holds the immutable, verified semantic truth (types, state transitions, netlists).
- **Emission SV**: Represents verified target constructs (`always_ff`, `always_comb`, `assign`, `logic`) tailored for clean RTL code generation. The printer never invents semantics, widths, or reset priorities.

---

## 12. Information Preservation Invariants (The 8 Golden Rules)

Every lowering and optimization pass must obey these rules:

1. **Rule 1 — Bit Width Invariance**: Never discard or conflate explicit bit widths. Converting sized integers to unsized representations is forbidden.
2. **Rule 2 — Signedness Preservation**: Never discard signedness context. Signed operations must preserve signed interpretation ($signed) during lowering.
3. **Rule 3 — Clock Domain Invariance**: Never lower a clock to a generic boolean without an explicit clock-gating or conversion operation.
4. **Rule 4 — Reset Priority Preservation**: Never reconstruct reset priority from line ordering; it must remain an explicit contract attribute.
5. **Rule 5 — State Integrity**: Never dissolve persistent state into combinational logic without an explicit transformation.
6. **Rule 6 — Memory Contracts**: Never discard read latency, port structure, write-mask granularity, or collision policies.
7. **Rule 7 — External Port ABI**: Preserve port directions, bit widths, aggregate packing, and clock associations across module boundaries.
8. **Rule 8 — Source Provenance**: Preserve file, line, and column source locations across all lowering stages to ensure accurate verifier diagnostics.

---

## 13. Implementation Scope & Order

### Phase 1: Core Semantic Foundation
1. **Type System**: Bit-accurate integers, `!seq.clock`, `!seq.reset`, packed/unpacked `hw.array` and `hw.struct`.
2. **Structural**: `hw.module`, `hw.output`, `hw.instance`, SSA connectivity.
3. **Combinational**: Sized arithmetic, bitwise, shifts, comparisons, `comb.mux`, and explicit width conversions (`zext`, `sext`, `trunc`, `extract`, `concat`).
4. **Sequential**: `seq.compreg`, `seq.firreg` with clock, enable, and sync/async reset contracts.

### Phase 2: Memory & Intermediate Normalization
5. **Memory**: `seq.mem` / `seq.hlmem` with explicit read/write ports and collision policies.
6. **Procedural SV Normalization**: Elaboration of blocking/nonblocking assignments and expressions into SSA dataflow.

### Phase 3: Emission & Optimization
7. **Target Lowering**: Core `hw` + `comb` + `seq` lowering into emission `sv` operations.
8. **Serialization**: `sv::printer` emitting verified synthesizable SystemVerilog text.
9. **Extensions**: Clock-domain crossing validation, `seq.latch`, and protocol interfaces.
