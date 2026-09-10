# COMB Dialect Specification & Architecture

## 1. Overview & Purpose

The `comb` dialect provides a formal, zero-delay representation for combinational logic operations, integer arithmetic, bitwise Boolean transformations, multiplexers, and bit-level extractions.

Combinational logic compute output values purely as functions of instantaneous input values:
$$O = F(I)$$

The dialect adheres strictly to the mathematical contracts defined in `AGENTS.md` (§11 Combinational Semantics).

---

## 2. Abstraction Level & Semantic Model

### Abstraction Level
- **Level**: Functional / combinational dataflow.
- **What it models**:
  - Bit-precise Boolean operations (`and`, `or`, `xor`).
  - Modular integer arithmetic with explicit signed/unsigned semantics (`add`, `sub`, `mul`, `divu`, `divs`, `modu`, `mods`, `shl`, `shru`, `shrs`).
  - Multiplexing (`mux`) and multi-predicate integer comparisons (`icmp`).
  - Bit slicing, concatenation, and replication.
- **What it does NOT model**:
  - Implementation architectures (it expresses mathematical *addition*, not whether an adder is ripple-carry, carry-lookahead, or Brent-Kung).
  - Delay or propagation time (combinational delay is strictly zero in the functional semantics).
  - Clocks, resets, or registers (delegated to `seq`).

### Causal Order & Purity
- **Purity**: All `comb` operations are side-effect free and referentially transparent (`Pure` trait). Any operation whose operands are constant can be folded. Any dead `comb` operation can be eliminated safely.
- **Causal Order**: Order in `comb` is strictly governed by dataflow dependencies (directed acyclic graph of def-use relationships). Combinational cycles (without intervening registers) are mathematically illegal and rejected by verifiers.

---

## 3. Operations & Semantic Contracts

### Arithmetic Operations
All arithmetic operations operate on `iN` integers and produce an `iN` result with wrap-around modulo $2^N$ arithmetic:
- `comb.add`: Two's-complement addition $(A + B) \pmod{2^N}$. Commutative and associative.
- `comb.sub`: Two's-complement subtraction $(A - B) \pmod{2^N}$.
- `comb.mul`: Two's-complement multiplication $(A \times B) \pmod{2^N}$. Commutative and associative.
- `comb.divu` / `comb.divs`: Unsigned and signed integer division. Division by zero yields an undefined/all-ones result according to hardware specifications.
- `comb.modu` / `comb.mods`: Unsigned and signed integer remainder.
- `comb.shl`, `comb.shru`, `comb.shrs`: Logical shift-left, logical shift-right, and arithmetic (sign-extending) shift-right. Shift amount is bounded by bitwidth $N$.

### Bitwise & Logical Operations
- `comb.and`: Bitwise conjunction $A \land B$.
- `comb.or`: Bitwise disjunction $A \lor B$.
- `comb.xor`: Bitwise exclusive-or $A \oplus B$. Inversion is canonicalized as $A \oplus \mathbf{1}_{N}$.

### Multiplexers & Comparisons
- `comb.mux`: Three-operand selection:
  $$\text{mux}(c, \text{true\_val}, \text{false\_val}) = c \;?\; \text{true\_val} : \text{false\_val}$$
  where $c$ is of type `i1`, and both branch values have identical type $T$.
- `comb.icmp`: Integer comparison producing an `i1` condition. Supports predicates:
  - `eq`, `ne` (Equality / inequality)
  - `slt`, `sle`, `sgt`, `sge` (Signed ordering)
  - `ult`, `ule`, `ugt`, `uge` (Unsigned ordering)

### Bit Manipulations
- `comb.concat`: Concatenates two or more values of widths $W_1, W_2, \dots$ into a single integer of width $\sum W_i$.
- `comb.extract`: Extracts a contiguous bit-slice of width $W$ from offset $O$.
- `comb.replicate`: Replicates an $N$-bit value $K$ times to produce an integer of width $K \times N$.
- `comb.parity`: Computes single-bit odd/even reduction parity across all bits.

---

## 4. Engineering Rationale & Design Incentives

1. **Why Wrap-Around Two's Complement Arithmetic?**
   Hardware integer buses do not trap or throw runtime exceptions on overflow. By defining `add`, `sub`, and `mul` as standard modulo $2^N$ operations, transformations such as bit-slice pushdown, sign extension elimination, and algebraic re-association are guaranteed sound.
2. **Why Separate `divu`/`divs` and `shru`/`shrs`?**
   Signedness in hardware is not an attribute of the wire or data bus; it is an attribute of the mathematical operator applied to that bus. The type system uses signless `iN` (following CIRCT and MLIR conventions), while operations encode signedness semantics explicitly.
3. **Canonicalization Contracts**:
   - Commutative normalization: In commutative operations (`add`, `mul`, `and`, `or`, `xor`), constant operands are sorted to the right.
   - Subtraction of constants is canonicalized to addition of the two's-complement negation.
   - Mux with identical true and false values folds directly to the branch value: $\text{mux}(c, v, v) \implies v$.
   - Mux with boolean condition folds: $\text{mux}(c, 1, 0) \implies c$.
