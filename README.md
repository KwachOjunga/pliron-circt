# pliron-circt

Dialect ecosystem for [pliron](https://github.com/pliron-org/pliron), retaining parity with LLVM CIRCT / MLIR IR.

## Dialect Ecosystem Overview

A list of currently supported dialects and their parity with the upstream CIRCT / MLIR ecosystem.

| Dialect | Abstraction Level | Semantics & Contract | Parity Target |
| :--- | :--- | :--- | :--- |
| **`hw`** | **Structural Netlist & Modules** | Module hierarchy, ports, instances, packed arrays, records, wires, and bit-level slicing | **CIRCT `hw` Dialect** |
| **`comb`** | Pure Combinational Gates | Combinational logic functions, zero-latency arithmetic, muxes, bitwise reduction | CIRCT `comb` Dialect |
| **`seq`** | Sequential Storage & State | Explicit clock domains, reset priority, registers, memory banks | CIRCT `seq` Dialect |
| **`sv`** | SystemVerilog Procedural AST | Synthesizable emission, always blocks, non-blocking assignments | CIRCT `sv` Dialect |

---

Use `hw` for structural identity and hierarchy, `comb` for pure zero-cycle
functions, and `seq` for clocked state. This division keeps enum encodings,
arithmetic signedness, and reset/clock priority available to verification and
lowering. Lower structurally only after these semantic analyses have run.
