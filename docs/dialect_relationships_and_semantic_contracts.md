# Dialect Relationships and Semantic Contracts

## 1. Executive Summary & Purpose

The `pliron-hw` compiler framework is architected around the core principle established in `AGENTS.md`:

> **"A dialect is a semantic contract, not merely a collection of operations."**

Hardware intermediate representations are uniquely vulnerable to semantic degradation. A representation that appears structurally sound can generate catastrophic hardware bugs if temporal semantics, clock domains, reset priorities, bit widths, signedness, or memory hazards are lost or conflated across compilation passes.

To guarantee correctness, `pliron-hw` divides hardware representation into four clean, orthogonal dialects:
1. **`hw`**: Structural hierarchy, module boundaries, instance composition, and bit-accurate aggregate types.
2. **`comb`**: Pure mathematical, zero-latency combinational dataflow.
3. **`seq`**: Explicit sequential state transition systems, clock domains, reset semantics, and memory resources.
4. **`sv`**: Target-facing SystemVerilog syntactic and emission structures.

This document formalizes the architectural rules governing how these dialects interact, the lowering pipelines between them, and the strict semantic contracts each dialect upholds.

---

## 2. Dialect Taxonomy & Abstraction Levels

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        Front-End Ingestion                             │
│                  SystemVerilog Source Text (IEEE 1800)                 │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                      SV Dialect (`sv`)                                 │
│      Syntactic representation of SystemVerilog AST constructs          │
│    (always_comb, always_ff, assign, logic_decl, expression ops)        │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                         Lowering & Normalization
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                        Core Semantic Dialects                          │
│                                                                        │
│   ┌────────────────────────┐  ┌────────────────────────────────────┐   │
│   │    Structural (`hw`)   │  │        Combinational (`comb`)      │   │
│   │  - Module boundaries   │  │  - Zero-cycle pure arithmetic      │   │
│   │  - Hierarchy & ports   │  │  - Bitwise & shift operations      │   │
│   │  - Aggregate layouts   │  │  - Pure functions: O = F(I)        │   │
│   │  - Graph regions       │  │  - Acyclic dataflow DAG            │   │
│   └───────────┬────────────┘  └─────────────────┬──────────────────┘   │
│               │                                 │                      │
│               └────────────────┬────────────────┘                      │
│                                │                                       │
│                                ▼                                       │
│               ┌─────────────────────────────────┐                      │
│               │         Sequential (`seq`)      │                      │
│               │  - State registers (CompReg)    │                      │
│               │  - Clocking & gating contracts  │                      │
│               │  - Reset priority (FirReg)      │                      │
│               │  - S(t+1) = F(S(t), I(t))       │                      │
│               │  - Multi-port memory resources  │                      │
│               └─────────────────────────────────┘                      │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                         Target Emission Lowering
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                      SV Dialect (`sv`)                                 │
│        Target-specific emission IR with verified schedules             │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                        Back-End Code Generation                        │
│                 Clean, Synthesizable SystemVerilog Text                │
└────────────────────────────────────────────────────────────────────────┘
```

### 2.1 The Four Abstraction Levels

| Dialect | Abstraction Level | Primary Invariant | What It Models | What It Does NOT Model |
| :--- | :--- | :--- | :--- | :--- |
| **`hw`** | Structural Netlist | Spatial concurrency & hierarchy | Modules, ports, wires, aggregate packing (`hw.array`, `hw.struct`), symbol resolution | Procedural control flow, computation, or time |
| **`comb`** | Pure Mathematical Dataflow | Determinism: $O = F(I)$ | Arithmetic, bitwise, shifts, muxes, comparisons | Clocks, registers, event delays, or sequential state |
| **`seq`** | Synchronous Transition System | $S(t+1) = F(S(t), I(t))$ | Clocks, clock gating, sync/async resets, registers, SRAM/ROM memories | Combinational arithmetic, textual syntax, simulation queues |
| **`sv`** | Syntactic RTL AST / Emission | Emission fidelity | Continuous assignments, procedural processes (`always_ff`, `always_comb`), syntax-level expressions | Authoritative hardware semantics or transformation truth |

---

## 3. The Rules of Dialect Relationships

### Rule 1: Separation of Concerns & Anti-Inflation
* **Combinational Isolation**: No operation in the `comb` dialect may accept a clock, define a state element, or introduce temporal delay.
* **Sequential Isolation**: No operation in the `seq` dialect may perform arithmetic computation internally. A register's next-state logic must be driven by an external SSA value (typically computed via `comb`).
* **Structural Purity**: The `hw` dialect defines container boundaries and connections, never execution steps.
* **Emission Subordination**: The `sv` dialect is never the source of truth for semantic analysis or circuit optimization. Transformations (e.g. constant folding, dead-code elimination, retiming) must operate on `comb`, `seq`, and `hw`.

### Rule 2: Ingestion Pipeline (Parsing $\rightarrow$ Core Semantics)
When parsing SystemVerilog source text:
1. The parser emits AST-level operations in `sv` (e.g. `sv.always_ff`, `sv.always_comb`, `sv.assign`, `sv.binary_expr`).
2. A **Lowering / Normalization Pass** transforms syntactic `sv` constructs into canonical core operations:
   - `sv.always_comb` $\rightarrow$ Unwrapped pure SSA dataflow in `comb`.
   - `sv.binary_expr`, `sv.unary_expr` $\rightarrow$ Canonical `comb.add`, `comb.mul`, `comb.and`, etc.
   - `sv.always_ff` $\rightarrow$ `seq.compreg` or `seq.firreg` with explicit `!seq.clock` and `!seq.reset`.
   - `sv.mem_decl` + procedural reads/writes $\rightarrow$ `seq.hlmem`, `seq.hlmem.read`, `seq.hlmem.write`.

### Rule 3: Target Emission Pipeline (Core Semantics $\rightarrow$ SystemVerilog)
When generating SystemVerilog code from optimized IR:
1. Core operations (`comb`, `seq`, `hw`) are converted to `sv` target operations via a verified lowering pass.
2. Registers in `seq` lower into `sv.always_ff` or `sv.always_ff_no_reset`, ensuring reset priority and clock sensitivity lists are faithfully configured.
3. Complex combinational trees lower into continuous assignments (`sv.assign`) or procedural blocks (`sv.always_comb`) depending on destination variable characteristics.
4. The `sv::printer` traverses the `sv` dialect AST and serializes clean IEEE 1800-compliant RTL text.

---

## 4. Semantic Contracts Upholding Dialect Integrity

### 4.1 Value vs. Signal vs. Storage
- **`comb` Value**: Represents an instantaneous snapshot of pure computational data. It has no physical identity, no wire delay, and no lifecycle beyond SSA use-def relationships.
- **`hw.wire` Signal**: Represents a physical structural net. It possesses name identity, supports forward references within graph regions, and bridges multi-driver or inout scenarios.
- **`seq` Storage**: Represents persistent state across clock cycles. A storage element is distinct from its output value; its output is a newly derived signal active for the duration of the current clock cycle.

### 4.2 Clock & Reset Semantics

#### Clock Contract
- Clocks in `seq` and `sv` must be typed as `!seq.clock` (or explicitly validated as 1-bit signless integers during transition).
- A boolean signal `i1` is **not** semantically equivalent to a clock domain. Clock signals may not participate in arbitrary arithmetic operations in `comb` without an explicit clock-conversion or gating construct (`seq.clock_gate`).

#### Reset Contract & Priority
In `seq.firreg` and `sv.always_ff`, the reset contract is explicitly parameterized:
```text
At active clock edge (or asynchronous reset assertion):
if (reset == active_level) {
    state <= reset_value;
} else if (enable) {
    state <= next_state;
} else {
    state <= state;
}
```
- **Reset Dominance**: Reset evaluation strictly dominates clock-enable evaluation.
- **Synchronous vs. Asynchronous**:
  - `is_async = true`: The reset edge is included in the process sensitivity list (`always_ff @(posedge clk or posedge/negedge rst)`).
  - `is_async = false`: The reset condition is evaluated strictly synchronously at the active clock edge.
- **Polarity**: Encoded via `active_high` or `active_low`. Inversion logic must not be silently hidden in expressions without preserving this metadata.

### 4.3 Combinational vs. Sequential Cycles
- **Combinational Loops Are Strictly Illegal**: A directed graph of operations in `comb` within an `hw.module` must be an Acyclic Directed Graph (DAG). Any circular dependency between combinational operations without an intervening `seq` storage element is an error detected during verification.
- **Sequential Feedback Is Legal**: Feedback loops in hardware are legal if and only if the cycle path passes through at least one clocked storage boundary (`seq.compreg`, `seq.firreg`, or synchronous memory write/read). The state boundary isolates time $t$ from time $t+1$.

### 4.4 Memory Semantics
Operations on `!seq.mem<depth x element_type>` must explicitly define port behavior:
1. **Addressing & Bounds**: Out-of-bounds reads/writes are statically guarded by address width $W = \lceil \log_2(\text{depth}) \rceil$.
2. **Read-During-Write Hazard Policy**: Every memory operation explicitly specifies its collision contract:
   - `read_first`: Read returns the old data stored in memory before the new write occurs.
   - `write_first`: Read returns the newly written data immediately.
   - `undefined`: Concurrent read/write to the same address produces undefined hardware results ($X$).
3. **Latency**:
   - Asynchronous read: Zero-cycle latency.
   - Synchronous read: $N$-cycle pipeline latency (explicitly recorded in `seq.hlmem.read`).

### 4.5 Bit-Width and Type Consistency
- All arithmetic operations in `comb` require strict operand bit-width equality (`lhs.width == rhs.width == result.width`).
- Width extension (zero-extension, sign-extension) and bit-truncation must be expressed via explicit operations (`comb.extract`, `comb.concat`, `hw.bitcast`). Silent implicit truncation or promotion is forbidden.

---

## 5. Information Preservation Invariants

During any lowering, optimization, or serialization pass:
1. **Never Discard Bit Widths**: Never convert sized integers (`i8`, `i16`) into a generic unsized integer type.
2. **Never Discard Clock Domain Identity**: Clocks must not be collapsed into generic control booleans.
3. **Never Discard Reset Priority**: The contract between reset and enable must remain explicit.
4. **Never Discard Hardware Identity on Ports**: Input and output port names define the external ABI of the hardware IP and must remain stable across all compilation steps.
5. **Never Discard Signedness Context**: Signed operations (`comb.icmp slt`, `comb.divs`) must preserve signed interpretation during lowering to SystemVerilog constructs (`$signed(...)`).

---

## 6. Summary of Architectural Guardrails

| Dialect Boundary | Permitted Interaction | Strictly Prohibited Interaction |
| :--- | :--- | :--- |
| **`comb` $\rightarrow$ `seq`** | `comb` outputs drive the `input` or `enable` operands of `seq.compreg`. | `seq` registers instantiated inside pure `comb` expression DAGs. |
| **`seq` $\rightarrow$ `comb`** | `seq.compreg` outputs serve as inputs to `comb` operators. | Combinational cycles crossing through `comb` without register break. |
| **`hw` $\rightarrow$ `comb`/`seq`** | `hw.module` graph region hosts instances of `comb` and `seq` operations. | `comb` or `seq` declaring external module boundaries. |
| **`sv` $\rightarrow$ `core`** | `sv` AST nodes lower to canonical `comb`, `seq`, and `hw` constructs. | Running hardware optimizations directly on unlowered `sv` text nodes. |
| **`core` $\rightarrow$ `sv`** | Verified `comb` + `seq` lower to `sv` target nodes prior to code emission. | Using raw SystemVerilog text strings as intermediate representations. |
