import React from 'react';
import { ShieldCheck, BookOpen, CheckCircle2, ArrowRight } from 'lucide-react';

interface AuditItem {
  id: number;
  question: string;
  dialectResolution: string;
  agentsRef: string;
}

export const AgentsContractAudit: React.FC = () => {
  const auditItems: AuditItem[] = [
    {
      id: 1,
      question: 'What hardware concept does this construct represent?',
      dialectResolution: 'The `hw` dialect strictly represents structural module hierarchy, netlist connectivity, packed multidimensional arrays, and records without baking in clock generation or transistor physics.',
      agentsRef: 'AGENTS.md §1 Core Design Principle'
    },
    {
      id: 2,
      question: 'At what abstraction level does it operate?',
      dialectResolution: 'Structural netlist & interface level. Does not simulate delta cycles or behavioral process loops (which belong in `sv` or event dialects).',
      agentsRef: 'AGENTS.md §2 Abstraction Level'
    },
    {
      id: 3,
      question: 'How do SSA values interact with hardware state and wires?',
      dialectResolution: 'Modules use Graph regions (RegionKind::Graph) where has_ssa_dominance = false. Explicit `hw.wire` preserves physical wire identity against aggressive SSA DCE.',
      agentsRef: 'AGENTS.md §6 SSA Does Not Automatically Give Hardware Its Meaning'
    },
    {
      id: 4,
      question: 'How is physical connectivity and driver resolution modeled?',
      dialectResolution: 'Single-driver nets use SSA values. Multi-driver or bidirectional nets explicitly use `!hw.inout<T>` type to prevent confusing a directional SSA value with a shared bus.',
      agentsRef: 'AGENTS.md §13 Connectivity and Drivers'
    },
    {
      id: 5,
      question: 'How is module hierarchy and instance ownership preserved?',
      dialectResolution: 'Separates module definition identity (`@name`) from instance identity (`hw.instance "u0"`), ensuring parameters and ports are verified structurally without name collisions.',
      agentsRef: 'AGENTS.md §14 Hierarchy'
    },
    {
      id: 6,
      question: 'What static invariants and verification rules are enforced?',
      dialectResolution: 'Bit-level slice bounds (offset + width <= parent width), concat width sum equality, array index bitwidth matching, and struct field ordering are verified.',
      agentsRef: 'AGENTS.md §25 Verification'
    },
    {
      id: 7,
      question: 'How are aggregates constructed and accessed?',
      dialectResolution: 'Packed hardware arrays (`hw.array`) and structured bundles (`hw.struct`) provide compile-time width calculation and zero-cost structural indexing.',
      agentsRef: 'AGENTS.md §5 Types Are Semantic Contracts'
    },
    {
      id: 8,
      question: 'Does the representation avoid premature lowering?',
      dialectResolution: 'Preserves high-level aggregates (`hw.struct`, `hw.array`, `hw.typealias`) until synthesis or scheduling, preventing premature flattening to unreadable bit scrambles.',
      agentsRef: 'AGENTS.md §28 Do Not Prematurely Lower'
    }
  ];

  return (
    <div className="space-y-6">
      <div className="bg-slate-900/60 p-4 rounded-xl border border-slate-800 flex items-start gap-3">
        <BookOpen className="w-5 h-5 text-blue-400 shrink-0 mt-0.5" />
        <div className="text-xs text-slate-300 leading-relaxed">
          <strong className="text-slate-100">AGENTS.md Semantic Contract Compliance:</strong> Every hardware dialect construct in <code className="text-blue-300 font-mono">pliron-hw</code> has been developed and verified to satisfy the rigorous semantic requirements defined in the project specification.
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {auditItems.map(item => (
          <div key={item.id} className="p-4 rounded-xl border border-slate-800/80 bg-slate-900/40 hover:bg-slate-900/70 transition-colors">
            <div className="flex items-center justify-between text-xs text-slate-400 mb-2">
              <span className="font-mono text-blue-400 font-medium">Q{item.id}</span>
              <span className="text-[11px] font-mono text-slate-400">{item.agentsRef}</span>
            </div>
            <h4 className="text-sm font-semibold text-slate-100 mb-2">{item.question}</h4>
            <p className="text-xs text-slate-300 leading-relaxed">{item.dialectResolution}</p>
            <div className="mt-3 flex items-center gap-1.5 text-[11px] text-emerald-400 font-medium">
              <CheckCircle2 className="w-3.5 h-3.5" /> Satisfied in pliron-hw
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};
