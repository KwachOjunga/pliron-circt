import React, { useState } from 'react';
import { DIALECT_ITEMS } from '../data/dialectData';
import { DialectCategory } from '../types';
import { CheckCircle2, Cpu, Box, Share2, Layers, Binary, ShieldCheck } from 'lucide-react';

export const ParityMatrix: React.FC = () => {
  const [selectedCategory, setSelectedCategory] = useState<DialectCategory | 'all'>('all');
  const [searchTerm, setSearchTerm] = useState('');

  const categories: { id: DialectCategory | 'all'; label: string; icon: any }[] = [
    { id: 'all', label: 'All Constructs', icon: Layers },
    { id: 'types', label: 'Types (7)', icon: Box },
    { id: 'module_hierarchy', label: 'Hierarchy & Modules (4)', icon: Cpu },
    { id: 'connectivity', label: 'Connectivity & Wires (2)', icon: Share2 },
    { id: 'bits_and_constants', label: 'Bits & Slices (3)', icon: Binary },
    { id: 'arrays', label: 'Arrays (4)', icon: Layers },
    { id: 'structs', label: 'Structs & Aggregates (4)', icon: ShieldCheck }
  ];

  const filtered = DIALECT_ITEMS.filter(item => {
    const matchesCat = selectedCategory === 'all' || item.category === selectedCategory;
    const matchesSearch = item.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
                          item.mlirSyntax.toLowerCase().includes(searchTerm.toLowerCase()) ||
                          item.description.toLowerCase().includes(searchTerm.toLowerCase());
    return matchesCat && matchesSearch;
  });

  return (
    <div className="space-y-6">
      {/* Category Tabs and Filter */}
      <div className="flex flex-wrap items-center justify-between gap-4 bg-slate-900/60 p-4 rounded-xl border border-slate-800">
        <div className="flex flex-wrap gap-2">
          {categories.map(cat => {
            const Icon = cat.icon;
            const isActive = selectedCategory === cat.id;
            return (
              <button
                key={cat.id}
                id={`tab-${cat.id}`}
                onClick={() => setSelectedCategory(cat.id)}
                className={`inline-flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm font-medium transition-colors ${
                  isActive
                    ? 'bg-blue-600 text-white shadow-sm shadow-blue-500/20'
                    : 'bg-slate-800/70 text-slate-300 hover:bg-slate-800 hover:text-white'
                }`}
              >
                <Icon className="w-4 h-4" />
                <span>{cat.label}</span>
              </button>
            );
          })}
        </div>

        <input
          id="search-matrix"
          type="text"
          placeholder="Filter MLIR ops, types, or syntax..."
          value={searchTerm}
          onChange={(e) => setSearchTerm(e.target.value)}
          className="px-3 py-1.5 bg-slate-950 border border-slate-700 rounded-lg text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:border-blue-500 w-full sm:w-64"
        />
      </div>

      {/* Parity Table */}
      <div className="overflow-x-auto rounded-xl border border-slate-800 bg-slate-900/40">
        <table className="w-full text-left text-sm border-collapse">
          <thead>
            <tr className="border-b border-slate-800 bg-slate-900/80 text-xs uppercase tracking-wider text-slate-400">
              <th className="py-3.5 px-4 font-semibold">Construct</th>
              <th className="py-3.5 px-4 font-semibold">CIRCT / MLIR Syntax</th>
              <th className="py-3.5 px-4 font-semibold">pliron-hw Rust API</th>
              <th className="py-3.5 px-4 font-semibold">Semantic Contract & Invariant (AGENTS.md)</th>
              <th className="py-3.5 px-4 font-semibold text-center">Status</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-800/60 font-mono text-xs">
            {filtered.map(item => (
              <tr key={item.id} className="hover:bg-slate-800/30 transition-colors">
                <td className="py-3 px-4 font-sans font-medium text-slate-100 whitespace-nowrap">
                  <div className="flex items-center gap-2">
                    <span className="font-semibold text-blue-400">{item.name}</span>
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-slate-800 text-slate-400 font-mono">
                      {item.category}
                    </span>
                  </div>
                  <p className="font-sans text-xs text-slate-400 mt-1 font-normal max-w-xs">{item.description}</p>
                </td>
                <td className="py-3 px-4 text-emerald-300 font-mono whitespace-nowrap">
                  <div className="bg-slate-950/80 px-2 py-1 rounded border border-slate-800/80 inline-block">
                    {item.mlirSyntax}
                  </div>
                </td>
                <td className="py-3 px-4 text-amber-300 font-mono">
                  <div className="bg-slate-950/80 px-2 py-1 rounded border border-slate-800/80">
                    {item.plironRust}
                  </div>
                </td>
                <td className="py-3 px-4 font-sans text-slate-300 max-w-sm">
                  <p className="text-xs leading-relaxed">{item.semanticContract}</p>
                  <div className="mt-1 text-[11px] text-blue-400/80 font-medium">
                    {item.agentsMdRef}
                  </div>
                </td>
                <td className="py-3 px-4 text-center whitespace-nowrap">
                  <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-medium bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                    <CheckCircle2 className="w-3.5 h-3.5" />
                    Parity Verified
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
};
