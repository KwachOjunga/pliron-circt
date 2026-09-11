# `comb` Dialect: Current Shortcomings

This document records limitations of the current `comb` implementation. It is
intentionally separate from the semantic specification so that the intended
contract and the behavior currently enforced by the code are not confused.

## 1. Semantic verification is incomplete

All operations in `src/comb/ops.rs` currently use the generated `succ`
verifier. The operation interfaces check structural facts such as operand,
result, and region counts where an interface is present, but they do not yet
check the dialect-specific semantic invariants described in
[`comb_dialect.md`](comb_dialect.md).

The following malformed constructions can therefore be created through the
public constructors and may pass module verification:

- binary operations with operands of different types or widths;
- arithmetic operations whose result type does not match the operand width;
- unary operations whose result width differs from the input width;
- reductions whose result is not a one-bit integer;
- `comb.mux` with a condition other than `i1`;
- `comb.mux` with branch values of different types or a result type different
  from the branch type;
- `comb.icmp` with operands of different widths or a result other than `i1`;
- `comb.concat` whose result width is not the sum of its operand widths;
- an empty variadic `comb.and`, `comb.or`, or `comb.xor`;
- variadic bitwise operations with mixed operand widths;
- `comb.extract` whose low bit and result width exceed the source width;
- `comb.replicate` with a zero count or a result width inconsistent with the
  repeated input width; and
- `comb.extract` or `comb.replicate` attributes whose integer values are not
  represented with a clearly constrained attribute type.

The constructors accept an arbitrary result `TypeHandle`, so constructor use
does not establish these invariants. A future implementation should add
operation-specific verifiers and negative tests for each invariant.

## 2. Integer semantics are not executable

The implementation defines operation names and operand/result structure, but
does not yet provide an evaluator, constant folder, interpreter, or lowering
pass for the operations. Consequently, the following statements remain
documentation-level contracts rather than executable behavior:

- modular overflow for `add`, `sub`, and `mul`;
- signed versus unsigned division and remainder;
- division or remainder by zero;
- shift amounts greater than or equal to the value width;
- arithmetic sign extension for `shrs`; and
- bit ordering for concatenation and extraction.

The division-by-zero rule also needs clarification. The specification currently
mentions both an undefined result and an all-ones result. Those are different
contracts: the former permits a result to remain unknown or unconstrained,
while the latter requires a particular bit pattern.

## 3. Purity and cycle claims are not encoded

The specification describes all `comb` operations as pure and says that
combinational cycles are rejected. The current operation declarations do not
implement a purity/side-effect interface, and this repository does not yet
provide a `comb`-specific cycle verifier.

This leaves several transformation questions unresolved:

- whether generic dead-code elimination may treat every operation as pure;
- whether common-subexpression elimination is valid across all `comb` ops;
- whether a value denotes an ordinary computed snapshot or a continuously
  driven signal; and
- which graph or dominance checks are required for cycle detection inside an
  `hw.module` graph region.

The dialect should encode purity using the framework's supported interface and
define where cycle verification lives before transformations rely on these
properties.

## 4. Canonicalization is only a design proposal

The specification lists commutative operand ordering, subtraction rewriting,
and mux simplifications, but no rewrite patterns or canonicalization hooks are
implemented. In particular, there is currently no code that proves or applies
the following rewrites:

- `add`, `mul`, `and`, `or`, and `xor` operand normalization;
- subtraction by a constant rewritten as addition of its modular negation;
- `mux(c, v, v)` rewritten to `v`; or
- `mux(c, 1, 0)` rewritten to `c`.

These rules also need explicit interaction with bitwidth, unknown-value
semantics, and any future multi-valued logic model before they are enabled.

## 5. The type model is narrower than the contract

The operations use pliron integer types as signless bitvectors and encode
signedness in operation names. This is a reasonable starting point, but the
current design does not document or enforce all of the associated boundaries:

- there is no dedicated `comb` bitvector type or helper API for querying width;
- signedness is not represented in the type, so every signed operation must
  validate width and interpretation itself;
- the code does not define whether zero-width integers are legal;
- the code does not define behavior for non-integer operand types; and
- conversions, extensions, truncations, and their legality are not modeled by
  `comb` operations.

This makes it difficult for generic analyses to distinguish a value, a signal,
and storage, or to reason about width-preserving versus width-changing
operations without knowing each concrete operation.

## 6. Attributes need stronger contracts

`comb.icmp` stores its predicate as a `StringAttr`, while the Rust API exposes
an `ICmpPredicate` enum. There is no verifier currently checking that parsed or
manually constructed attributes contain one of the ten supported predicate
names.

`comb.extract` and `comb.replicate` likewise store indices and counts as
`IntegerAttr`, but do not yet verify non-negativity, width, or the relationship
between the attribute's integer type and the operation's input width. The
dialect also has no documented policy for attributes that are wider than the
host integer types used by helper code.

## 7. Interface and lowering boundaries are unfinished

The current dialect has constructors and registration, but no documented or
implemented lowering from `comb` to a lower-level Boolean, RTL, or target
dialect. There is also no interface for generic consumers to ask an operation
for its width behavior, signedness, latency, or hardware cost.

Without those boundaries, downstream passes must either special-case every
operation or assume facts that are not represented in the IR. The intended
zero-cycle abstraction is also not connected to a scheduling or timing model;
"zero latency" here means functional combinational behavior, not a claim about
physical propagation delay.

## 8. Documentation and syntax are ahead of implementation

The specification includes textual examples and semantic claims, but the
repository does not yet provide parser/printer coverage for the full `comb`
operation family, round-trip tests, invalid syntax examples, or a generated
operation reference. The examples should therefore be treated as illustrative
until parser support and syntax tests exist for each operation and attribute.

The specification also refers to `iN` syntax, while the current tests construct
pliron integer types directly. The textual syntax, printed type names, and
operation formats should be aligned before the examples are used as a stable
interchange format.

## 9. Testing gaps

The current tests primarily check that constructors produce values of the
requested result type and that a few well-formed modules verify. They do not
yet test:

- rejected malformed operations;
- all signed and unsigned predicates;
- zero and oversized shift amounts;
- division/remainder by zero;
- overflow and truncation behavior;
- exact concat/extract bit ordering;
- replication count edge cases;
- parser/printer round trips; or
- canonicalization and lowering behavior.

Until those tests exist, passing `cargo test` demonstrates structural
construction only and should not be read as proof of the full semantic
contract.

## 10. Recommended order of closure

The highest-value next steps are:

1. Add shared integer-width and one-bit helpers plus operation-specific
   verifiers.
2. Add negative tests for every verifier rule and clarify division-by-zero
   semantics.
3. Encode purity and cycle legality using pliron interfaces or a dialect-level
   verification pass.
4. Add constant folding with explicit width and undefined-value behavior.
5. Define parser/printer syntax and round-trip tests.
6. Specify and implement the first lowering boundary, including which
   information is preserved or intentionally discarded.
