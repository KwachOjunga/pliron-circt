# Hardware Dialect Examples

This catalog shows the same design ideas at the abstraction level where they
are easiest to verify and transform. The examples use pseudo-IR where the
syntax is illustrative; operation names and contracts match the implemented
dialects.

## 1. Pure arithmetic belongs in `comb`

```text
%sum = comb.add %a, %b : i8
```

Use `comb` because this is a pure function of current inputs. It has no state,
clock, reset, or storage identity. A compiler can fold constants, reassociate
safe arithmetic, or lower it to an adder without changing cycle latency.

SystemVerilog emission:

```systemverilog
assign sum = a + b;
```

Do not model the adder as `seq.compreg`: that would incorrectly add a cycle.
Do not model it as `hw.wire`: a wire gives identity but does not describe the
arithmetic function.

## 2. Conditional data selection belongs in `comb`

```text
%selected = comb.mux %enable, %new_value, %old_value : i16
```

`comb.mux` expresses a value-level choice. It is useful before scheduling
because the optimizer can simplify identical branches and preserve width
semantics. The equivalent emitted form is:

```systemverilog
assign selected = enable ? new_value : old_value;
```

The enable is data here. It must not be confused with a clock gate, where the
same-looking boolean controls whether clock edges reach state.

## 3. A plain register belongs in `seq`

```text
%state = seq.compreg %clock, %next : i32
```

Use `seq` because the important fact is the state transition:

```text
state(t + 1) = next(t)
```

That fact enables latency analysis, retiming checks, and clock-domain checks.
The SV lowering is:

```systemverilog
always_ff @(posedge clock) begin
  state <= next;
end
```

A direct `assign state = next` would destroy the temporal boundary.

## 4. Reset behavior must remain in `seq`

```text
%state = seq.firreg %clock, %next, %reset, %zero
  {is_async_reset = false, reset_polarity = "active_high"} : i32
```

The operation preserves reset priority and synchronous behavior until target
mapping. A legal SV form is:

```systemverilog
always_ff @(posedge clock) begin
  if (reset)
    state <= zero;
  else
    state <= next;
end
```

Keeping reset in `seq` lets an FPGA or ASIC backend choose a native resettable
flip-flop instead of prematurely expanding the reset into a mux.

## 5. A clock gate is not a boolean `comb` expression

```text
%gated = seq.clock_gate %clock, %enable : !seq.clock
```

Use `seq.clock_gate` because the enable must be stable during the active clock
phase and the result retains clock identity. A naive combinational expression:

```text
%gated = comb.and %clock, %enable : i1
```

loses clock-domain meaning and can introduce glitches. A target backend may
lower the seq operation to an integrated clock-gating cell:

```systemverilog
my_icg u_gate (.clk_in(clock), .enable(enable), .clk_out(gated));
```

## 6. Memory identity belongs in `seq`

```text
%mem = seq.hlmem {read_during_write = "read-first"}
  : !seq.mem<256 x i32>
%data = seq.hlmem_read %clock, %mem, %address : i32
seq.hlmem_write %clock, %mem, %address, %write_data, %write_enable
```

A memory is not merely an array of unrelated registers. The dialect preserves
depth, element type, port timing, and collision policy so lowering can choose
SRAM, block RAM, or registers.

```systemverilog
logic [31:0] mem [0:255];
always_ff @(posedge clock) begin
  if (write_enable)
    mem[address] <= write_data;
  read_data <= mem[address];
end
```

The collision policy determines whether the read sees old data, new data, or
an undefined value. A backend must not silently select a different behavior.

## 7. Structural identity belongs in `hw`

```text
%named = hw.wire "debug_bus", %value : i8
```

Use `hw.wire` when a net has hardware identity, debug significance, or a name
that must survive transformation. Use `comb` when only the value function
matters. Use `sv.assign` only after the target-facing name and emission form
have been chosen.

```systemverilog
wire [7:0] debug_bus;
assign debug_bus = value;
```

The distinction allows an optimizer to remove an unnamed temporary while
preserving a named physical or debug net.

## 8. Module hierarchy belongs in `hw`

```text
hw.module @top { ...
  %child = hw.instance "u_alu", @alu, %a, %b : i8
}
```

`hw` owns symbol identity, ports, instance connectivity, and hierarchy. The
SV dialect can later emit:

```systemverilog
alu u_alu (.a(a), .b(b), .result(result));
```

Do not encode hierarchy only as an SV string. Keeping it in `hw` enables
instance resolution, interface checking, and hierarchy-preserving transforms
before source emission.

## 9. Aggregates belong in `hw`

```text
%packet = hw.struct_create %valid, %payload : !hw.struct<valid: i1, payload: i32>
%payload = hw.struct_extract %packet, "payload" : i32
```

The structural dialect preserves field names and layout. `comb` can then
compute over extracted scalar values, while `sv` can choose a packed struct,
bit vector, or explicit ports at emission time.

```systemverilog
typedef struct packed {
  logic       valid;
  logic [31:0] payload;
} packet_t;
```

Lowering directly to a bit slice too early can discard field identity and make
interface evolution harder.

## 10. Enum meaning belongs in `hw`, comparison belongs in `comb`

```text
!hw.enum<State: i2, idle = 0, busy = 1, done = 2>
%is_done = comb.icmp "eq", %state, %done : i2 -> i1
```

`hw` preserves the named domain and explicit encodings. `comb` expresses the
pure comparison. `seq` is only needed where the state is stored:

```text
%state = seq.compreg %clock, %next_state : !hw.enum<State, ...>
```

This separation lets verification check illegal encodings while allowing
combinational comparison folding.

## 11. SV emission is a target boundary

After semantic verification and lowering:

```text
%next = comb.add %a, %b : i8
%state = seq.compreg %clock, %next : i8
sv.assign "next_value", %next
sv.always_ff_no_reset "state", %clock, %next
```

The `sv` operations serve deterministic source emission. They should not be
used as a replacement for the semantic source operations because an SV target
name does not itself prove latency, reset priority, or memory behavior.

The implemented module pass `lower_module_registers` walks a verified
`hw.module` and creates SV register intent before `hw.output`. The printer then
renders the supported SV operations as SystemVerilog source.

## 12. Why boundaries improve transformations

Consider a counter:

```text
%incremented = comb.add %count, %one : i32
%count_next = comb.mux %enable, %incremented, %count : i32
%count = seq.firreg %clock, %count_next, %reset, %zero
```

Each dialect exposes a different legal transformation set:

- `comb` may simplify the add or mux without changing cycle count.
- `seq` may analyze reset priority and register latency.
- `hw` may preserve the counter's module port and wire identity.
- `sv` may choose `always_ff`, declarations, and source names.

A monolithic HDL-like operation would make these transformations harder to
state and easier to get wrong.

## 13. Invalid abstraction choices

### Register as a continuous assignment

```systemverilog
assign q = d;
```

This is not equivalent to a D register. It removes state and latency.

### Clock as an ordinary boolean

```text
%gated = comb.and %clock, %enable : i1
```

This erases clock identity and glitch assumptions.

### Memory as arbitrary array logic

```text
logic [31:0] mem [0:255];
assign read_data = mem[address];
```

This changes a synchronous one-cycle read into an asynchronous read.

### Reset as an untyped string

```text
sv.always_ff {reset = "maybe_async"}
```

This cannot support reliable event-control lowering. Reset mode and polarity
must be typed or verified attributes.

## 14. Example progression

A useful development workflow is:

```text
1. Build hierarchy and ports in hw.
2. Express pure next-state logic in comb.
3. Add state, reset, clocks, and memories in seq.
4. Verify local and module-wide hardware contracts.
5. Lower supported operations to sv intent.
6. Canonicalize without crossing temporal barriers.
7. Render or export SystemVerilog.
```

The example progression is also a checklist for deciding where a new hardware
concept belongs.
