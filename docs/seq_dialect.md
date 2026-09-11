# SEQ Dialect Specification & Architecture

## 1. Overview & Purpose

The `seq` dialect models synchronous hardware state, clock domains, reset hierarchies, and sequential memory primitives.

In digital hardware systems, state is updated at discrete clock events:
$$S(t+1) = F(S(t), I(t))$$
$$O(t) = G(S(t), I(t))$$

The `seq` dialect provides first-class primitives to capture this behavior explicitly without prematurely lowering registers into technology-specific gate loops or multiplexer feedbacks.

## Why `seq` matters

Combinational logic describes what a circuit computes during a cycle, but it
does not describe where a cycle ends or where a value is retained. Without an
explicit sequential dialect, a compiler has to infer state from feedback
loops, ordinary boolean values, or target-specific Verilog syntax. That loses
the information needed to answer important hardware questions:

- Which values are sampled on the same clock edge?
- Which paths have a one-cycle latency?
- Which signals are clocks rather than ordinary data?
- Where may retiming, clock-domain analysis, reset insertion, or power gating
  legally occur?
- Which register or memory primitive should a synthesis backend select?

`seq` makes the state boundary and clock identity explicit. This lets
`comb` remain a pure zero-cycle dataflow layer while `seq` records the points
where values cross time. A transformation can therefore simplify the logic
between registers without accidentally changing latency or treating a clock
as data.

The dialect is especially important at the boundary between an algorithmic
description and RTL. At that boundary, preserving a register as a register
is more useful than immediately expanding it into a mux and a feedback wire:
the former can still be analyzed for latency, clock domains, reset behavior,
and technology mapping; the latter has already discarded much of that intent.

### Current implementation status

The repository currently implements and registers:

- `!seq.clock` and `!seq.reset` marker types;
- `!seq.mem<depth x element_type>`, an abstract synchronous memory handle;
- `seq.compreg`, an unreset edge-triggered register boundary;
- `seq.firreg`, a resettable edge-triggered register;
- `seq.clock_gate`, a clock-domain-preserving gated-clock operation; and
- `seq.hlmem`, `seq.hlmem_read`, and `seq.hlmem_write` for synchronous memory
  declaration and access.

The operations have structural and operation-specific verifiers. The current
verifiers reject malformed local contracts early, while leaving global
analyses such as clock-domain crossing and address-range proofs to later
passes.

## Verifier contract

Verification answers whether one operation is locally well-formed. It does
not simulate the circuit or prove that a complete design is free of timing
violations. The following rules are enforced:

| Construct | Permitted | Rejected |
|---|---|---|
| `seq.compreg` | A `!seq.clock` operand and an input/result type match | Data used as a clock; mismatched input/result types |
| `seq.firreg` | A `!seq.clock`, a `!seq.reset`, matching input/reset-value/result types, and `active_high` or `active_low` polarity | Wrong clock/reset types; mismatched value types; unknown polarity |
| `seq.clock_gate` | A `!seq.clock`, an `i1` enable, and a `!seq.clock` result | Data clock; non-boolean enable; non-clock result |
| `seq.hlmem` | A `!seq.mem` result and `read-first`, `write-first`, or `undefined` policy | Any other result type or collision policy |
| `seq.hlmem_read` | A clock, memory handle, and result matching the memory element type | Non-clock or non-memory operands; wrong result element type |
| `seq.hlmem_write` | A clock, memory handle, data matching the memory element type, and `i1` enable | Non-clock/non-memory operands; wrong data type; non-boolean enable |

These checks make illegal local states difficult to represent and allow later
passes to rely on the basic temporal and storage contracts. They do not yet
prove that an address can reach every memory entry, that two clocks are in the
same domain, that a gated clock is glitch-free in an implementation, or that
two concurrent memory ports are free of collisions. Those require analyses
over the surrounding module and are deliberately not hidden inside a local
operation verifier.

The verifier failures are tested in `tests/seq_tests.rs`: invalid clock use is
rejected, and reset-policy vocabulary is checked against the documented
lowering choices. Valid tests demonstrate the corresponding permitted forms.

## How `seq` is used

A typical hardware module uses the dialect in four steps:

1. `hw.module` provides input values, including a value with `!seq.clock`
   type.
2. `comb` computes the next-state value from current inputs and registered
   values.
3. `seq.compreg` places an explicit one-cycle state boundary around that
   next-state value.
4. `hw.output` exposes the current register result or a derived combinational
   result.

The current implementation also permits `seq.clock_gate` to produce the clock
value consumed by a register. The intended composition is:

```text
clock: !seq.clock
enable: i1
input: i8
gated_clock = seq.clock_gate clock, enable : !seq.clock
current = seq.compreg gated_clock, input : i8
```

The `seq` operation is a semantic boundary, not a software function call.
`seq.compreg` does not return the newly written value immediately. Its result
is the value held by the register during the current cycle; its input becomes
the held value at the next active edge of its clock. This distinction is what
allows scheduling and latency analyses to reason about the IR.

### Verilog correspondence

The following Verilog illustrates the intended meaning of `seq.compreg`:

```verilog
module register_example (
  input  logic       clk,
  input  logic [7:0] d,
  output logic [7:0] q
);
  always_ff @(posedge clk) begin
    q <= d;
  end
endmodule
```

The nonblocking assignment is significant: the old `q` remains observable
throughout the current edge update, and `d` becomes `q` after the sequential
state transition. A lowering from `seq.compreg` must preserve that one-cycle
state behavior rather than emit a combinational assignment such as `assign
q = d`.

For the current IR composition, a gated clock can be lowered to an integrated
clock-gating cell or, for a simple illustrative model, to a clock expression:

```verilog
module gated_register_example (
  input  logic       clk,
  input  logic       enable,
  input  logic [7:0] d,
  output logic [7:0] q
);
  logic gated_clk;

  // Production ASIC flows generally replace this with an ICG cell.
  assign gated_clk = clk & enable;

  always_ff @(posedge gated_clk) begin
    q <= d;
  end
endmodule
```

The example is useful for understanding the dataflow, but a backend should
not blindly emit `clk & enable` for a glitch-sensitive design. The documented
`seq.clock_gate` contract says that enable is held stable while the input
clock is active; an implementation may therefore choose a latch-based ICG
primitive instead:

```verilog
logic enable_latched;

always_latch begin
  if (!clk)
    enable_latched <= enable;
end

assign gated_clk = clk & enable_latched;
```

The choice between these forms is a lowering and target-legality decision.
The `seq` operation preserves the clock-gating intent so that decision is not
made accidentally by an early boolean rewrite.

### Composition with `comb`

Next-state logic belongs in `comb`; storage belongs in `seq`:

```text
next = comb.add current input : i8
updated = seq.compreg clock next : i8
```

The corresponding RTL shape is:

```verilog
logic [7:0] next;
assign next = current + input;

always_ff @(posedge clk) begin
  current <= next;
end
```

This separation gives optimization passes a clean boundary: arithmetic
rewrites may change `next`, while sequential transformations must preserve the
clock edge and the register latency. It also prevents a pass from confusing
an `i1` enable or data bit with a value of type `!seq.clock`.

---

## 2. Abstraction Level & Temporal Semantics

### Abstraction Level
- **Level**: Register-Transfer Level (RTL) sequential state elements.
- **What it models**:
  - Explicit clock signals (`!seq.clock`) and reset lines (`!seq.reset`).
  - Edge-triggered storage elements (registers with synchronous/asynchronous reset, active-high/low polarity, and clock enable).
  - Clock gating cells (`seq.clock_gate`) for low-power dynamic gating.
  - Multi-port synchronous memories (`seq.hlmem`) with parameterized read-during-write contracts.
- **What it does NOT model**:
  - Continuous-time physical analog waveforms or setup/hold slack numbers (delegated to physical timing constraint dialects).
  - Combinational transformations (delegated to `comb`).

### Clock & Reset Semantics (AGENTS.md §7, §8, §9)
- **Clock Identity**: Clocks are represented by a dedicated type `!seq.clock` rather than generic `i1` wires. This prevents accidental arithmetic manipulation of clock nets and enforces clock-tree integrity.
- **Transition Priority**:
  For an edge-triggered register with reset and enable:
  ```text
  At active clock edge (posedge clock):
      if reset is asserted:
          state <= reset_value
      else if enable is asserted:
          state <= next_value
      else:
          state <= state (hold current value)
  ```
  Reset strictly dominates enable. This contract is invariant across all optimization passes.
- **Synchronous vs. Asynchronous Reset**:
  - **Async Reset**: State transitions to `reset_value` immediately upon assertion of the reset line, independent of clock edges.
  - **Sync Reset**: Reset is evaluated solely at active clock edges.

  For the implemented subset, `seq.compreg` has no reset operand and therefore
  has only the transition contract $Q(t+1) = D(t)$ at the active edge. The
  dedicated `!seq.reset` type is available for future reset-bearing operations;
  it does not by itself impose polarity, synchrony, or priority semantics.

---

## 3. Operations Reference

### `seq.compreg`

**Use case:** Preserve a normal one-cycle state boundary between combinational
next-state logic and the current state. It is the appropriate operation for a
plain D flip-flop with no reset or enable behavior.

- Operands: `(clock: !seq.clock, input: T)`.
- Result: `current: T`.
- Semantics: at the active clock edge, `current(t + 1) = input(t)`.
- Verilog shape: `always_ff @(posedge clk) q <= d;`.

`seq.compreg` is currently the concrete register operation. Its operands are
`(clock, input)` and its result has the declared input value type. Its
operation-specific verifier requires the clock operand to be `!seq.clock` and
requires the input and result types to match.

In Verilog terms, this operation corresponds to an `always_ff` block with a
single positive-edge event control and a nonblocking assignment. Reset and
enable behavior are intentionally absent from this operation; adding either
requires a distinct, explicit contract rather than an implicit attribute.

### `seq.firreg`

**Use case:** Represent a register whose reset behavior must remain visible to
reset analysis and technology mapping. This is useful for FPGA flip-flops,
ASIC cells with reset pins, and FIRRTL-style lowering where reset priority is
part of the functional contract.

- Operands: `(clock, input, reset, reset_value)`.
- Result: `current: T`.
- Attributes: `is_async_reset: bool` and `reset_polarity: StringAttr` with
  `"active_high"` or `"active_low"`.
- Synchronous semantics: at the active edge, reset loads `reset_value`; when
  reset is inactive, input is loaded.
- Asynchronous semantics: asserting reset updates state independently of the
  clock; reset remains higher priority than input.

For an active-high synchronous reset, a lowering has this shape:

```verilog
always_ff @(posedge clk) begin
  if (reset)
    q <= reset_value;
  else
    q <= d;
end
```

For an active-low asynchronous reset:

```verilog
always_ff @(posedge clk or negedge reset_n) begin
  if (!reset_n)
    q <= reset_value;
  else
    q <= d;
end
```

The operation exists to keep these two forms distinguishable until lowering;
they must not be normalized into an unannotated feedback mux.

### `seq.clock_gate`

**Use case:** Represent an intentional clock-domain boundary used to disable
state updates for power or clock-tree reasons. It serves as an analyzable
clock-gating intent rather than ordinary boolean logic.

It models an integrated clock gating (ICG) cell:
  - Operands: `in_clk: !seq.clock`, `enable: i1`.
  - Result: `gated_clk: !seq.clock`.
  - Semantic Contract: Latching enable during clock low phase to eliminate glitches on the output clock.

`seq.clock_gate` is currently the concrete clock-gating operation. Its operands
are `(in_clk, enable)` and its result carries the declared clock result type.
Its operation-specific verifier enforces that `in_clk` and the result are
`!seq.clock` and that `enable` is `i1`.

In Verilog, the abstract operation commonly maps to an integrated clock-gating
cell. A backend may represent that cell directly, for example:

```verilog
my_icg u_icg (
  .clk_in  (clk),
  .enable  (enable),
  .clk_out (gated_clk)
);
```

The cell mapping is preferable to arbitrary boolean rewriting because it
preserves clock-tree intent and gives implementation tools a recognizable
clock-gating primitive.

### `seq.hlmem`

**Use case:** Declare storage that should remain a memory abstraction during
optimization and target selection. The result is a `!seq.mem<depth x T>`
resource, allowing a backend to choose registers, SRAM, block RAM, or a vendor
macro without changing the IR's address space.

- Operands: none.
- Result: one memory handle.
- Attribute: `read_during_write`, normally `"read-first"`, `"write-first"`,
  or `"undefined"`.

### `seq.hlmem_read`

**Use case:** Model a synchronous read port with explicit one-cycle latency,
which is required by most FPGA block RAMs and many SRAM interfaces.

- Operands: `(clock, memory, address)`.
- Result: the memory element type.
- Semantics: sample address at the active edge; produce the selected word one
  cycle later.

Equivalent Verilog shape:

```verilog
always_ff @(posedge clk) begin
  read_data <= mem[address];
end
```

### `seq.hlmem_write`

**Use case:** Model a write port whose update is committed only at a clock
edge and only when enabled. Keeping this as an operation exposes write timing
and allows memory inference.

- Operands: `(clock, memory, address, data, enable)`.
- Result: none.
- Semantics: when `enable` is asserted at the active edge,
  `memory[address]` receives `data`.

Equivalent Verilog shape:

```verilog
always_ff @(posedge clk) begin
  if (write_enable)
    mem[write_address] <= write_data;
end
```

- **Read-During-Write Invariants**:
  - `read-first`: Read returns the old contents of the address prior to write commitment.
  - `write-first`: Read returns the newly written data within the same cycle.
  - `undefined`: Read result is indeterminate on concurrent address collision (used when arbitration guarantees no write collisions).

---

## 4. Engineering Rationale & Design Decisions

1. **Why dedicated `!seq.clock` and `!seq.reset` types?**
   Treating clocks as ordinary `i1` signals invites compiler passes to perform invalid boolean optimizations (e.g. constant-folding a clock or inverting a clock net without understanding edge semantics). A dedicated type ensures clock nets can only connect to clock ports or clock gaters.
2. **Why preserve FIRRTL-style `seq.firreg` before gate-level lowering?**
   Lowering a register with reset into an unbundled combinational multiplexer and a primitive flip-flop destroys synthesis information. Modern FPGA and ASIC standard cell libraries contain dedicated flip-flops with built-in asynchronous reset pins that do not consume LUT logic. Preserving `seq.firreg` ensures high-quality synthesis mapping.
3. **Preservation of Cycle Latency**:
   No pass in the `seq` dialect may alter the cycle latency between two observable registers without user-directed retiming annotations. Cycle count is a hard functional contract.

## 5. Operation usage summary

| Operation | Serves | Typical lowering |
|---|---|---|
| `seq.compreg` | Plain one-cycle state | `always_ff` D flip-flop |
| `seq.firreg` | State with explicit reset policy | Resettable flip-flop |
| `seq.clock_gate` | Clock enable and power intent | ICG cell or target clock gate |
| `seq.hlmem` | Abstract storage declaration | Registers, SRAM, BRAM, or macro |
| `seq.hlmem_read` | Registered memory read | Synchronous read port |
| `seq.hlmem_write` | Edge-triggered memory write | Enabled synchronous write port |

The important usage rule is that `comb` computes values within a cycle and
`seq` names the places where values persist across cycles. A lowering may
change implementation, but it must preserve the operation's state, edge,
reset, latency, and memory collision contracts.
