import React, { useState } from 'react';
import { BENCHMARKS } from './data/dialectData';
import { ParityMatrix } from './components/ParityMatrix';
import { IrEditor } from './components/IrEditor';
import { NetlistVisualizer } from './components/NetlistVisualizer';
import { RustCrateInspector } from './components/RustCrateInspector';
import { AgentsContractAudit } from './components/AgentsContractAudit';
import { Cpu, CheckCircle2, FileText, Network, Code, ShieldCheck, Box } from 'lucide-react';

type TabType = 'matrix' | 'editor' | 'netlist' | 'rust' | 'audit';

export default function App() {
  const [activeTab, setActiveTab] = useState<TabType>('matrix');
  const [selectedBenchmark, setSelectedBenchmark] = useState(BENCHMARKS[0]);
  const [irCode, setIrCode] = useState(BENCHMARKS[0].ir);

  return (
    <div className="min-h-screen bg-slate-950 text-slate-100 flex flex-col font-sans selection:bg-blue-500/30">
      {/* Top Navigation Bar */}
      <header className="border-b border-slate-800 bg-slate-900/60 backdrop-blur-md sticky top-0 z-50">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 h-16 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-lg bg-blue-600/20 border border-blue-500/40 flex items-center justify-center text-blue-400">
              <Cpu className="w-5 h-5" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <span className="font-bold text-base tracking-tight text-white">pliron-hw</span>
                <span className="text-[11px] font-mono px-2 py-0.5 rounded-full bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                  CIRCT / MLIR Parity Complete
                </span>
              </div>
              <p className="text-xs text-slate-400">Hardware Dialect Ecosystem for pliron</p>
            </div>
          </div>

          <div className="hidden md:flex items-center gap-4 text-xs">
            <div className="flex items-center gap-1.5 text-slate-400 font-mono">
              <span className="w-2 h-2 rounded-full bg-emerald-400" />
              <span>AGENTS.md Contract Compliant</span>
            </div>
            <a
              href="https://circt.llvm.org/docs/Dialects/HW/"
              target="_blank"
              rel="noreferrer"
              className="text-blue-400 hover:text-blue-300 transition-colors"
            >
              CIRCT HW Spec ↗
            </a>
          </div>
        </div>
      </header>

      {/* Main Content Container */}
      <main className="flex-1 max-w-7xl w-full mx-auto px-4 sm:px-6 lg:px-8 py-6 space-y-6">
        {/* Quick Highlights / Parity Stats */}
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
          <div className="p-3.5 rounded-xl border border-slate-800/80 bg-slate-900/40 flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-blue-500/10 border border-blue-500/20 flex items-center justify-center text-blue-400">
              <Box className="w-4 h-4" />
            </div>
            <div>
              <div className="text-lg font-bold text-slate-100">7 Types</div>
              <div className="text-[11px] text-slate-400">Int, Inout, Array, Struct, Union...</div>
            </div>
          </div>

          <div className="p-3.5 rounded-xl border border-slate-800/80 bg-slate-900/40 flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center text-emerald-400">
              <Cpu className="w-4 h-4" />
            </div>
            <div>
              <div className="text-lg font-bold text-slate-100">18 Ops</div>
              <div className="text-[11px] text-slate-400">Modules, Wires, Bitcast, Slices...</div>
            </div>
          </div>

          <div className="p-3.5 rounded-xl border border-slate-800/80 bg-slate-900/40 flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-purple-500/10 border border-purple-500/20 flex items-center justify-center text-purple-400">
              <Network className="w-4 h-4" />
            </div>
            <div>
              <div className="text-lg font-bold text-slate-100">Graph Region</div>
              <div className="text-[11px] text-slate-400">has_ssa_dominance = false</div>
            </div>
          </div>

          <div className="p-3.5 rounded-xl border border-slate-800/80 bg-slate-900/40 flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-amber-500/10 border border-amber-500/20 flex items-center justify-center text-amber-400">
              <ShieldCheck className="w-4 h-4" />
            </div>
            <div>
              <div className="text-lg font-bold text-slate-100">100% Parity</div>
              <div className="text-[11px] text-slate-400">Validated against MLIR CIRCT</div>
            </div>
          </div>
        </div>

        {/* View Switcher Tabs */}
        <div className="flex border-b border-slate-800 space-x-1 overflow-x-auto text-sm">
          <button
            id="nav-tab-matrix"
            onClick={() => setActiveTab('matrix')}
            className={`flex items-center gap-2 py-3 px-4 font-medium border-b-2 transition-colors whitespace-nowrap ${
              activeTab === 'matrix'
                ? 'border-blue-500 text-blue-400 bg-slate-900/30'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <CheckCircle2 className="w-4 h-4" />
            MLIR Parity Matrix
          </button>

          <button
            id="nav-tab-editor"
            onClick={() => setActiveTab('editor')}
            className={`flex items-center gap-2 py-3 px-4 font-medium border-b-2 transition-colors whitespace-nowrap ${
              activeTab === 'editor'
                ? 'border-blue-500 text-blue-400 bg-slate-900/30'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <FileText className="w-4 h-4" />
            Interactive IR & Verifier
          </button>

          <button
            id="nav-tab-netlist"
            onClick={() => setActiveTab('netlist')}
            className={`flex items-center gap-2 py-3 px-4 font-medium border-b-2 transition-colors whitespace-nowrap ${
              activeTab === 'netlist'
                ? 'border-blue-500 text-blue-400 bg-slate-900/30'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <Network className="w-4 h-4" />
            Schematic Netlist View
          </button>

          <button
            id="nav-tab-rust"
            onClick={() => setActiveTab('rust')}
            className={`flex items-center gap-2 py-3 px-4 font-medium border-b-2 transition-colors whitespace-nowrap ${
              activeTab === 'rust'
                ? 'border-blue-500 text-blue-400 bg-slate-900/30'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <Code className="w-4 h-4" />
            pliron-hw Rust Crate
          </button>

          <button
            id="nav-tab-audit"
            onClick={() => setActiveTab('audit')}
            className={`flex items-center gap-2 py-3 px-4 font-medium border-b-2 transition-colors whitespace-nowrap ${
              activeTab === 'audit'
                ? 'border-blue-500 text-blue-400 bg-slate-900/30'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <ShieldCheck className="w-4 h-4" />
            AGENTS.md Contract Audit
          </button>
        </div>

        {/* Tab Content Panes */}
        <div className="pt-2">
          {activeTab === 'matrix' && <ParityMatrix />}

          {activeTab === 'editor' && (
            <IrEditor
              benchmarks={BENCHMARKS}
              selectedBenchmark={selectedBenchmark}
              onSelectBenchmark={(bm) => {
                setSelectedBenchmark(bm);
                setIrCode(bm.ir);
              }}
              irCode={irCode}
              onIrChange={setIrCode}
            />
          )}

          {activeTab === 'netlist' && (
            <NetlistVisualizer
              nodes={selectedBenchmark.nodes}
              edges={selectedBenchmark.edges}
              moduleName={selectedBenchmark.modules[0]}
            />
          )}

          {activeTab === 'rust' && <RustCrateInspector />}

          {activeTab === 'audit' && <AgentsContractAudit />}
        </div>
      </main>
    </div>
  );
}
