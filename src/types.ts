export type DialectCategory = 'types' | 'module_hierarchy' | 'connectivity' | 'bits_and_constants' | 'arrays' | 'structs';

export interface DialectItem {
  id: string;
  name: string;
  mlirSyntax: string;
  plironRust: string;
  category: DialectCategory;
  description: string;
  semanticContract: string;
  status: 'complete' | 'parity_verified';
  agentsMdRef: string;
}

export interface Diagnostic {
  line: number;
  message: string;
  severity: 'error' | 'warning' | 'info';
  rule: string;
}

export interface NetlistPort {
  id: string;
  name: string;
  type: string;
  direction: 'in' | 'out' | 'inout';
  width: number;
}

export interface NetlistNode {
  id: string;
  label: string;
  op: string;
  kind: 'module' | 'instance' | 'comb_op' | 'constant' | 'wire' | 'port_in' | 'port_out' | 'array_op' | 'struct_op';
  ports: NetlistPort[];
  x: number;
  y: number;
  detail?: string;
}

export interface NetlistEdge {
  id: string;
  sourceNodeId: string;
  sourcePortId: string;
  targetNodeId: string;
  targetPortId: string;
  label?: string;
  width?: number;
}

export interface BenchmarkExample {
  id: string;
  name: string;
  description: string;
  ir: string;
  modules: string[];
  nodes: NetlistNode[];
  edges: NetlistEdge[];
}
