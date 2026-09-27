# SystemVerilog Parser Migration: Architectural Strategy, Stubs & Foreseen Challenges

## 1. Overview and Motivation

`pliron-circt` currently incorporates a minimal, custom handwritten lexer and recursive-descent parser in [`src/sv/parser.rs`](../src/sv/parser.rs). While functional for a tiny subset of SystemVerilog (single ANSI module headers, simple wire/logic declarations, basic continuous assignments, and elementary `always_ff` blocks), handwritten front-ends suffer from critical limitations:
1. They deviate from the IEEE 1800-2017 standard grammar.
2. They do not handle realistic SystemVerilog syntactic variations (preprocessor directives, comments, whitespace subtleties, arbitrary parenthesized expressions, multi-line formatting, port declaration variants).
3. Expanding the grammar manually causes an explosion in lexer/parser complexity and fragility.

To address this, the SystemVerilog parser is being transitioned bit by bit to use [`sv-parser`](https://github.com/KwachOjunga/sv-parser), a production-grade, IEEE 1800-2017 compliant parser that produces a Concrete Syntax Tree (CST) and handles macro preprocessing, source tracking, and full lexical analysis.

This document details:
- The architectural bridge between `sv-parser`'s CST and Pliron's hardware dialects (`hw`, `sv`, `seq`).
- The foreseen semantic and engineering challenges during this conversion.
- The stubbed, staged conversion methodology guaranteeing no breakage of existing functionality.
- Semantic correctness and verification requirements in accordance with [`AGENTS.md`](../AGENTS.md).

---

## 2. Deep Dive into Foreseen Challenges

### 2.1 Concrete Syntax Tree (CST) vs. Semantic IR Gap

`sv-parser` emits a lossless Concrete Syntax Tree reflecting the full grammar in Annex A of IEEE 1800-2017. Every keyword, comma, parenthesis, bracket, and syntactic intermediate wrapper exists as an explicit CST node (e.g., `ModuleDeclarationAnsi`, `ListOfPortDeclarations`, `AnsiPortDeclaration`, `PortHeaderOrType`, `NetPortHeader`, etc.).

**Challenges:**
- **Deep nesting traversal**: Accessing the name of a port requires traversing 6 to 10 nested wrapper types. Navigating this without brittle unwrapping or excessive boilerplate requires structured helpers and the `unwrap_node!` / `unwrap_locate!` pattern.
- **Source span preservation**: Mapping CST token `Locate` (byte offset and line number) back to Pliron's `Location` requires mapping offsets accurately into compiler error diagnostics.

### 2.2 Port Semantics: ANSI vs. Non-ANSI & Type Invariants

SystemVerilog modules express interfaces through two fundamentally different styles:
1. **ANSI Port Lists**:
   ```systemverilog
   module alu (
       input  logic [7:0] a,
       input  logic [7:0] b,
       output logic [7:0] sum
   );
   ```
2. **Non-ANSI Port Lists**:
   ```systemverilog
   module alu (a, b, sum);
       input  [7:0] a;
       input  [7:0] b;
       output [7:0] sum;
   ```

**Challenges:**
- In non-ANSI style, ports are declared twice: once in the module port list, and subsequently in the module body as port declarations. The parser must reconcile these declarations before creating `hw.module` block arguments.
- **Clock and Reset Semantics**: As specified in `AGENTS.md`, clocks (`seq.clock`) and resets (`seq.reset`) are semantic hardware resources, not merely `i1` values. The parser must distinguish 1-bit clocks (`clk`, `clock`) and resets (`rst`, `reset`) to emit typed inputs (`ClockType`, `ResetType`) rather than untyped bit signals, while allowing explicit override annotations if needed.

### 2.3 Signal Identity, Out-of-Order Declarations, and SSA Scoping

SystemVerilog is a declarative structural language. Within a module body, signals may be declared after their use in continuous assignments or procedural blocks, or driven across multiple procedural processes.

Pliron hardware dialects (`hw`, `sv`), however, live in an SSA graph region within `hw.module`:
- Operations must dominate their uses.
- Values are defined by specific operations (`sv.logic_decl`, `sv.wire_decl`, `sv.reg_decl`, `sv.assign`, entry block arguments).

**Challenges:**
- **Two-pass elaboration**: The lowerer must perform a pre-scan pass to register all declared ports and module-level nets (`logic`, `wire`, `reg`, `mem`) into a symbol table before lowering assignment expressions and procedural statements that reference them.
- **Output port driving**: In SystemVerilog, output ports can be assigned directly via `assign out = expr;` or via internal logic. In Pliron IR, the module terminates with `hw.output(v1, v2, ...)`. The parser must track the active SSA value driving each output port throughout the module body.

### 2.4 Vector Width & Range Evaluation

Vector bounds in SystemVerilog appear in forms such as `[7:0]`, `[0:15]`, or parameterized ranges `[WIDTH-1:0]`.

**Challenges:**
- In the CST, range bounds are full expression nodes (`ConstantRange(Expression, Expression)`).
- For synthesizable RTL with fixed bounds, the parser needs a robust constant evaluation helper to calculate `msb - lsb + 1` into a concrete `u32` width for `IntegerType::get(ctx, width, Signless)`.
- Support for descending ranges `[high:low]` vs ascending ranges `[low:high]` must be handled consistently without causing underflow or incorrect width calculation.

### 2.5 Procedural Assignment Semantics (`always_comb`, `always_ff`)

SystemVerilog distinguishes:
1. **Continuous assignments** (`assign x = expr;`): Emits `sv.assign`.
2. **Blocking procedural assignments** (`x = expr;`): Inside `always_comb`, represents combinational logic (`sv.always_comb` or `sv.bpa`).
3. **Non-blocking procedural assignments** (`x <= expr;`): Inside `always_ff`, represents registered temporal state transfer (`sv.always_ff`, `sv.always_ff_no_reset`, or `sv.nba`).

**Challenges:**
- **Sensitivity list analysis**: `always_ff @(posedge clk or negedge rst_n)`:
  - The parser must extract the clock event (`posedge clk`) and verify it references a valid clock signal.
  - If a second edge is present (`or negedge rst_n`), it indicates an asynchronous reset; the edge polarity determines `reset_polarity = "active_low"` vs `"active_high"`.
- **Reset condition parsing**: Inside the sequential block:
  ```systemverilog
  if (!rst_n) begin
      q <= 0;
  end else begin
      q <= d;
  end
  ```
  The parser must extract:
  - The reset value expression (`0`).
  - The next-state expression (`d`).
  - The target signal (`q`).
- **Verifying against `AGENTS.md`**: As mandated by `AGENTS.md`, state transitions must be explicit:
  - What state exists.
  - Clock and edge polarity.
  - Reset behavior (synchronous vs asynchronous, polarity, priority).

### 2.6 Expression Lowering & Operator Precedence

SystemVerilog expressions include:
- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Bitwise: `&`, `|`, `^`, `~^`, `^~`
- Logical: `&&`, `||`, `!`
- Relational & Equality: `==`, `!=`, `<`, `<=`, `>`, `>=`
- Reduction: `&a`, `|a`, `^a`
- Conditional / Multiplexer: `cond ? expr_true : expr_false` $\rightarrow$ `sv.mux_expr`
- Concatenations: `{a, b, c}` $\rightarrow$ `sv.concat_expr`
- Slices: `a[7:4]` $\rightarrow$ `sv.slice_expr`
- Indexing: `a[i]` $\rightarrow$ `sv.index_expr`
- Sized & unbased numeric literals: `'0`, `32'd42`, `8'hFF`, `4'b1010` $\rightarrow$ `sv.constant_expr`

**Challenges:**
- CST expression nodes are nested binary/unary trees (`BinaryExpression`, `UnaryExpression`, etc.).
- The lowering must recursively evaluate sub-expressions, inserting `sv.binary_expr`, `sv.unary_expr`, etc., into the basic block and returning the resulting SSA `Value`.
- Type compatibility: Operations must ensure operands have compatible `IntegerType` bit widths, matching Pliron verification rules.

### 2.7 Structural Instantiations & Memory Resources

- **Module instances** (`child_mod u1 (.a(x), .b(y));` or `child_mod u1 (x, y);`):
  Must lower to `sv.instance` with target symbol, instance name, and resolved operand SSA values.
- **Memory declarations** (`logic [7:0] mem [0:15];`):
  Must lower to `sv.mem_decl` with `MemoryType(element_type, depth)`.

### 2.8 Unsupported Constructs & Graceful Diagnostics

Real-world SystemVerilog files contain non-synthesizable constructs (e.g. initial blocks, system tasks `$display`, delays `#10`, interface blocks, packages).
Instead of crashing or dropping constructs silently, the parser must:
- Implement clear stub handlers.
- Emit meaningful Pliron verification errors (`verify_err!(loc, "unsupported SystemVerilog construct: ...")`).

---

## 3. Staged Implementation Plan with Stubs

To ensure continuous semantic validity and keep the test suite green at every commit, the migration is executed in the following structured steps:

```
Step 1: Scaffolding, CST Bridge & Stubs
        (Integrate sv_parser, parse_sv_str wrapper, define SvLowerer with stub handlers)
        ↓
Step 2: Module Header & Port List Lowering
        (ANSI port parsing, ClockType/ResetType detection, hw.module argument creation)
        ↓
Step 3: Signal Declarations & Scope Symbol Table
        (logic, wire, reg, mem declarations inserted into SSA basic block)
        ↓
Step 4: Expression Lowering Pipeline
        (constant literals, identifiers, unary, binary, ternary mux, slice, concat)
        ↓
Step 5: Continuous Assignments & Combinational Blocks
        (assign -> sv.assign, always_comb -> sv.always_comb)
        ↓
Step 6: Sequential Blocks (always_ff)
        (clock/reset edge extraction, sync/async branching, sv.always_ff / sv.always_ff_no_reset)
        ↓
Step 7: Instances, Array Memory, and Comprehensive Semantic Tests
        (sv.instance, sv.mem_decl, edge-case tests, full test suite validation)
```

Each step is committed independently with accompanying unit/integration tests confirming semantic correctness.
