# pliron-hw

Hardware dialect ecosystem for [pliron](https://github.com/pliron-org/pliron), retaining parity with LLVM CIRCT / MLIR hardware IR.

## Dialect Ecosystem Overview

| Dialect | Abstraction Level | Semantics & Contract (AGENTS.md) | Parity Target |
| :--- | :--- | :--- | :--- |
| **`hw`** | **Structural Netlist & Modules** | Module hierarchy, ports, instances, packed arrays, records, wires, and bit-level slicing | **CIRCT `hw` Dialect** |
| **`comb`** | Pure Combinational Gates | Combinational logic functions, zero-latency arithmetic, muxes, bitwise reduction | CIRCT `comb` Dialect |
| **`seq`** | Sequential Storage & State | Explicit clock domains, reset priority, registers, memory banks | CIRCT `seq` Dialect |
| **`sv`** | SystemVerilog Procedural AST | Synthesizable emission, always blocks, non-blocking assignments | CIRCT `sv` Dialect |

---

## The `hw` Dialect (Partial MLIR Parity)

The `hw` dialect defines core hardware types and structural operations for representing netlists and hierarchy without prescribing synthesis or event-driven simulation mechanics.

### 1. Type System Parity

| Type | MLIR / CIRCT Syntax | pliron-hw Rust Representation | Semantic Contract |
| :--- | :--- | :--- | :--- |
| `IntType` | `!hw.int<w>` / `i<w>` | `hw.int<width>` | Arbitrary-width signless hardware wire or bus |
| `InoutType` | `!hw.inout<elem>` | `hw.inout<element_type>` | Bidirectional / inout net (multi-driver resolution per AGENTS.md §13) |
| `ArrayType` | `!hw.array<size x elem>` | `hw.array<size x element_type>` | Packed fixed-size multidimensional hardware array |
| `StructType` | `!hw.struct<f0: t0, ...>` | `hw.struct<f0: t0, ...>` | Named hardware aggregate / bundle of ports or fields |
| `UnionType` | `!hw.union<f0: t0, ...>` | `hw.union<f0: t0, ...>` | Hardware union sharing physical bit storage |
| `TypeAliasType` | `!hw.typealias<@sym, t>` | `hw.typealias<@sym, inner_type>` | Reference to a symbolic `hw.typedecl` |
| `ModuleType` | `!hw.module_type<...>` | `hw.module_type<in (...), out (...)>` | First-class functional hardware interface signature |
| `EnumType` | `!hw.enum<Name: iN, ...>` | `hw::types::EnumType::get(ctx, name, underlying, variants)` | Named finite domain with explicit bit encodings |

### 2. Operations Parity

- **Hierarchy & Modules**:
  - `hw.module`: Structural hardware module container with single `Graph` region (`has_ssa_dominance = false`) and input block arguments.
  - `hw.module.extern`: External black-box module declaration (ASIC macro, PLL, IP block).
  - `hw.output`: Terminator driving enclosing module output ports.
  - `hw.instance`: Instantiation of a module with instance name, module symbol, port inputs, and result wires.
- **Connectivity & Net Identity**:
  - `hw.wire`: Named internal hardware wire with explicit identity, preventing silent collapse during passes (AGENTS.md §13).
  - `hw.bitcast`: Reinterpretation between types of identical total bitwidth.
- **Constants & Bit-Level Manipulation**:
  - `hw.constant`: Bitvector integer constant materialization.
  - `hw.concat`: Concatenation of N operands into a single wider bitvector.
  - `hw.slice`: Contiguous bit-slice extraction starting at `low_bit`.
- **Array Operations**:
  - `hw.array_create`: Constructs packed array from element SSA operands.
  - `hw.array_get`: Dynamic or static indexing into array.
  - `hw.array_slice`: Sub-array extraction.
  - `hw.array_concat`: Concatenation of arrays of identical element types.
- **Struct Operations**:
  - `hw.struct_create`: Pack values into a hardware struct.
  - `hw.struct_extract`: Read field by name.
  - `hw.struct_inject`: Functional update of a field.
  - `hw.struct_explode`: Unpack all fields into separate SSA values.
- **Declarations**:
  - `hw.typedecl`: Named type declaration symbol.

Use `hw` for structural identity and hierarchy, `comb` for pure zero-cycle
functions, and `seq` for clocked state. This division keeps enum encodings,
arithmetic signedness, and reset/clock priority available to verification and
lowering. Lower structurally only after these semantic analyses have run.

Further design guidance is available in:

- [`docs/pliron_hardware_ir_constraints.md`](docs/pliron_hardware_ir_constraints.md): pliron constraints and their impact on hardware abstractions;
- [`docs/hardware_layout_interface.md`](docs/hardware_layout_interface.md): hardware physical layout, bitwidth calculations, and aggregate packing;
- [`docs/sv_dialect.md`](docs/sv_dialect.md): SV operation contracts;
- [`docs/sv_transformations.md`](docs/sv_transformations.md): legal and illegal transformations between semantic dialects and SV emission intent.

