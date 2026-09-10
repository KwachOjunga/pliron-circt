import { DialectItem, BenchmarkExample } from '../types';

export const DIALECT_ITEMS: DialectItem[] = [
  // Types
  {
    id: 'type-int',
    name: 'IntType',
    mlirSyntax: '!hw.int<w> / i<w>',
    plironRust: 'hw::types::IntType::get(ctx, width)',
    category: 'types',
    description: 'Arbitrary-width signless hardware wire or bus.',
    semanticContract: 'Defines bitwidth identity. No signedness assumption is made at this level.',
    status: 'parity_verified',
    agentsMdRef: '§5 Types Are Semantic Contracts'
  },
  {
    id: 'type-inout',
    name: 'InoutType',
    mlirSyntax: '!hw.inout<element_type>',
    plironRust: 'hw::types::InoutType::get(ctx, elem_type)',
    category: 'types',
    description: 'Bidirectional / multi-driver port or net.',
    semanticContract: 'Represents physical bidirectional connectivity with multi-driver resolution semantics.',
    status: 'parity_verified',
    agentsMdRef: '§13 Connectivity and Drivers'
  },
  {
    id: 'type-array',
    name: 'ArrayType',
    mlirSyntax: '!hw.array<size x element_type>',
    plironRust: 'hw::types::ArrayType::get(ctx, size, elem_type)',
    category: 'types',
    description: 'Packed fixed-size multidimensional hardware array.',
    semanticContract: 'Fixed compile-time size and homogeneous element type, laid out continuously in hardware.',
    status: 'parity_verified',
    agentsMdRef: '§5 Aggregate Types'
  },
  {
    id: 'type-struct',
    name: 'StructType',
    mlirSyntax: '!hw.struct<name: type, ...>',
    plironRust: 'hw::types::StructType::get(ctx, fields)',
    category: 'types',
    description: 'Ordered aggregate record of named fields and types.',
    semanticContract: 'Hardware bundle of signals with explicit field identity, preventing loose wire mismatches.',
    status: 'parity_verified',
    agentsMdRef: '§5 Aggregate Types'
  },
  {
    id: 'type-union',
    name: 'UnionType',
    mlirSyntax: '!hw.union<name: type, ...>',
    plironRust: 'hw::types::UnionType::get(ctx, fields)',
    category: 'types',
    description: 'Hardware union sharing physical bit storage.',
    semanticContract: 'Maximum width among fields determines physical allocation; represents polymorphic hardware packets.',
    status: 'parity_verified',
    agentsMdRef: '§5 Value vs Storage'
  },
  {
    id: 'type-alias',
    name: 'TypeAliasType',
    mlirSyntax: '!hw.typealias<@sym, type>',
    plironRust: 'hw::types::TypeAliasType::get(ctx, sym, inner)',
    category: 'types',
    description: 'Named reference to an @hw.typedecl symbol.',
    semanticContract: 'Preserves high-level design taxonomy across passes without premature canonicalization.',
    status: 'parity_verified',
    agentsMdRef: '§28 Do Not Prematurely Lower'
  },
  {
    id: 'type-module',
    name: 'ModuleType',
    mlirSyntax: '!hw.module_type<in (...), out (...)>',
    plironRust: 'hw::types::ModuleType::get(ctx, in, out)',
    category: 'types',
    description: 'First-class functional hardware interface signature.',
    semanticContract: 'Captures port lists and directional semantics as an analyzable first-class type.',
    status: 'parity_verified',
    agentsMdRef: '§14 Hierarchy & Ports'
  },

  // Module Hierarchy Operations
  {
    id: 'op-module',
    name: 'ModuleOp',
    mlirSyntax: 'hw.module @name(%in: T) -> (%out: U) { ... }',
    plironRust: 'hw::ops::ModuleOp::new(ctx, name, inputs)',
    category: 'module_hierarchy',
    description: 'Structural hardware module container with single Graph region and block argument inputs.',
    semanticContract: 'Graph region with no SSA dominance requirement (RegionKind::Graph), modeling concurrent nets.',
    status: 'parity_verified',
    agentsMdRef: '§6 SSA & Graph Regions'
  },
  {
    id: 'op-extern',
    name: 'ExternModuleOp',
    mlirSyntax: 'hw.module.extern @pll(%clk: i1) -> (%out: i1)',
    plironRust: 'hw::ops::ExternModuleOp::new(ctx, name)',
    category: 'module_hierarchy',
    description: 'External blackbox IP, vendor macro, or ASIC standard cell declaration.',
    semanticContract: 'Opaque implementation with known port signatures and boundary timing assumptions.',
    status: 'parity_verified',
    agentsMdRef: '§33 External Modules & Black Boxes'
  },
  {
    id: 'op-output',
    name: 'OutputOp',
    mlirSyntax: 'hw.output %val0, %val1 : T0, T1',
    plironRust: 'hw::ops::OutputOp::new(ctx, outputs)',
    category: 'module_hierarchy',
    description: 'Module output terminator driving internal signals to external pins.',
    semanticContract: 'Required terminator for hw.module; count and types must exactly match module output signature.',
    status: 'parity_verified',
    agentsMdRef: '§22 Operations & Termination'
  },
  {
    id: 'op-instance',
    name: 'InstanceOp',
    mlirSyntax: '%out = hw.instance "u0" @target(%in) : (T) -> (U)',
    plironRust: 'hw::ops::InstanceOp::new(ctx, inst_name, mod_name, ins, outs)',
    category: 'module_hierarchy',
    description: 'Module instantiation linking module input operands to result wires.',
    semanticContract: 'Creates unique instance identity distinct from module definition identity.',
    status: 'parity_verified',
    agentsMdRef: '§14 Instance Ownership & Identity'
  },

  // Connectivity
  {
    id: 'op-wire',
    name: 'WireOp',
    mlirSyntax: '%w = hw.wire %in sym @w : T',
    plironRust: 'hw::ops::WireOp::new(ctx, name, input)',
    category: 'connectivity',
    description: 'Explicit named hardware net with identity.',
    semanticContract: 'Protects critical debug and physical net identities from being collapsed by SSA DCE.',
    status: 'parity_verified',
    agentsMdRef: '§13 Connectivity & Wire Identity'
  },
  {
    id: 'op-bitcast',
    name: 'BitcastOp',
    mlirSyntax: '%res = hw.bitcast %in : (TypeA) -> TypeB',
    plironRust: 'hw::ops::BitcastOp::new(ctx, input, result_type)',
    category: 'connectivity',
    description: 'Bit-level reinterpretation between types of identical bitwidth.',
    semanticContract: 'Zero-latency, zero-gate structural bit reinterpretation (e.g. array<4 x i8> to i32).',
    status: 'parity_verified',
    agentsMdRef: '§25 Verification & Width Compatibility'
  },

  // Bits & Constants
  {
    id: 'op-constant',
    name: 'ConstantOp',
    mlirSyntax: '%c = hw.constant 42 : hw.int<32>',
    plironRust: 'hw::ops::ConstantOp::new(ctx, int_attr)',
    category: 'bits_and_constants',
    description: 'Constant bitvector materialization.',
    semanticContract: 'Evaluates to compile-time constant bit pattern clamped to target width.',
    status: 'parity_verified',
    agentsMdRef: '§11 Combinational Semantics'
  },
  {
    id: 'op-concat',
    name: 'ConcatOp',
    mlirSyntax: '%wide = hw.concat %hi, %lo : (i8, i8) -> i16',
    plironRust: 'hw::ops::ConcatOp::new(ctx, inputs, result_type)',
    category: 'bits_and_constants',
    description: 'Bitvector concatenation of N operands MSB-to-LSB.',
    semanticContract: 'Result width MUST equal the exact sum of all input operand widths.',
    status: 'parity_verified',
    agentsMdRef: '§25 Width Invariant Checking'
  },
  {
    id: 'op-slice',
    name: 'SliceOp',
    mlirSyntax: '%out = hw.slice %in at 4 : (i32) -> i8',
    plironRust: 'hw::ops::SliceOp::new(ctx, input, low_bit, result_type)',
    category: 'bits_and_constants',
    description: 'Bit-slice extraction starting at low_bit.',
    semanticContract: 'Invariant: low_bit + result_width <= input_width. Prevents out-of-bounds net tap.',
    status: 'parity_verified',
    agentsMdRef: '§25 Verification & Bounds'
  },

  // Arrays
  {
    id: 'op-array-create',
    name: 'ArrayCreateOp',
    mlirSyntax: '%arr = hw.array_create %e0, %e1, %e2 : i8',
    plironRust: 'hw::ops::ArrayCreateOp::new(ctx, elems, arr_ty)',
    category: 'arrays',
    description: 'Packs individual element SSA values into a uniform packed array.',
    semanticContract: 'All element types must match array element type; element count must equal array size.',
    status: 'parity_verified',
    agentsMdRef: '§5 Aggregate Construction'
  },
  {
    id: 'op-array-get',
    name: 'ArrayGetOp',
    mlirSyntax: '%elem = hw.array_get %arr[%idx] : !hw.array<4 x i8>, i2',
    plironRust: 'hw::ops::ArrayGetOp::new(ctx, arr, idx, elem_ty)',
    category: 'arrays',
    description: 'Dynamic or static indexing into packed array.',
    semanticContract: 'Synthesizes to hardware multiplexer selecting one element by index bitvector.',
    status: 'parity_verified',
    agentsMdRef: '§11 Combinational Muxing'
  },
  {
    id: 'op-array-slice',
    name: 'ArraySliceOp',
    mlirSyntax: '%sub = hw.array_slice %arr at %idx : (!hw.array<16 x i8>) -> !hw.array<4 x i8>',
    plironRust: 'hw::ops::ArraySliceOp::new(ctx, arr, low_idx, slice_ty)',
    category: 'arrays',
    description: 'Sub-array extraction from a larger hardware array.',
    semanticContract: 'Extracts contiguous subarray of elements with dynamic or constant base offset.',
    status: 'parity_verified',
    agentsMdRef: '§25 Verification & Array Bounds'
  },
  {
    id: 'op-array-concat',
    name: 'ArrayConcatOp',
    mlirSyntax: '%arr = hw.array_concat %a, %b : (!hw.array<2 x i8>, !hw.array<2 x i8>) -> !hw.array<4 x i8>',
    plironRust: 'hw::ops::ArrayConcatOp::new(ctx, arrs, res_ty)',
    category: 'arrays',
    description: 'Concatenates multiple arrays of identical element type.',
    semanticContract: 'Total element count of result must equal sum of input array sizes.',
    status: 'parity_verified',
    agentsMdRef: '§25 Type Compatibility'
  },

  // Structs
  {
    id: 'op-struct-create',
    name: 'StructCreateOp',
    mlirSyntax: '%st = hw.struct_create (%v0, %v1) : !hw.struct<a: i8, b: i16>',
    plironRust: 'hw::ops::StructCreateOp::new(ctx, fields, struct_ty)',
    category: 'structs',
    description: 'Packs individual field values into a hardware struct.',
    semanticContract: 'Operands must strictly match field types and field count in exact declaration order.',
    status: 'parity_verified',
    agentsMdRef: '§5 Aggregate Invariants'
  },
  {
    id: 'op-struct-extract',
    name: 'StructExtractOp',
    mlirSyntax: '%f = hw.struct_extract %st["a"] : !hw.struct<a: i8, b: i16>',
    plironRust: 'hw::ops::StructExtractOp::new(ctx, st, field_name, fty)',
    category: 'structs',
    description: 'Extracts named field from a hardware struct.',
    semanticContract: 'Zero-cost wire tap into struct aggregate by field identifier.',
    status: 'parity_verified',
    agentsMdRef: '§11 Combinational Projections'
  },
  {
    id: 'op-struct-inject',
    name: 'StructInjectOp',
    mlirSyntax: '%new_st = hw.struct_inject %st["a"], %val : !hw.struct<a: i8, b: i16>',
    plironRust: 'hw::ops::StructInjectOp::new(ctx, st, field_name, new_val, st_ty)',
    category: 'structs',
    description: 'Functional update of one struct field producing a new struct value.',
    semanticContract: 'Non-destructive field replacement; leaves all other struct fields unchanged.',
    status: 'parity_verified',
    agentsMdRef: '§11 Functional State Updates'
  },
  {
    id: 'op-struct-explode',
    name: 'StructExplodeOp',
    mlirSyntax: '%a, %b = hw.struct_explode %st : !hw.struct<a: i8, b: i16>',
    plironRust: 'hw::ops::StructExplodeOp::new(ctx, st, field_types)',
    category: 'structs',
    description: 'Unpacks all fields of a struct into individual SSA values.',
    semanticContract: 'Inverse of struct_create; decomposes bundle into separate wires.',
    status: 'parity_verified',
    agentsMdRef: '§5 Flattening & Unpacking'
  }
];

export const BENCHMARKS: BenchmarkExample[] = [
  {
    id: 'alu32',
    name: '32-Bit ALU with Flag Bus',
    description: 'Complete structural module demonstrating hw.module, hw.constant, hw.concat, hw.slice, hw.wire, and hw.output.',
    modules: ['alu32'],
    ir: `// Complete hw dialect module: 32-bit ALU with Flag Bus
hw.module @alu32(%a: hw.int<32>, %b: hw.int<32>, %op_sel: hw.int<2>) -> (%result: hw.int<32>, %zero_flag: hw.int<1>, %high_byte: hw.int<8>) {
  // Constant zero for flag comparisons
  %c0 = hw.constant 0 : hw.int<1>
  %c1 = hw.constant 1 : hw.int<1>

  // Named internal debug wire for monitoring
  %op_wire = hw.wire %op_sel sym @op_probe : hw.int<2>

  // Extract upper 8 bits using hw.slice
  %hi = hw.slice %a at 24 : (hw.int<32>) -> hw.int<8>

  // Concat lower 16 bits of %a and %b into temporary bus
  %lo_a = hw.slice %a at 0 : (hw.int<32>) -> hw.int<16>
  %lo_b = hw.slice %b at 0 : (hw.int<32>) -> hw.int<16>
  %combined_bus = hw.concat %lo_a, %lo_b : (hw.int<16>, hw.int<16>) -> hw.int<32>

  // Output ports
  hw.output %combined_bus, %c0, %hi : hw.int<32>, hw.int<1>, hw.int<8>
}`,
    nodes: [
      { id: 'in_a', label: '%a [32b]', op: 'port_in', kind: 'port_in', x: 40, y: 60, ports: [{ id: 'p_out', name: 'out', type: 'hw.int<32>', direction: 'out', width: 32 }] },
      { id: 'in_b', label: '%b [32b]', op: 'port_in', kind: 'port_in', x: 40, y: 160, ports: [{ id: 'p_out', name: 'out', type: 'hw.int<32>', direction: 'out', width: 32 }] },
      { id: 'in_sel', label: '%op_sel [2b]', op: 'port_in', kind: 'port_in', x: 40, y: 260, ports: [{ id: 'p_out', name: 'out', type: 'hw.int<2>', direction: 'out', width: 2 }] },
      { id: 'op_wire', label: 'hw.wire (probe)', op: 'hw.wire', kind: 'wire', x: 200, y: 260, ports: [{ id: 'w_in', name: 'in', type: 'hw.int<2>', direction: 'in', width: 2 }, { id: 'w_out', name: 'out', type: 'hw.int<2>', direction: 'out', width: 2 }] },
      { id: 'slice_hi', label: 'hw.slice [24..31]', op: 'hw.slice', kind: 'comb_op', x: 200, y: 40, ports: [{ id: 's_in', name: 'in', type: 'hw.int<32>', direction: 'in', width: 32 }, { id: 's_out', name: 'out', type: 'hw.int<8>', direction: 'out', width: 8 }] },
      { id: 'slice_a', label: 'hw.slice [0..15]', op: 'hw.slice', kind: 'comb_op', x: 200, y: 110, ports: [{ id: 's_in', name: 'in', type: 'hw.int<32>', direction: 'in', width: 32 }, { id: 's_out', name: 'out', type: 'hw.int<16>', direction: 'out', width: 16 }] },
      { id: 'slice_b', label: 'hw.slice [0..15]', op: 'hw.slice', kind: 'comb_op', x: 200, y: 180, ports: [{ id: 's_in', name: 'in', type: 'hw.int<32>', direction: 'in', width: 32 }, { id: 's_out', name: 'out', type: 'hw.int<16>', direction: 'out', width: 16 }] },
      { id: 'concat_lo', label: 'hw.concat [32b]', op: 'hw.concat', kind: 'comb_op', x: 380, y: 140, ports: [{ id: 'c_in0', name: 'hi', type: 'hw.int<16>', direction: 'in', width: 16 }, { id: 'c_in1', name: 'lo', type: 'hw.int<16>', direction: 'in', width: 16 }, { id: 'c_out', name: 'out', type: 'hw.int<32>', direction: 'out', width: 32 }] },
      { id: 'c_zero', label: 'hw.constant 0', op: 'hw.constant', kind: 'constant', x: 380, y: 240, ports: [{ id: 'c_out', name: 'out', type: 'hw.int<1>', direction: 'out', width: 1 }] },
      { id: 'out_res', label: '%result [32b]', op: 'port_out', kind: 'port_out', x: 560, y: 120, ports: [{ id: 'p_in', name: 'in', type: 'hw.int<32>', direction: 'in', width: 32 }] },
      { id: 'out_zf', label: '%zero_flag [1b]', op: 'port_out', kind: 'port_out', x: 560, y: 220, ports: [{ id: 'p_in', name: 'in', type: 'hw.int<1>', direction: 'in', width: 1 }] },
      { id: 'out_hi', label: '%high_byte [8b]', op: 'port_out', kind: 'port_out', x: 560, y: 40, ports: [{ id: 'p_in', name: 'in', type: 'hw.int<8>', direction: 'in', width: 8 }] }
    ],
    edges: [
      { id: 'e1', sourceNodeId: 'in_a', sourcePortId: 'p_out', targetNodeId: 'slice_hi', targetPortId: 's_in', width: 32 },
      { id: 'e2', sourceNodeId: 'in_a', sourcePortId: 'p_out', targetNodeId: 'slice_a', targetPortId: 's_in', width: 32 },
      { id: 'e3', sourceNodeId: 'in_b', sourcePortId: 'p_out', targetNodeId: 'slice_b', targetPortId: 's_in', width: 32 },
      { id: 'e4', sourceNodeId: 'in_sel', sourcePortId: 'p_out', targetNodeId: 'op_wire', targetPortId: 'w_in', width: 2 },
      { id: 'e5', sourceNodeId: 'slice_a', sourcePortId: 's_out', targetNodeId: 'concat_lo', targetPortId: 'c_in0', width: 16 },
      { id: 'e6', sourceNodeId: 'slice_b', sourcePortId: 's_out', targetNodeId: 'concat_lo', targetPortId: 'c_in1', width: 16 },
      { id: 'e7', sourceNodeId: 'concat_lo', sourcePortId: 'c_out', targetNodeId: 'out_res', targetPortId: 'p_in', width: 32 },
      { id: 'e8', sourceNodeId: 'c_zero', sourcePortId: 'c_out', targetNodeId: 'out_zf', targetPortId: 'p_in', width: 1 },
      { id: 'e9', sourceNodeId: 'slice_hi', sourcePortId: 's_out', targetNodeId: 'out_hi', targetPortId: 'p_in', width: 8 }
    ]
  },
  {
    id: 'packet_router',
    name: 'Hardware Packet Router (Structs & Arrays)',
    description: 'Demonstrates hw.struct, hw.struct_create, hw.struct_extract, hw.array, hw.array_create, and hw.array_get.',
    modules: ['packet_router'],
    ir: `// Packet Router utilizing hw.struct and hw.array aggregates
hw.module @packet_router(%valid: hw.int<1>, %dest: hw.int<2>, %payload: hw.int<32>) -> (%active_payload: hw.int<32>) {
  // Construct a hardware packet struct
  %pkt = hw.struct_create (%valid, %dest, %payload) : hw.struct<v: hw.int<1>, dst: hw.int<2>, data: hw.int<32>>

  // Extract fields
  %extracted_data = hw.struct_extract %pkt["data"] : hw.struct<v: hw.int<1>, dst: hw.int<2>, data: hw.int<32>>
  %extracted_dst  = hw.struct_extract %pkt["dst"]  : hw.struct<v: hw.int<1>, dst: hw.int<2>, data: hw.int<32>>

  // Pack into a 4-channel router array
  %c_zero32 = hw.constant 0 : hw.int<32>
  %channels = hw.array_create %extracted_data, %c_zero32, %c_zero32, %c_zero32 : hw.int<32>

  // Dynamic demux query using hw.array_get
  %routed_data = hw.array_get %channels[%extracted_dst] : hw.array<4 x hw.int<32>>, hw.int<2>

  hw.output %routed_data : hw.int<32>
}`,
    nodes: [
      { id: 'in_v', label: '%valid [1b]', op: 'port_in', kind: 'port_in', x: 40, y: 60, ports: [{ id: 'p_out', name: 'out', type: 'hw.int<1>', direction: 'out', width: 1 }] },
      { id: 'in_dst', label: '%dest [2b]', op: 'port_in', kind: 'port_in', x: 40, y: 150, ports: [{ id: 'p_out', name: 'out', type: 'hw.int<2>', direction: 'out', width: 2 }] },
      { id: 'in_pay', label: '%payload [32b]', op: 'port_in', kind: 'port_in', x: 40, y: 240, ports: [{ id: 'p_out', name: 'out', type: 'hw.int<32>', direction: 'out', width: 32 }] },
      { id: 'st_create', label: 'hw.struct_create', op: 'hw.struct_create', kind: 'struct_op', x: 220, y: 130, ports: [{ id: 's_in0', name: 'v', type: 'hw.int<1>', direction: 'in', width: 1 }, { id: 's_in1', name: 'dst', type: 'hw.int<2>', direction: 'in', width: 2 }, { id: 's_in2', name: 'data', type: 'hw.int<32>', direction: 'in', width: 32 }, { id: 's_out', name: 'pkt', type: 'hw.struct', direction: 'out', width: 35 }] },
      { id: 'st_ext_data', label: 'hw.struct_extract ["data"]', op: 'hw.struct_extract', kind: 'struct_op', x: 380, y: 80, ports: [{ id: 's_in', name: 'pkt', type: 'hw.struct', direction: 'in', width: 35 }, { id: 's_out', name: 'data', type: 'hw.int<32>', direction: 'out', width: 32 }] },
      { id: 'st_ext_dst', label: 'hw.struct_extract ["dst"]', op: 'hw.struct_extract', kind: 'struct_op', x: 380, y: 220, ports: [{ id: 's_in', name: 'pkt', type: 'hw.struct', direction: 'in', width: 35 }, { id: 's_out', name: 'dst', type: 'hw.int<2>', direction: 'out', width: 2 }] },
      { id: 'arr_create', label: 'hw.array_create [4 x 32b]', op: 'hw.array_create', kind: 'array_op', x: 540, y: 80, ports: [{ id: 'a_in0', name: 'ch0', type: 'hw.int<32>', direction: 'in', width: 32 }, { id: 'a_out', name: 'arr', type: 'hw.array', direction: 'out', width: 128 }] },
      { id: 'arr_get', label: 'hw.array_get [Mux]', op: 'hw.array_get', kind: 'array_op', x: 700, y: 140, ports: [{ id: 'g_arr', name: 'arr', type: 'hw.array', direction: 'in', width: 128 }, { id: 'g_idx', name: 'idx', type: 'hw.int<2>', direction: 'in', width: 2 }, { id: 'g_out', name: 'out', type: 'hw.int<32>', direction: 'out', width: 32 }] },
      { id: 'out_data', label: '%active_payload [32b]', op: 'port_out', kind: 'port_out', x: 860, y: 140, ports: [{ id: 'p_in', name: 'in', type: 'hw.int<32>', direction: 'in', width: 32 }] }
    ],
    edges: [
      { id: 'e1', sourceNodeId: 'in_v', sourcePortId: 'p_out', targetNodeId: 'st_create', targetPortId: 's_in0', width: 1 },
      { id: 'e2', sourceNodeId: 'in_dst', sourcePortId: 'p_out', targetNodeId: 'st_create', targetPortId: 's_in1', width: 2 },
      { id: 'e3', sourceNodeId: 'in_pay', sourcePortId: 'p_out', targetNodeId: 'st_create', targetPortId: 's_in2', width: 32 },
      { id: 'e4', sourceNodeId: 'st_create', sourcePortId: 's_out', targetNodeId: 'st_ext_data', targetPortId: 's_in', width: 35 },
      { id: 'e5', sourceNodeId: 'st_create', sourcePortId: 's_out', targetNodeId: 'st_ext_dst', targetPortId: 's_in', width: 35 },
      { id: 'e6', sourceNodeId: 'st_ext_data', sourcePortId: 's_out', targetNodeId: 'arr_create', targetPortId: 'a_in0', width: 32 },
      { id: 'e7', sourceNodeId: 'arr_create', sourcePortId: 'a_out', targetNodeId: 'arr_get', targetPortId: 'g_arr', width: 128 },
      { id: 'e8', sourceNodeId: 'st_ext_dst', sourcePortId: 's_out', targetNodeId: 'arr_get', targetPortId: 'g_idx', width: 2 },
      { id: 'e9', sourceNodeId: 'arr_get', sourcePortId: 'g_out', targetNodeId: 'out_data', targetPortId: 'p_in', width: 32 }
    ]
  },
  {
    id: 'top_hierarchy',
    name: 'Hierarchical Chip with hw.instance & hw.module.extern',
    description: 'Shows module hierarchy with black-box external clock synthesizer and sub-module instantiations.',
    modules: ['top_chip', 'pll_macro', 'sub_core'],
    ir: `// External black box PLL macro
hw.module.extern @pll_macro(%ref_clk: hw.int<1>) -> (%core_clk: hw.int<1>, %lock: hw.int<1>)

// Sub-core processing unit
hw.module @sub_core(%clk: hw.int<1>, %din: hw.int<16>) -> (%dout: hw.int<16>) {
  %hi = hw.slice %din at 8 : (hw.int<16>) -> hw.int<8>
  %lo = hw.slice %din at 0 : (hw.int<16>) -> hw.int<8>
  %swapped = hw.concat %lo, %hi : (hw.int<8>, hw.int<8>) -> hw.int<16>
  hw.output %swapped : hw.int<16>
}

// Top-level chip instantiating PLL and two sub-cores
hw.module @top_chip(%ref_clk: hw.int<1>, %sys_din: hw.int<16>) -> (%sys_dout: hw.int<16>, %pll_lock: hw.int<1>) {
  // Instance 0: External PLL
  %core_clk, %lock = hw.instance "u_pll" @pll_macro(%ref_clk) : (hw.int<1>) -> (hw.int<1>, hw.int<1>)

  // Instance 1: Core A
  %core_a_out = hw.instance "u_core_a" @sub_core(%core_clk, %sys_din) : (hw.int<1>, hw.int<16>) -> (hw.int<16>)

  // Instance 2: Core B cascading from Core A
  %core_b_out = hw.instance "u_core_b" @sub_core(%core_clk, %core_a_out) : (hw.int<1>, hw.int<16>) -> (hw.int<16>)

  hw.output %core_b_out, %lock : hw.int<16>, hw.int<1>
}`,
    nodes: [
      { id: 'in_clk', label: '%ref_clk [1b]', op: 'port_in', kind: 'port_in', x: 40, y: 80, ports: [{ id: 'p_out', name: 'out', type: 'hw.int<1>', direction: 'out', width: 1 }] },
      { id: 'in_din', label: '%sys_din [16b]', op: 'port_in', kind: 'port_in', x: 40, y: 220, ports: [{ id: 'p_out', name: 'out', type: 'hw.int<16>', direction: 'out', width: 16 }] },
      { id: 'inst_pll', label: 'hw.instance "u_pll" (@pll_macro)', op: 'hw.instance', kind: 'instance', x: 220, y: 60, ports: [{ id: 'i_clk', name: 'ref', type: 'hw.int<1>', direction: 'in', width: 1 }, { id: 'o_clk', name: 'core_clk', type: 'hw.int<1>', direction: 'out', width: 1 }, { id: 'o_lock', name: 'lock', type: 'hw.int<1>', direction: 'out', width: 1 }] },
      { id: 'inst_a', label: 'hw.instance "u_core_a" (@sub_core)', op: 'hw.instance', kind: 'instance', x: 440, y: 160, ports: [{ id: 'i_clk', name: 'clk', type: 'hw.int<1>', direction: 'in', width: 1 }, { id: 'i_din', name: 'din', type: 'hw.int<16>', direction: 'in', width: 16 }, { id: 'o_dout', name: 'dout', type: 'hw.int<16>', direction: 'out', width: 16 }] },
      { id: 'inst_b', label: 'hw.instance "u_core_b" (@sub_core)', op: 'hw.instance', kind: 'instance', x: 660, y: 160, ports: [{ id: 'i_clk', name: 'clk', type: 'hw.int<1>', direction: 'in', width: 1 }, { id: 'i_din', name: 'din', type: 'hw.int<16>', direction: 'in', width: 16 }, { id: 'o_dout', name: 'dout', type: 'hw.int<16>', direction: 'out', width: 16 }] },
      { id: 'out_dout', label: '%sys_dout [16b]', op: 'port_out', kind: 'port_out', x: 860, y: 160, ports: [{ id: 'p_in', name: 'in', type: 'hw.int<16>', direction: 'in', width: 16 }] },
      { id: 'out_lock', label: '%pll_lock [1b]', op: 'port_out', kind: 'port_out', x: 860, y: 60, ports: [{ id: 'p_in', name: 'in', type: 'hw.int<1>', direction: 'in', width: 1 }] }
    ],
    edges: [
      { id: 'e1', sourceNodeId: 'in_clk', sourcePortId: 'p_out', targetNodeId: 'inst_pll', targetPortId: 'i_clk', width: 1 },
      { id: 'e2', sourceNodeId: 'inst_pll', sourcePortId: 'o_lock', targetNodeId: 'out_lock', targetPortId: 'p_in', width: 1 },
      { id: 'e3', sourceNodeId: 'inst_pll', sourcePortId: 'o_clk', targetNodeId: 'inst_a', targetPortId: 'i_clk', width: 1 },
      { id: 'e4', sourceNodeId: 'in_din', sourcePortId: 'p_out', targetNodeId: 'inst_a', targetPortId: 'i_din', width: 16 },
      { id: 'e5', sourceNodeId: 'inst_pll', sourcePortId: 'o_clk', targetNodeId: 'inst_b', targetPortId: 'i_clk', width: 1 },
      { id: 'e6', sourceNodeId: 'inst_a', sourcePortId: 'o_dout', targetNodeId: 'inst_b', targetPortId: 'i_din', width: 16 },
      { id: 'e7', sourceNodeId: 'inst_b', sourcePortId: 'o_dout', targetNodeId: 'out_dout', targetPortId: 'p_in', width: 16 }
    ]
  }
];
