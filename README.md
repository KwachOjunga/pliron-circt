# pliron-hw

Hardware dialect ecosystem for [pliron](https://github.com/pliron-org/pliron), inspired by CIRCT.

## Dialects

- **`hw`**: Structural hardware dialect (modules, instances, ports, bitvectors, arrays, inouts).
- **`comb`**: Pure combinational logic operations (add, mux, bitwise gates, slice/concat).
- **`seq`**: Sequential hardware state (clocks, resets, registers, memories).
- **`sv`**: SystemVerilog procedural constructs and AST generation for synthesizable emission.
