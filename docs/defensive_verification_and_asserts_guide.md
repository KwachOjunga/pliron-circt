# Defensive Verification and Asserts Strategy

## 1. Executive Summary & Defensive Philosophy

In software compilers, an unhandled invariant failure typically causes a crash or bad code generation that is quickly caught during testing. In hardware compilation, a subtle invariant failure (e.g. silent bit-truncation, accidental clock-domain crossing, an inverted reset condition, or an undetected combinational loop) can synthesize into broken silicon, resulting in catastrophic failure.

To make `pliron-hw` bulletproof against wrongful usage, the compiler must practice **defense-in-depth**: every operation, type, region, and transformation must assert its preconditions and enforce its semantic contract at multiple layers.

The central architectural principle governing defensive enforcement is:

> **"Each semantic question must have one authoritative owner, and lowering must never silently discard information owned by another dialect. Invariants must be validated early, explicitly, and defensively."**

This document provides an exhaustive guide and actionable checklist for peppering assertions and formal verifications across the entire `pliron-hw` codebase.

---

## 2. The 4-Tier Defensive Architecture

```text
┌────────────────────────────────────────────────────────────────────────┐
│  Tier 0: Rust Type System & Newtype Wrappers                           │
│  - Prevents mixing invalid raw pointers, untyped integers, or handles  │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│  Tier 1: Constructor Preconditions & Panic Guards (`assert!`)          │
│  - Builder methods fail fast if called with illegal arguments          │
│  - Prevents constructing malformed Operation data in memory            │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│  Tier 2: Pliron `Verify` Trait Implementations (`verify_err!`)         │
│  - Validates operation invariants against the Context                  │
│  - Checks operand types, bitwidths, counts, attributes, and regions    │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│  Tier 3: Pass-Level & Structural Graph Invariants                      │
│  - Detects combinational cycles, floating wires, multi-driver nets     │
│  - Validates cross-operation symbol references, CDC, and hierarchies   │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 3. When to Use `assert!` vs. `verify_err!`

| Mechanism | Intended Scope | Failure Consequence | Example Use Case |
| :--- | :--- | :--- | :--- |
| **`assert!` / `debug_assert!`** | Rust API constructor preconditions, internal invariants, and compiler bugs | Immediate panic in debug builds | Calling `Op::new(ctx, ...)` with an empty array of arguments when at least one is required by the Rust API. |
| **`verify_err!`** | Pliron IR verification (`Verify::verify`) | Returns `Err(CompilerError)` with location | User IR has mismatched operand bit widths: `i8` added to `i16`. |
| **`verify_error!`** | Custom diagnostic generation within passes | Returns structured diagnostic | Unresolved symbol reference during module instantiation or undetected CDC. |

**Rule of Thumb**:
- If a Rust developer calls an internal function with completely invalid arguments that violate function contracts, use `assert!`.
- If an IR construct is being validated, whether produced by a parser, lowering pass, or user script, use `verify_err!`.

---

## 4. Operation-by-Operation Verification Checklist

### 4.1 `hw` Dialect Operations

#### `hw.module`
- [ ] **Port Signature Match**: The entry block argument types of the module's graph region must exactly equal the declared input port types.
- [ ] **Terminator Requirement**: The module's single block must terminate with `hw.output`.
- [ ] **Output Port Match**: The operands of `hw.output` must exactly match the declared module output types in count and bit-width.
- [ ] **Symbol Non-Empty**: The module symbol name must not be empty.
- [ ] **Unique Port Names**: Input and output port names must be mutually disjoint and unique.

#### `hw.instance`
- [ ] **Target Resolution**: The referenced target module symbol must exist in the root context.
- [ ] **Input Operand Compatibility**: Operand count and types must exactly match the target module's input port types.
- [ ] **Result Compatibility**: Result count and types must exactly match the target module's output port types.
- [ ] **Non-Empty Instance Name**: The instance identifier attribute must not be empty.
- [ ] **Parameter Binding Check**: All required parameters of the target module must be legally bound with compatible types.

#### `hw.wire`
- [ ] **Type Legitimacy**: Wire result type must be a valid hardware type (`IntegerType`, `ArrayType`, `StructType`, `UnionType`, `InoutType`).
- [ ] **Single Driver Rule**: In pure SSA modules, each wire must have exactly one driving assignment.

#### `hw.bitcast`
- [ ] **Strict Bitwidth Invariance**: Total bitwidth of the input value must **exactly equal** the total bitwidth of the target cast type.
- [ ] **No Width Conversion Guard**: Explicitly assert that `hw.bitcast` is NOT being used to extend or truncate bitwidths (use `comb.zext`, `comb.sext`, or `comb.trunc` instead).
- [ ] **No-Op Guard**: Casting a type to itself should be flagged or canonicalized away.

#### `hw.array_create`, `hw.array_get`, `hw.array_slice`
- [ ] **Uniform Element Types**: In `hw.array_create`, every input operand must have the identical element type.
- [ ] **Index Bounds Checking**: In `hw.array_get` with constant index, assert $0 \le \text{index} < N$.
- [ ] **Slice Bounds Checking**: In `hw.array_slice`, assert $\text{low\_index} + \text{width} \le N$.
- [ ] **Packed vs. Unpacked**: Distinguish indexing semantics across packed bit-vectors and unpacked arrays.

---

### 4.2 `comb` Dialect Operations

#### Arithmetic & Logic (`AddOp`, `SubOp`, `MulOp`, `AndOp`, `OrOp`, `XorOp`)
- [ ] **Width Equality**: $\text{width}(\text{lhs}) == \text{width}(\text{rhs}) == \text{width}(\text{result})$.
- [ ] **Positive Width**: Assert width $> 0$.
- [ ] **No Implicit Sizing**: Reject operations where operands have mismatched widths; operands must be explicitly cast prior to arithmetic.

#### Comparisons (`IcmpOp`)
- [ ] **Operand Width Match**: $\text{width}(\text{lhs}) == \text{width}(\text{rhs})$.
- [ ] **Predicate Validity**: Predicate string must be one of `eq`, `ne`, `slt`, `sle`, `sgt`, `sge`, `ult`, `ule`, `ugt`, `uge`.
- [ ] **Result Type**: Result must be strictly `i1` (`IntegerType::get(ctx, 1, Signless)`).

#### Multiplexers (`MuxOp`)
- [ ] **Condition Type**: Condition operand must be strictly `i1`.
- [ ] **Branch Type Equality**: $\text{width}(\text{true\_val}) == \text{width}(\text{false\_val}) == \text{width}(\text{result})$.
- [ ] **Identical Types**: True and false values must share identical Pliron `TypeHandle`.

#### Explicit Width Conversions (`ZextOp`, `SextOp`, `TruncOp`)
- [ ] **`comb.zext`**: Assert $\text{width}(\text{result}) > \text{width}(\text{input})$.
- [ ] **`comb.sext`**: Assert $\text{width}(\text{result}) > \text{width}(\text{input})$ and verify signedness interpretation.
- [ ] **`comb.trunc`**: Assert $\text{width}(\text{result}) < \text{width}(\text{input})$.

#### Bit Extraction & Concatenation (`ExtractOp`, `ConcatOp`)
- [ ] **Extraction Range Check**: Assert $\text{low\_bit} + \text{width} \le \text{width}(\text{input})$.
- [ ] **Concatenation Width Sum**: Assert $\text{width}(\text{result}) == \sum_{i} \text{width}(\text{input}_i)$.
- [ ] **Non-Empty Inputs**: `ConcatOp` must have at least 2 input operands.

---

### 4.3 `seq` Dialect Operations

#### State Registers (`CompRegOp`, `FirRegOp`, `RegOp`)
- [ ] **Clock Type Enforcement**: The `clk` operand must strictly be `!seq.clock`.
- [ ] **Clock Edge Attribute**: Edge must be explicitly `"posedge"` or `"negedge"`.
- [ ] **Data Width Match**: $\text{type}(\text{input}) == \text{type}(\text{result})$.
- [ ] **Reset Contract (`FirRegOp`)**:
  - Reset signal must be `!seq.reset` or `i1`.
  - Reset value type must exactly equal data input type: $\text{type}(\text{reset\_value}) == \text{type}(\text{input})$.
  - Reset polarity attribute must be strictly `"active_high"` or `"active_low"`.
  - Reset mode must be strictly `"async"` or `"sync"`.
  - Reset dominance: Reset strictly dominates enable.
- [ ] **Enable Signal**: If enable is present, its type must be strictly `i1`.

#### Latches (`seq.latch`)
- [ ] **Level-Sensitive Enable**: Assert enable signal is `i1`.
- [ ] **Data Type Match**: $\text{type}(\text{input}) == \text{type}(\text{result})$.
- [ ] **No Clock Operand**: Explicitly assert that `seq.latch` does NOT accept an edge-triggered clock.

#### Clock Gating (`ClockGateOp`)
- [ ] **Input Clock**: Operand 0 must be `!seq.clock`.
- [ ] **Enable**: Operand 1 must be `i1`.
- [ ] **Result**: Result must be `!seq.clock`.

#### Clock-Domain Crossing (`seq.synchronizer`, `seq.cdc`)
- [ ] **Dual Clock Verification**: Source and destination clock domains must be explicitly defined and distinct.
- [ ] **Type Parity**: Input data type must equal output data type.

#### High-Level Memory (`HLMemOp`, `HLMemReadOp`, `HLMemWriteOp`, `seq.mem`)
- [ ] **Memory Allocation**:
  - Memory depth must be $> 0$.
  - Element bitwidth must be $> 0$.
- [ ] **Memory Read**:
  - Address bitwidth must be $\ge \lceil \log_2(\text{depth}) \rceil$.
  - Result type must match memory element type.
  - Clock operand must be valid `!seq.clock` if read is synchronous.
- [ ] **Memory Write**:
  - Write enable operand must be strictly `i1`.
  - Data operand type must exactly match memory element type.
  - Clock operand must be valid `!seq.clock`.
- [ ] **Hazard Collision Contract**: Read-during-write policy must be one of `"read_first"`, `"write_first"`, `"no_change"`, or `"undefined"`.

---

### 4.4 `sv` Dialect Operations

#### Declarations (`LogicDeclOp`, `WireDeclOp`, `RegDeclOp`)
- [ ] **Non-Empty Target**: Target symbol string must have length $> 0$.
- [ ] **Hardware Type Legitimacy**: Result must be a valid integer or aggregate type.

#### Continuous & Procedural Assignments (`AssignOp`, `AlwaysCombOp`)
- [ ] **Target String Non-Empty**: Target attribute must not be empty.
- [ ] **Assignment Type Match**: In `AssignOp`, operand type must match declared target type.
- [ ] **Combinational Purity**: Expressions feeding `AlwaysCombOp` must not originate from clocked sequential operations.

#### Sequential Process (`AlwaysFfOp`, `AlwaysFfNoResetOp`)
- [ ] **Clock Sensitivity**: Clock operand must be a valid 1-bit or `!seq.clock` value.
- [ ] **Reset Validity (`AlwaysFfOp`)**:
  - Reset signal and reset value must be present.
  - Reset value type must match next-state value type.
  - `reset_polarity` must be `"active_high"` or `"active_low"`.
  - `is_async_reset` must be a boolean attribute.
- [ ] **Non-Blocking Context**: Ensure non-blocking assignment semantics are observed for state targets.

#### Expressions (`BinaryExprOp`, `UnaryExprOp`, `ConstantExprOp`, `MuxExprOp`)
- [ ] **Operator Whitelist**: Binary operator must be in `["+", "-", "*", "/", "%", "&", "|", "^", "==", "!=", "<", "<=", ">", ">="]`.
- [ ] **Unary Operator Whitelist**: Unary operator must be in `["~", "!", "-", "&", "|", "^"]`.
- [ ] **Condition Type in Mux**: `cond` must be 1-bit integer.
- [ ] **Constant Precision**: `ConstantExprOp` value bitwidth must match its declared integer type.

---

## 5. Structural & Pass-Level Assertions

### 5.1 Combinational Cycle Detection (Acyclic Assert)
Before synthesizing or lowering `comb` trees within an `hw.module`:
```rust
pub fn assert_no_combinational_cycles(ctx: &Context, module: ModuleOp) -> Result<()> {
    // Perform Tarjan's or Kosaraju's SCC algorithm on combinational use-def edges.
    // If any cycle exists without an intervening seq.compreg / seq.firreg boundary:
    // return verify_err!(loc, "detected illegal combinational feedback loop");
    Ok(())
}
```

### 5.2 Single-Driver Invariant Check
For each wire or signal in `hw.module`:
```rust
pub fn assert_single_driver(ctx: &Context, wire: Value) -> Result<()> {
    // Ensure the wire has exactly one driving operation unless typed as !hw.inout
    Ok(())
}
```

### 5.3 Clock-Domain Crossing Invariant Check
Validate dataflow paths across clock domains:
```rust
pub fn assert_no_implicit_cdc(ctx: &Context, module: ModuleOp) -> Result<()> {
    // Trace use-def chains from sequential state registers.
    // If a register in clock domain A feeds a register in clock domain B without
    // passing through an explicit seq.synchronizer or CDC barrier:
    // return verify_err!(loc, "detected illegal implicit clock-domain crossing");
    Ok(())
}
```

---

## 6. How to Write Negative Tests (Verifying the Asserts)

Every assertion added to `Verify::verify` or constructor methods must have a corresponding **negative unit test** ensuring that invalid IR is rejected with the exact expected error message.

### Example Negative Test Template:
```rust
#[test]
fn test_verify_rejects_mismatched_binary_widths() {
    let mut ctx = Context::default();
    crate::hw::register(&mut ctx);
    crate::comb::register(&mut ctx);

    // Create 8-bit LHS and 16-bit RHS
    let lhs = create_dummy_value(&mut ctx, 8);
    let rhs = create_dummy_value(&mut ctx, 16);
    let res_ty = IntegerType::get(&mut ctx, 8, Signedness::Signless).into();

    let add_op = AddOp::new(&mut ctx, lhs, rhs, res_ty);
    let verify_res = add_op.get_operation().deref(&ctx).verify(&ctx);

    assert!(verify_res.is_err());
    let err_msg = format!("{}", verify_res.unwrap_err());
    assert!(
        err_msg.contains("operand width mismatch"),
        "Expected error message regarding width mismatch, got: {}",
        err_msg
    );
}
```

---

## 7. Migration & Rollout Plan

1. **Step 1: Constructor Guards**: Add `assert!(!name.is_empty())` and type check guards to all `::new(...)` constructors across `hw`, `comb`, `seq`, and `sv`.
2. **Step 2: Complete `Verify` Implementations**: Implement strict `Verify::verify` for any operations currently using stub verifications (`Ok(())`).
3. **Step 3: Suite of Negative Tests**: Implement unit tests targeting each failure branch to ensure test coverage of defensive checks.
4. **Step 4: Pass Precondition Checking**: Run the verifier at the start and end of every lowering pass to guarantee invariant preservation.
