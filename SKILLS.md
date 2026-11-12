---
name: "sv-frontend-mold-style"
description: "Design and implement the pliron-circt SystemVerilog front-end (v-parser) as a mold-style parallel pipeline lowering SV to the hw, comb, seq and sv dialects. Use for any v-parser, SV-to-IR or front-end performance work."
---

# SystemVerilog front-end, built the way mold is built

This skill covers work on the SystemVerilog front-end that lives in the `v-parser` crate of
`pliron-circt`. The goal has two halves and both matter:

1. Correctness: every SystemVerilog construct we accept resolves to verified IR in the
   `hw`, `comb`, `seq` and `sv` dialects defined by `pliron-circt`, with no silent loss of meaning.
2. Capacity: the front-end is structured like the mold linker, so a design of thousands of
   files is ingested in a handful of flat, data-parallel passes instead of a recursive walk
   that touches one module at a time.

Facts in this file were checked against `pliron-circt` at commit `bf9bcdc` (v0.2.0,
pliron 0.18.0, sv-parser 0.13.5) and mold 3.0.0. Both projects move. When the code and this
file disagree, the code wins, and you should say so in your report.

## Read before you write

In the repository, in this order:

- `AGENTS.md`. The dialect design rules. Binding for anything that adds or changes an op.
- `docs/pliron_hardware_ir_constraints.md`. What pliron allows and forbids.
- `docs/dialect_relationships_and_semantic_contracts.md`. Which dialect owns which question.
- `docs/sv_dialect.md` and `docs/sv_transformations.md`. The op contracts the parser targets.
- `docs/systemverilog_parser_specification.md` and `docs/sv_parser_migration_challenges.md`.
  The accepted subset and the staged plan.
- `v-parser/src/core.rs`. The current `SvModuleLowerer`.

Some of those docs are stale. They mention `src/sv/parser.rs` and `pliron-hw`, neither of
which is where the parser lives now, and they name attributes and ops that differ from the
code. For names, `src/*/ops.rs` is the only authority. Examples that will bite you:

- The constant op is registered as `sv.constant` (Rust type `ConstantExprOp`), not `sv.constant_expr`.
- `sv.assign` stores its target under `assign_target`. `sv.always_ff` uses `ff_target`,
  `ff_async_reset`, `ff_reset_polarity`. Other ops follow the same prefixed pattern
  (`logic_target`, `wire_target`, `reg_target`, `comb_target`, `ff_nr_target`, `bpa_target`,
  `nba_target`, `case_target`, `binary_operator`, `unary_operator`, `slice_low_bit`,
  `slice_width`, `sv_instance_name`, `sv_instance_module`).

For mold, read `common/concurrent_map.rs`, `common/worker_local.rs`, `common/parallel.rs`,
`common/mapped_file.rs`, `common/perf.rs`, `elf/driver.rs` and `elf/passes.rs`, plus the paper
"mold: A Massively Parallel Linker" (arXiv 2608.23228). mold 3.0 is written in Rust on rayon,
so its code is a direct reference, not an analogy.

## Constraints that shape the whole design

These are not preferences. Each one was verified in source.

- A pliron `Context` is `Send` but not `Sync`. Its arenas are `SlotMap<_, RefCell<T>>` and
  builders take `&mut Context`. So IR construction has exactly one writer. All parallelism
  must happen before the first op is created, on plain data that does not mention `Context`.
- Types, ops and attributes are owned by the `Context`. A `TypeHandle` or `Ptr<Operation>`
  from one context means nothing in another, so "build per-thread contexts and merge" is not
  an option.
- Call `register_all(&mut ctx)` before building anything.
- Attribute dictionary keys are globally unique across op definitions. A new op needs
  op-prefixed keys or registration panics.
- Op names have one dialect separator. `seq.hlmem_read`, not `seq.hlmem.read`.
- `hw.module` is a graph region. Operation order is for printing, not scheduling. Do not
  encode source order as meaning.
- Resolve all values, types and metadata before the mutable borrow that creates an op.
- A constructor building an op proves nothing. Only `verify` plus `hw::validation` do.

## How mold maps onto this problem

| mold | This front-end |
| --- | --- |
| Object file | One SystemVerilog compilation unit (normally one source file) |
| Archive member, loaded lazily | Library file found through `-y` / `-v`, loaded only if referenced |
| Global symbol | A definition name: module, interface, program, primitive, package |
| Undefined symbol | An instantiated module name, an imported package name |
| Symbol resolution by file priority | Pick the defining file for each name by command-line order |
| `mark_live_objects` | Reachability from the top module(s) |
| Mergeable string / ICF dedup | Unique (module, parameter values) specialisations |
| Output chunks sized, then written in parallel | Lowering plans built in parallel, then emitted |
| Writing the output file | Emitting IR into the single `Context` |

The mold habits to copy:

- The driver is a flat list of named passes over `Vec`s. Each pass is one `par_iter` with a
  timer around it. No pass recurses into another.
- Inputs are memory-mapped once and leaked to `'static`, so no lifetimes thread through the
  data structures and nothing is reference counted.
- Things are referred to by dense `u32` ids (`FileId`, `DefId`, `ModuleId`, `ExprId`), never
  by `Rc`, `Arc` or borrowed pointers. Ids make tables shareable and results reproducible.
- Count first, then allocate once. mold sizes its concurrent map from a HyperLogLog
  estimate and the map aborts if it fills up, because it never resizes.
- Hot paths take no locks. Use atomics for flags and "lowest priority wins" slots, and
  per-worker scratch (`WorkerLocal` pattern: one cache-line-aligned slot per rayon worker,
  fetched once per file, not once per token).
- Output is independent of thread scheduling. Ties are broken by file priority and offset,
  never by who got there first.
- Thread count defaults to available cores capped at 32, overridable. mold found it stops
  scaling past that. Measure before raising it.
- Timers and counters are built in from day one (`--perf`, `--stats`).

## The pipeline

Keep these as separate functions in a `passes` module, called in order from the driver.
Passes 1 to 6 never touch `Context`.

1. **Command line.** Source files, `-f` file lists, `+incdir`, `+define`, `-y`/`-v` library
   paths, `--top`, `--threads`. Assign each file a priority equal to its position.
2. **Load.** Map every file (`memmap2`), leak it. Record `FileId -> (path, &'static [u8])`.
3. **Parse, in parallel per compilation unit.** Preprocess and parse with `sv-parser`. Each
   task owns its `SyntaxTree`. From the CST extract a `FileSummary` of plain data: the
   definitions in the file, the names each definition references, and their byte spans.
   Macro state crosses file boundaries only inside one compilation unit, so the default is
   one unit per file. A single-unit mode must exist and must fall back to serial
   preprocessing in command-line order. Never share `` `define `` state between parallel tasks.
4. **Resolve definitions, in parallel.** Insert every definition name into a concurrent map
   keyed by the interned name. On collision between two command-line files, report a
   duplicate definition with both locations. Between a command-line file and a library file,
   the command-line file wins. Decide by priority with a compare-and-swap minimum so the
   answer never depends on timing.
5. **Liveness.** Starting from the top modules (given, or every module nobody instantiates),
   walk instantiation and import edges in parallel and mark what is reachable. Load library
   files discovered on the way and loop until nothing new appears. Report unresolved names
   here, all at once, each with its location. Drop unreachable units.
6. **Elaborate, in parallel per module.** Turn each live module's CST into a `ModulePlan`:
   ports (direction, width, clock/reset classification), declarations, and a flat arena of
   statements and expressions addressed by `ExprId`, each with a resolved width, signedness
   and span. Constant-fold ranges and parameters here. Collect diagnostics per worker.
   A `ModulePlan` is `Send`, contains no pliron types, and describes types with a small
   descriptor enum (`Int(u32)`, `Clock`, `Reset`, `Mem { elem, depth }`, ...).
   Parameterised modules: key each specialisation by (module, canonical parameter values)
   in the concurrent map, so one plan is built per distinct key.
7. **Emit, serial, one `Context`.** Walk plans in a deterministic order (resolved priority,
   then offset). For each module: create `hw.module` with input types, bind block arguments,
   create all declarations, then drivers and processes, then `hw.output`. This is the only
   pass that may import `pliron::context::Context`. Keep it a dumb translation of the plan.
   Any decision made here is a decision that could not be parallelised or tested without a
   context, so push it back into pass 6.
8. **Verify and report.** Run op verification, `validate_module` on each module, and
   `validate_modules` across the set. Sort diagnostics by (file priority, offset) and print.

Keep `sv-parser` behind one module (`syntax`) whose public surface is `FileSummary` and
`ModulePlan` builders. `sv-parser` is a full IEEE 1800-2017 CST parser and will be the
throughput ceiling. If profiling says the CST is the bottleneck, that boundary lets it be
replaced without touching resolution, elaboration or emission.

Start with ordinary crates (`rayon`, `memmap2`, `rustc-hash`, a sharded map). Hand-roll a
mold-style fixed-size map only when a profile on a real corpus shows the map is the cost.

## What each construct resolves to

The front-end emits `hw.module` containing `sv.*` ops, with `seq` clock and reset types on
ports. That is what `v-parser` does today and what `docs/sv_dialect.md` describes. Raising
`sv` processes into `seq.firreg` / `seq.compreg` and `comb.*` is a separate pass with its own
contract, because `docs/sv_transformations.md` marks that direction as not
semantics-preserving by default. Do not fold it into the parser unless asked.

| SystemVerilog | IR | State at `bf9bcdc` |
| --- | --- | --- |
| ANSI module header | `hw.module`, inputs as block arguments, `hw.output` | Lowered |
| Non-ANSI header | same, after reconciling list and body declarations | Stub that errors |
| 1-bit `clk`/`clock`, `rst`/`reset` inputs | `!seq.clock`, `!seq.reset` | Lowered, by name heuristic |
| `logic` / `wire` / `reg` | `sv.logic_decl` / `sv.wire_decl` / `sv.reg_decl` | Lowered |
| Unpacked array of vectors | `sv.mem_decl` with `MemoryType` | Lowered |
| `assign` | `sv.assign` | Lowered |
| `always_comb` | `sv.always_comb` (`sv.bpa` for statements inside a block) | Single target, `sv.bpa` unused |
| `always_ff` | `sv.always_ff`, `sv.always_ff_no_reset` (`sv.nba` for statements) | Single target, simple reset shape, `sv.nba` unused |
| Binary, unary | `sv.binary_expr`, `sv.unary_expr` | Lowered |
| `?:`, `{}`, `a[h:l]`, `a[i]` | `sv.mux_expr`, `sv.concat_expr`, `sv.slice_expr`, `sv.index_expr` | Lowered |
| Literals | `sv.constant` | u64 only |
| `case` | `sv.case` | Op exists, not parsed |
| Instances | `sv.instance` (`hw.instance` is the structural form) | `sv.instance` only, target not resolved across files |
| `typedef struct` / `enum` / `union` | `hw.struct`, `hw.enum`, `hw.union`, `hw.typealias` | Types exist, not parsed |
| Parameters | `hw.param_decl`, `hw.param_value` | Ops exist, not parsed |
| Packages, interfaces, generate | none yet | Needs elaboration design first |
| Delays, `initial`, classes, system tasks | none | Reject with a located diagnostic |

When a construct has no op that carries its meaning, stop and propose the op under the
`AGENTS.md` rules. Do not approximate it with a nearby op and do not stash semantics in a
free-form string attribute.

## Rules for lowering

- Two steps per module, always: register every port and declaration, then lower drivers.
  SystemVerilog allows use before declaration and the IR needs the value to exist.
- Clocks and resets are typed resources. The name heuristic is a fallback. An explicit
  sensitivity list (`@(posedge x)`) is stronger evidence than a name, and both should lose
  to an explicit user annotation.
- Width and signedness are decided in elaboration by the IEEE 1800 sizing rules, and the
  plan records the result on every expression. Emission never guesses a width.
- Preserve reset polarity, sync versus async, reset priority, and blocking versus
  non-blocking kind. These are the properties `sv_dialect.md` says must survive.
- Track the driver of every output and every net. Multiple drivers, undriven outputs and
  inferred latches are diagnostics, not undefined behaviour.

## Diagnostics

- Input can never panic the tool. No `unwrap`, `expect` or indexing on anything derived from
  source text. `parse_number_literal` currently falls back to `0` and width `32` on a parse
  failure and truncates to `u64`. That is a silent wrong answer and must become an error or
  a wide `APInt`.
- Every diagnostic carries a real location. `locate_to_loc` currently returns
  `Location::Unknown`. Map `Locate { offset, line, len }` to `Location::SrcPos`, and keep the
  byte span in the plan so pass 8 can sort.
- An unsupported construct is reported as `unsupported SystemVerilog construct: <name>` with
  its location. It is never skipped.
- Collect and report all errors from a pass. Do not stop at the first one.

## Testing and proof of done

- `./pre-ci.sh` must pass: `cargo fmt --check`, clippy with `-D warnings` across the
  workspace, tarpaulin, and tests with `RUSTFLAGS="-D warnings"`.
- Snapshot the printed IR with `expect-test`. One positive and one rejection test per
  construct. `docs/hw_limitations.md` notes the suite leans toward construction, so add the
  rejection case.
- Determinism test: run the same design with `--threads 1` and `--threads N`, and with the
  file order of a library directory shuffled. Printed IR and diagnostics must be identical
  byte for byte.
- Round trip where the printer supports it: parse, `render_module`, parse again, compare.
- Keep a benchmark corpus (a large open design plus a synthetic many-file design) and
  record per-pass timings before and after any change that claims a speedup. A performance
  claim without numbers from the timers is not accepted.
- Work one issue at a time: fix it, verify it, record the status, then move on.

## Things not to do

- Do not create ops, types or attributes from inside a rayon task.
- Do not put `Rc`, `RefCell`, `Arc<Mutex<_>>` or pliron handles inside `FileSummary` or `ModulePlan`.
- Do not let a `HashMap` iteration order reach output or diagnostics.
- Do not add a recursive "lower this module, then its children" path. Children are found by
  the liveness pass and lowered by the same flat loop.
- Do not widen the accepted subset without updating the mapping table in
  `docs/systemverilog_parser_specification.md` in the same change.
