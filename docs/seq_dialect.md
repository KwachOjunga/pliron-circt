# SEQ Dialect Specification & Architecture

## 1. Overview & Purpose

The `seq` dialect models synchronous hardware state, clock domains, reset hierarchies, and sequential memory primitives.

In digital hardware systems, state is updated at discrete clock events:
$$S(t+1) = F(S(t), I(t))$$
$$O(t) = G(S(t), I(t))$$

The `seq` dialect provides first-class primitives to capture this behavior explicitly without prematurely lowering registers into technology-specific gate loops or multiplexer feedbacks.

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

---

## 3. Operations Reference

### Registers
- `seq.firreg`: Full-featured register modeled after FIRRTL semantics.
  - Operands: `clk: !seq.clock`, `next_val: T`, optional `reset: i1` or `!seq.reset`, optional `reset_val: T`.
  - Attributes: `is_async_reset: bool`, `reset_polarity: StringAttr` ("active_high" or "active_low").
  - Result: `curr_val: T`.
- `seq.compreg`: Simple computation register (D flip-flop) without reset, updating every active clock edge: $Q(t+1) = D(t)$.

### Clock Gating
- `seq.clock_gate`: Models an integrated clock gating (ICG) cell:
  - Operands: `in_clk: !seq.clock`, `enable: i1`.
  - Result: `gated_clk: !seq.clock`.
  - Semantic Contract: Latching enable during clock low phase to eliminate glitches on the output clock.

### Synchronous Memories
- `seq.hlmem`: High-Level Memory declaration defining an array of $D$ words of type $T$.
- `seq.hlmem.read`: Synchronous memory read port sampling address on clock edge with 1-cycle latency.
- `seq.hlmem.write`: Synchronous write port updating word at target address when `enable` is asserted.
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
