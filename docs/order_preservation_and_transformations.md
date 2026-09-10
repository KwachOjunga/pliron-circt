# Dialect Transformations & Order Preservation Architecture

## 1. The Core Problem: What Does "Order" Mean in Hardware IR?

In traditional software compilation (e.g., C or LLVM IR), execution order is primarily an imperative instruction sequence governed by the program counter ($PC \to PC+1$) and memory ordering (loads/stores with alias analysis).

In hardware compilers, **hardware is inherently parallel, spatial, and multi-dimensional**. There is no single program counter. Instead, hardware IR exhibits four orthogonal dimensions of "order" that must be rigorously tracked and preserved when transforming dialects amongst each other:

```text
                  ┌──────────────────────────────────────────────┐
                  │          DIMENSIONS OF HARDWARE ORDER        │
                  └──────────────────────────────────────────────┘
                                         │
         ┌───────────────────────────────┼──────────────────────────────┐
         ▼                               ▼                              ▼
  [1. Temporal Order]            [2. Causal Order]             [3. Spatial Order]
  Discrete clock cycles          Zero-delay dataflow           Hierarchical netlist
  t -> t+1 -> t+2                SSA Def-Use DAG               Instances & Ports
  (Governed by: `seq`)           (Governed by: `comb`)         (Governed by: `hw`)
                                         │
                                         ▼
                             [4. Compilation Phase Order]
                             Elaboration -> Canonicalization -> Lowering
```

---

## 2. The Four Dimensions of Order & Their Preservation Contracts

### Dimension 1: Temporal (Cycle) Order (`seq`)
- **Semantic Contract**: State transitions occur only at discrete active clock edges ($\uparrow clk$):
  $$S(t+1) = F(S(t), I(t))$$
- **Preservation Rule**:
  1. **Register Boundary as Temporal Barrier**: A register (`seq.firreg` or `seq.compreg`) splits the dataflow graph into distinct time cycles. No transformation may hoist or sink combinational logic across a register boundary unless explicit retiming transformations are intentionally enabled with formal cycle-latency equivalence checks.
  2. **Latency Invariance**: If output $Y$ is produced $K$ cycles after input $X$, every legal transformation must ensure the latency remains exactly $K$ cycles.
  3. **Reset Dominance Order**: When lowering `seq.firreg`, the reset priority over enable must be strictly preserved:
     $$\text{state}(t+1) = \text{reset} \;?\; V_{rst} : (\text{en} \;?\; D : \text{state}(t))$$
     Lowering to technology multiplexers must place the reset multiplexer closest to the register input pin to prevent enable signals from masking an active reset.

### Dimension 2: Causal (Dataflow) Order (`comb`)
- **Semantic Contract**: Combinational logic computes instantaneous functions with zero cycle delay:
  $$O = F(I)$$
- **Preservation Rule**:
  1. **Acyclic Invariant**: Within a single clock cycle, combinational loops are physical hazards (oscillations or latching race conditions). The causal order is a topological sort of the directed acyclic graph (DAG) formed by SSA operands and results.
  2. **Referential Transparency**: Since `comb` operations are pure, any node can be reordered relative to other independent nodes without changing hardware behavior, provided all inputs dominate (or causally precede) their uses.
  3. **Bit-Level Algebraic Equivalence**: Constant folding and identity simplification preserve bit-level values identically across all possible input bit patterns.

### Dimension 3: Spatial and Structural Order (`hw`)
- **Semantic Contract**: Modules, instances, and wires represent spatial physical components.
- **Preservation Rule**:
  1. **Net Identity Preservation**: While an SSA value can be renamed, a named net (`hw.wire`) or port boundary has physical meaning for synthesis, pinout assignment, and waveform probing. Transformations must preserve symbol tables and port positions.
  2. **Graph-Region Concurrency**: Inside `hw.module`, all statements represent continuously active concurrent hardware. Lexical order inside a graph region does not imply sequential execution order; connectivity alone defines electrical paths.

### Dimension 4: Compilation Phase Order (The Lowering Pipeline)
- **Semantic Contract**: Transformations must proceed monotonically from high-level behavioral intent down to concrete structural implementations.
- **The Anti-Premature-Lowering Principle (AGENTS.md §28)**:
  - Do not lower `seq.firreg` into gate-level multiplexer loops before combinational logic optimization.
  - Do not flatten module hierarchies before module-level interface verification.
  - Do not unpack arrays or structs into individual 1-bit wires before high-level memory or protocol analysis.

---

## 3. The Transformation Pipeline: Step-by-Step

The following sequence details how code moves between the three dialects without loss of order:

```text
┌────────────────────────────────────────────────────────────────────────┐
│ Input IR: High-Level Unified Representation                            │
│ (Uses `hw` for ports/modules, `seq` for registers, `comb` for logic)   │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 1: Structural Validation & Elaboration                           │
│ - Resolve `hw.param_value` against `hw.param_decl`                     │
│ - Validate bitwidths on all `hw.instance` port connections             │
│ - Status: Structural order verified.                                   │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 2: Combinational Optimization (`comb` Canonicalization)          │
│ - Constant fold operations (`comb.add %c1, %c2` -> `%c3`)              │
│ - Redundant slice elimination (`comb.extract(comb.concat(a, b))` -> a) │
│ - Dead combinational logic pruning                                     │
│ - Status: Causal dataflow order preserved; zero cycle change.          │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 3: Sequential Lowering (`seq` -> `hw` + `comb`)                  │
│ - For targets without primitive reset registers:                       │
│   Lower `seq.firreg` to `hw.instance` of standard cell DFF OR          │
│   synthesize priority mux tree:                                        │
│     %d_gated = comb.mux %en, %d, %q                                    │
│     %d_next  = comb.mux %rst, %rst_val, %d_gated                       │
│     %q       = seq.compreg %clk, %d_next                               │
│ - Status: Temporal order strictly preserved; reset priority intact.    │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 4: Structural Netlist Schedule & Verilog Emission                │
│ - Topologically sort operations in the graph region                    │
│ - Emit clean SystemVerilog / structural netlist                        │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Verification and Legality Checks

Every transformation pass enforces strict invariants before and after execution:

1. **Cycle Equivalence Invariant**: Total register depth along any path from module input to module output must remain constant.
2. **Width Compatibility Invariant**: Every operand bitwidth must match the target operation's type signature exactly.
3. **No Unintentional Combinational Cycles**: Detecting combinational cycles by performing cycle-detection on all non-register dependency paths.
4. **Clock-Domain Separation**: Operations cannot mix clocks from different clock domains without an explicit synchronizer or CDC bridge.
