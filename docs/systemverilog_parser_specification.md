# SystemVerilog Parser Specification & Capabilities

## 1. Overview & Architecture

The SystemVerilog front-end in `pliron-hw` (`src/sv/parser.rs`) implements a lightweight, hand-crafted recursive-descent lexer and parser. Its primary objective is to ingest synthesizable SystemVerilog RTL modules directly into verified Pliron hardware Intermediate Representation (IR), bridging textual hardware descriptions into structured representations across the `sv` and `hw` dialects.

The parser does **not** attempt full IEEE 1800 compliance. Instead, it defines a deterministic, synthesizable subset focused on:
- Module interface extraction (`hw.module`, `hw.output`).
- Signal and net declarations (`sv.logic_decl`, `sv.wire_decl`, `sv.reg_decl`).
- Continuous concurrent assignments (`sv.assign`).
- Procedural combinational blocks (`sv.always_comb`).
- Procedural sequential clock/reset processes (`sv.always_ff`, `sv.always_ff_no_reset`).
- Sized literals and expression trees (`sv.binary_expr`, `sv.unary_expr`, `sv.constant_expr`).

---

## 2. Lexical Grammar (Tokens)

The lexer (`Lexer<'a>`) transforms raw UTF-8 SystemVerilog source text into a sequential token stream (`Vec<Token>`).

### 2.1 Whitespace & Comments
- **Whitespace**: Standard whitespace characters (` `, `\t`, `\r`, `\n`) are ignored between tokens.
- **Line Comments**: Single-line comments starting with `//` discard all characters until the terminating newline `\n`.
- **Block Comments**: Multi-line comments enclosed in `/* ... */` are skipped, handling nested characters until the matching `*/`.

### 2.2 Keywords
The following reserved SystemVerilog keywords are recognized:
```text
module        endmodule     input         output        inout
logic         wire          reg           assign        always_ff
always_comb   posedge       negedge       begin         end
if            else
```

### 2.3 Identifiers
- An identifier matches `[a-zA-Z_][a-zA-Z0-9_$]*`.
- Identifiers that match keywords are tokenized as the corresponding keyword token; all other identifiers produce `Token::Ident(String)`.

### 2.4 Numeric Literals
The lexer parses two categories of integer literals into `Token::Number { value: u64, width: u32 }`:
1. **Unsized Decimal Literals**:
   - Matches a sequence of decimal digits `[0-9]+` (with optional embedded underscores `_`).
   - Default bit width is assigned as **32** bits (`IntegerType::get(ctx, 32, Signless)`).
   - Example: `42`, `1000`.
2. **Sized Base Literals**:
   - Matches `<width>'<base><value>`, where:
     - `<width>` is a non-zero decimal integer.
     - `<base>` is one of `'d`, `'D` (decimal), `'h`, `'H` (hexadecimal), `'b`, `'B` (binary), or `'o`, `'O` (octal).
     - `<value>` consists of digits valid for the chosen base.
   - Example: `8'hFF` (width = 8, value = 255), `1'b0` (width = 1, value = 0), `16'd1024` (width = 16, value = 1024).

### 2.5 Operators & Punctuation
The following symbols are tokenized:
```text
(    )    [    ]    {    }    ;    :    ,    @
=    <=   +    -    *    /    &    |    ^    ~    !
==   !=   <    >    >=   <=
```

---

## 3. Syntactic Grammar & Supported Constructs

### 3.1 Module Definition
```text
module <module_name> (
  [ <port_decl> { , <port_decl> } ]
);
  { <module_item> }
endmodule
```
- **Module Name**: Extracted and recorded as the symbol name of `hw.module`.
- **Port Declarations**: ANSI-style port lists are supported.
  - Syntax: `[input | output | inout] [logic | wire | reg] [ [msb:lsb] ] <ident>`
  - If direction is omitted, it defaults to `input`.
  - If type is omitted, it defaults to signless `logic`.
  - Width ranges `[msb:lsb]` calculate bit width as `(msb - lsb + 1)`. If omitted, the default width is `1` bit.
  - **SSA Mapping**:
    - Input ports become entry block arguments of the `hw.module` graph region. Port identifiers are bound to SSA value names via `.set_name()`.
    - Output ports are collected and driven by the terminating `hw.output` operation at the end of the module body.

### 3.2 Signal & Net Declarations
Local signal declarations within the module body:
```text
logic [ [msb:lsb] ] <name> ;
wire  [ [msb:lsb] ] <name> ;
reg   [ [msb:lsb] ] <name> ;
```
- **`logic` Declaration**: Emits `sv.logic_decl` with result type `iN` and attribute `logic_target = "<name>"`.
- **`wire` Declaration**: Emits `sv.wire_decl` with result type `iN` and attribute `wire_target = "<name>"`.
- **`reg` Declaration**: Emits `sv.reg_decl` with result type `iN` and attribute `reg_target = "<name>"`.
- The newly declared signal is registered in the parser's local identifier scope for resolution in subsequent expressions.

### 3.3 Continuous Assignments
```text
assign <target_name> = <expression> ;
```
- `<target_name>`: Must refer to a previously declared net, variable, or output port.
- `<expression>`: Evaluated recursively into an SSA `Value`.
- Emits an `sv.assign` operation with operand `<expression>` and attribute `target = "<target_name>"`.
- The result of `sv.assign` updates the identifier scope for `<target_name>`.

### 3.4 Procedural Combinational Blocks (`always_comb`)
```text
always_comb begin
  <target_name> = <expression> ;
end
```
*(Also accepts single-statement forms without `begin ... end`)*
- Parses a blocking assignment (`=`).
- Emits an `sv.always_comb` operation with:
  - Operand: Evaluated SSA `Value` of `<expression>`.
  - Attribute: `comb_target = "<target_name>"`.
- **Semantics**: Represents combinational logic evaluated whenever any input in `<expression>` changes.

### 3.5 Procedural Sequential Blocks (`always_ff`)

#### 3.5.1 Unclocked / Plain Register (No Reset)
```text
always_ff @(posedge <clock_name>) begin
  <target_name> <= <next_expr> ;
end
```
- Sensitivity: `@(posedge <clk>)`.
- Statement: Non-blocking assignment (`<=`).
- Emits `sv.always_ff_no_reset` with:
  - Operand 0: SSA clock value (`clk_val`).
  - Operand 1: Evaluated SSA value of `<next_expr>`.
  - Attribute: `ff_nr_target = "<target_name>"`.

#### 3.5.2 Register with Asynchronous / Synchronous Reset
```text
always_ff @(posedge <clock_name> or posedge <reset_name>) begin
  if (<reset_name>) begin
    <target_name> <= <reset_value_expr> ;
  end else begin
    <target_name> <= <next_expr> ;
  end
end
```
*(Also supports `@(posedge <clock_name>)` with synchronous reset condition `if (<reset>) ...`)*
- Sensitivity list: Parses clock edge and optional reset edge (`posedge` or `negedge`).
- Reset branch: Extracted from `if (<reset_expr>)` matching `<target_name> <= <reset_val>`.
- Next-state branch: Extracted from `else` matching `<target_name> <= <next_val>`.
- Emits `sv.always_ff` with:
  - Operand 0: SSA clock value.
  - Operand 1: Evaluated SSA next-state value.
  - Operand 2: SSA reset signal value.
  - Operand 3: Evaluated SSA reset value.
  - Attributes: `target`, `is_async_reset = bool`, `reset_polarity = "active_high" | "active_low"`.

---

## 4. Expression Grammar & Evaluation

Expressions are parsed via recursive descent into a hierarchy of `sv` expression operations:

### 4.1 Operator Precedence (Current Parser Model)
The current expression parser uses a right-leaning binary expression structure with the following operators:
- **Unary Operators**:
  - `~` (Bitwise NOT) $\rightarrow$ `sv.unary_expr` with operator `"~"`.
  - `!` (Logical NOT) $\rightarrow$ `sv.unary_expr` with operator `"!"`.
- **Binary Operators**:
  - Arithmetic: `+`, `-`, `*`, `/`.
  - Bitwise: `&`, `|`, `^`.
  - Relational / Equality: `==`, `!=`, `<`, `<=`, `>`, `>=`.
  - Generated Op: `sv.binary_expr` with attributes `operator`, `lhs`, `rhs`, and computed result type.
- **Parentheses**: `( <expression> )` forces grouping.
- **Primary Atoms**:
  - Variable Identifiers: Resolved against the symbol scope.
  - Numeric Constants: Emits `sv.constant_expr` with value attribute, width attribute, and signless integer type.

---

## 5. Comprehensive Summary: Supported vs. Unsupported Constructs

| SystemVerilog Construct | Status | Pliron IR Mapping | Notes |
| :--- | :---: | :--- | :--- |
| **Module Header (ANSI ports)** | Supported | `hw.module` | `input`, `output`, `inout` with bit-width ranges |
| **Non-ANSI Module Header** | Not Supported | - | E.g. `module foo(a, b); input a;` |
| **Logic / Wire / Reg Decls** | Supported | `sv.logic_decl`, `sv.wire_decl`, `sv.reg_decl` | Supports 1D packed bit-vectors `[msb:lsb]` |
| **Continuous Assignment (`assign`)** | Supported | `sv.assign` | Continuous net assignment |
| **`always_comb`** | Supported | `sv.always_comb` | Blocking assignments to single target |
| **`always_ff` (No Reset)** | Supported | `sv.always_ff_no_reset` | Single-edge triggered non-blocking assign |
| **`always_ff` (Async/Sync Reset)** | Supported | `sv.always_ff` | Reset polarity and async status preserved |
| **Basic Binary Expressions** | Supported | `sv.binary_expr` | `+`, `-`, `*`, `/`, `&`, `|`, `^`, `==`, `!=`, `<`, `<=`, `>`, `>=` |
| **Basic Unary Expressions** | Supported | `sv.unary_expr` | `~`, `!` |
| **Constant Literals** | Supported | `sv.constant_expr` | Unsized decimal and sized base (`8'hFF`, `1'b0`) |
| **Ternary Operator (`? :`)** | Defined in IR | `sv.mux_expr` | Op supported in IR; AST parser pending |
| **Concatenation (`{a, b}`)** | Defined in IR | `sv.concat_expr` | Op supported in IR; AST parser pending |
| **Bit Slicing (`a[msb:lsb]`)** | Defined in IR | `sv.slice_expr` | Op supported in IR; AST parser pending |
| **Array Indexing (`a[idx]`)** | Defined in IR | `sv.index_expr` | Op supported in IR; AST parser pending |
| **Case Statements (`case`)** | Defined in IR | `sv.case` | Op supported in IR; AST parser pending |
| **Module Instantiations** | Defined in IR | `sv.instance`, `hw.instance` | Op supported in IR; AST parser pending |
| **Packages (`package`)** | Not Supported | - | Planned for future elaboration layer |
| **Interfaces & Modports** | Not Supported | - | Planned for future protocol modeling |
| **Generate Blocks (`generate`)** | Not Supported | - | Compile-time elaboration feature |
| **Procedural Loops (`for`, `while`)** | Not Supported | - | Not allowed in standard synthesizable IR |
| **Delays (`#5`) & Timing** | Not Supported | - | Non-synthesizable simulation constructs |
| **User Types (`typedef struct`)** | Supported in `hw` | `hw.struct`, `hw.enum` | Direct SV syntax parsing pending |
| **SVA / Assertions (`assert property`)** | Not Supported | - | Planned for verification dialect |
| **Classes & OOP** | Excluded by Design | - | Non-synthesizable / testbench constructs |

---

## 6. Error Reporting & Diagnostic Guarantees

1. **Unresolved Identifier**: Accessing an identifier that has not been declared as an input port or local signal returns a descriptive error:
   ```text
   unresolved identifier '<name>' at <location>
   ```
2. **Syntax Expectation Failures**: Encountering an unexpected token emits a structured parser error identifying the expected token and the actual token encountered.
3. **Module Verification**: Upon completing parsing, `hw.module` and all child `sv` operations are immediately subject to Pliron's `Verify::verify` contract before being returned to caller code.
