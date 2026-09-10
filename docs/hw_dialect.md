# HW Dialect Specification & Architecture

## 1. Overview & Purpose

The `hw` dialect in `pliron-hw` defines the foundational structural netlist and module hierarchy layer for hardware compilation. It represents hardware components, module boundaries, inter-module connectivity, bit-level transformations, aggregate types (arrays, structs, and unions), and compile-time elaboration facilities (parameters and non-local hierarchical paths).

The design follows the principles in `AGENTS.md`: a dialect is a **semantic contract**, not merely a collection of operations.

---

## 2. Abstraction Level & Semantic Model

### Abstraction Level
- **Level**: Structural netlist with graph-region module definitions, strongly typed ports, and aggregate bit manipulation.
- **What it models**:
  - Module interfaces (directional inputs and outputs, inout bidirectional ports).
  - Netlist connectivity and hierarchical instantiation.
  - Bit-accurate data bundling (packed arrays, structs, tagged unions, type aliases).
  - Elaboration-time parameters and cross-hierarchy routing paths (`hw.hierpath`).
- **What it does NOT model**:
  - Procedural execution or software control flow (no branch instructions, no program counter).
  - Inherent cycle delays or register state transitions (delegated to the `seq` dialect).
  - Arbitrary arithmetic and logical evaluations (delegated to the `comb` dialect).

### Semantic Model
1. **Module Hierarchy**:
   Modules are defined as symbols using `hw.module` or `hw.module_extern`. A module's body is a single **Graph Region** (`has_ssa_dominance = false`), capturing concurrent spatial execution. Inside this region, operations denote hardware instances or structural wires operating concurrently.
2. **Value vs. Signal vs. Storage**:
   - An SSA `Value` in `hw` denotes a **continuously driven signal** at the structural level.
   - `hw.wire` introduces an explicit named net identity with forward-referencing capabilities within the graph region.
   - Values do not represent sequential storage elements. Storage is explicitly created via `seq` operations.

---

## 3. Types as Semantic Contracts

Every type in the `hw` dialect specifies exact bit-level interpretation:

| Type | MLIR Syntax | pliron Rust Construct | Semantic Contract |
| :--- | :--- | :--- | :--- |
| **Bit / Integer** | `iN` | `pliron::builtin::types::IntegerType` | Signless $N$-bit wire/signal. Width is strictly part of type identity. |
| **Inout** | `!hw.inout<T>` | `hw::types::InoutType` | Bidirectional connection port without fixed directionality. Requires tri-state or multi-driver arbitration. |
| **Array** | `!hw.array<N x T>` | `hw::types::ArrayType` | Homogeneous packed aggregate of $N$ elements of type $T$. Total bitwidth is $N \times \text{width}(T)$. |
| **Struct** | `!hw.struct<f1: T1, ...>` | `hw::types::StructType` | Heterogeneous ordered bundle of named fields. Field order and bitwidths are strictly preserved. |
| **Union** | `!hw.union<f1: T1, ...>` | `hw::types::UnionType` | Tagged variant occupying $\max(\text{width}(T_i))$ bits. Encodes mutual exclusivity in hardware storage. |
| **Type Alias** | `!hw.typealias<@sym, T>` | `hw::types::TypeAliasType` | Symbolic name referencing an `hw.typedecl`. Unfolded during lowering without changing bit-level semantics. |

---

## 4. Operation Reference & Invariants

### Hierarchy & Modules
- `hw.module`: Defines a synthesizable hardware block with named inputs and a graph region terminated by `hw.output`.
  - **Invariants**: Argument types must match declared port input types; `hw.output` operand types must match declared module output types.
- `hw.module_extern`: Black-box module reference for standard cells or third-party IP cores.
- `hw.instance`: Instantiates a module symbol with input bindings, producing result SSA values representing module output pins.

### Connectivity & Wires
- `hw.wire`: Represents an explicit net. Essential for cyclic or feedback wiring in graph regions before lowering to technology cells.
- `hw.bitcast`: Lowers or changes type view between bit-equivalent types (e.g. `!hw.array<32 x i1>` to `i32`) without altering physical bit layout.

### Aggregate Operations
- `hw.array_create`: Concatenates $N$ scalar elements of type $T$ into `!hw.array<N x T>`.
- `hw.array_get`: Index-based element lookup `val = arr[idx]`.
- `hw.array_slice`: Extracts a sub-array `!hw.array<W x T>` starting at dynamic or static offset.
- `hw.array_concat`: Combines two arrays into a larger packed array.
- `hw.array_inject`: Non-destructive functional update of an array element at a specified index.
- `hw.struct_create`, `hw.struct_extract`, `hw.struct_inject`, `hw.struct_explode`: Complete algebraic pack/unpack/update operations for hardware record types.
- `hw.union_create`, `hw.union_extract`: Constructs and unpacks tagged variant fields within shared bit storage.

### Parameters & Hierarchical Paths
- `hw.param_decl`: Declares a compile-time elaboration parameter with a default value.
- `hw.param_value`: Accesses an elaboration-time parameter value inside a module body.
- `hw.hierpath`: Declares an explicit path through instance hierarchies (`[@top, @cpu, @alu]`) used by timing constraints, debug probes, and synthesis floorplanning.

---

## 5. Engineering Rationale & Design Incentives

1. **Why Graph Regions instead of Dominance Trees?**
   Hardware is fundamentally concurrent and cyclic. In an RTL module, two nets can cross-connect across registers or combinational loops (detected as errors only if no register intervenes). Strict SSA dominance trees (like in LLVM) break down when modeling feedback circuits. Graph regions allow natural structural netlisting.
2. **Why Separate `hw` from `comb` and `seq`?**
   Keeping structural netlists separate from logic operations maintains modularity. A structural pass (e.g., module inlining, port renaming, wire deduplication) should not need to know whether the arithmetic inside is signed or unsigned.
3. **Why Non-Destructive Injections (`array_inject`, `struct_inject`)?**
   Hardware languages often assign individual fields: `bundle.valid = 1`. In SSA representation, values are immutable. Inject operations allow clean, functional updates that synthesize directly into wire selection or multiplexer trees without imperative memory mutation.
