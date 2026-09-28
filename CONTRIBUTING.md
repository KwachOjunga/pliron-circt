Thank you for considering contributing to pliron-circt!

Understand that when designing hardware dialects, one should think in terms of the 
hardware abstraction and transformations the dialect needs to enable, rather than 
simply encoding HDL syntax.

Following the CIRCT approach, a dialect should define precise 
semantics for hardware concepts such as combinational logic, sequential state, types 
and widths, clocks and resets, memories, hierarchy, and instances, while exposing the 
invariants required for verification and transformation.

Operations ought to represent meaningful hardware constructs with well-defined 
contracts so that analyses and lowering passes can reason about them independently of 
any particular HDL or target. 

The objective is to make the IR a reliable semantic layer between high-level hardware 
descriptions and progressively lower representations such as RTL, SystemVerilog, or 
technology-specific netlists.

Much of what should or should not be done is mired in the design details of the IR.
Further design guidance is available in:

- [`docs/pliron_hardware_ir_constraints.md`](docs/pliron_hardware_ir_constraints.md): pliron constraints and their impact on hardware abstractions;
- [`docs/hardware_layout_interface.md`](docs/hardware_layout_interface.md): hardware physical layout, bitwidth calculations, and aggregate packing;
- [`docs/sv_dialect.md`](docs/sv_dialect.md): SV operation contracts;
- [`docs/sv_transformations.md`](docs/sv_transformations.md): legal and illegal transformations between semantic dialects and SV emission intent.
