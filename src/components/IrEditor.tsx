import React, { useState, useMemo } from 'react';
import { BenchmarkExample, Diagnostic } from '../types';
import { verifyHardwareIr } from '../utils/verifier';
import { Play, RotateCcw, AlertTriangle, CheckCircle2, Copy, Check, FileCode } from 'lucide-react';

interface IrEditorProps {
  benchmarks: BenchmarkExample[];
  selectedBenchmark: BenchmarkExample;
  onSelectBenchmark: (bm: BenchmarkExample) => void;
  irCode: string;
  onIrChange: (code: string) => void;
}

export const IrEditor: React.FC<IrEditorProps> = ({
  benchmarks,
  selectedBenchmark,
  onSelectBenchmark,
  irCode,
  onIrChange,
}) => {
  const [copied, setCopied] = useState(false);

  const diagnostics: Diagnostic[] = useMemo(() => {
    return verifyHardwareIr(irCode);
  }, [irCode]);

  const hasErrors = diagnostics.some(d => d.severity === 'error');
  const hasWarnings = diagnostics.some(d => d.severity === 'warning');

  const handleCopy = () => {
    navigator.clipboard.writeText(irCode);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleReset = () => {
    onIrChange(selectedBenchmark.ir);
  };

  return (
    <div className="space-y-4">
      {/* Top Action Bar */}
      <div className="flex flex-wrap items-center justify-between gap-3 bg-slate-900/70 p-3 rounded-xl border border-slate-800">
        <div className="flex items-center gap-3">
          <label htmlFor="benchmark-select" className="text-xs font-semibold uppercase tracking-wider text-slate-400">
            Canonical Benchmark:
          </label>
          <select
            id="benchmark-select"
            value={selectedBenchmark.id}
            onChange={(e) => {
              const found = benchmarks.find(b => b.id === e.target.value);
              if (found) {
                onSelectBenchmark(found);
                onIrChange(found.ir);
              }
            }}
            className="bg-slate-950 border border-slate-700 text-slate-200 text-xs rounded-lg px-3 py-1.5 focus:outline-none focus:border-blue-500 font-medium"
          >
            {benchmarks.map(b => (
              <option key={b.id} value={b.id}>{b.name}</option>
            ))}
          </select>
        </div>

        <div className="flex items-center gap-2">
          <button
            id="reset-ir-btn"
            onClick={handleReset}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-slate-800 hover:bg-slate-700 text-slate-200 transition-colors"
            title="Reset code to original example"
          >
            <RotateCcw className="w-3.5 h-3.5" />
            Reset
          </button>
          <button
            id="copy-ir-btn"
            onClick={handleCopy}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-slate-800 hover:bg-slate-700 text-slate-200 transition-colors"
          >
            {copied ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
            {copied ? 'Copied' : 'Copy IR'}
          </button>
        </div>
      </div>

      {/* Code Editor and Diagnostics Split */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        {/* Editor Area */}
        <div className="lg:col-span-2 rounded-xl border border-slate-800 bg-slate-950 flex flex-col h-[480px]">
          <div className="px-4 py-2 bg-slate-900/80 border-b border-slate-800 flex items-center justify-between text-xs text-slate-400">
            <span className="flex items-center gap-2 font-mono">
              <FileCode className="w-4 h-4 text-blue-400" />
              hw_dialect.mlir
            </span>
            <span className="text-[11px] font-mono text-slate-500">MLIR Syntax • CIRCT HW Spec</span>
          </div>
          <div className="relative flex-1 flex overflow-hidden">
            <textarea
              id="ir-textarea"
              value={irCode}
              onChange={(e) => onIrChange(e.target.value)}
              spellCheck={false}
              className="w-full h-full p-4 font-mono text-xs leading-relaxed bg-transparent text-slate-100 resize-none focus:outline-none selection:bg-blue-500/30 overflow-auto"
            />
          </div>
        </div>

        {/* Live Verifier Diagnostic Panel */}
        <div className="rounded-xl border border-slate-800 bg-slate-950 flex flex-col h-[480px]">
          <div className="px-4 py-2.5 bg-slate-900/80 border-b border-slate-800 flex items-center justify-between">
            <span className="text-xs font-semibold uppercase tracking-wider text-slate-300 flex items-center gap-2">
              <Play className="w-3.5 h-3.5 text-blue-400" />
              Live Dialect Verifier
            </span>
            <span className={`text-[10px] px-2 py-0.5 rounded-full font-medium ${
              hasErrors
                ? 'bg-rose-500/10 text-rose-400 border border-rose-500/20'
                : hasWarnings
                ? 'bg-amber-500/10 text-amber-400 border border-amber-500/20'
                : 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20'
            }`}>
              {hasErrors ? 'Violations Found' : 'Clean / Valid'}
            </span>
          </div>

          <div className="p-4 space-y-3 overflow-y-auto flex-1 font-sans">
            <div className="text-xs text-slate-400 leading-relaxed">
              Enforces semantic invariants defined in <strong className="text-slate-200">AGENTS.md</strong> (bitwidth preservation, graph region semantics, terminator matching, bounds checking).
            </div>

            <div className="space-y-2 mt-3">
              {diagnostics.map((diag, idx) => (
                <div
                  key={idx}
                  className={`p-3 rounded-lg border text-xs leading-relaxed ${
                    diag.severity === 'error'
                      ? 'bg-rose-950/30 border-rose-800/60 text-rose-200'
                      : diag.severity === 'warning'
                      ? 'bg-amber-950/30 border-amber-800/60 text-amber-200'
                      : 'bg-emerald-950/30 border-emerald-800/60 text-emerald-200'
                  }`}
                >
                  <div className="flex items-start gap-2">
                    {diag.severity === 'error' ? (
                      <AlertTriangle className="w-4 h-4 text-rose-400 shrink-0 mt-0.5" />
                    ) : diag.severity === 'warning' ? (
                      <AlertTriangle className="w-4 h-4 text-amber-400 shrink-0 mt-0.5" />
                    ) : (
                      <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0 mt-0.5" />
                    )}
                    <div>
                      <div className="font-semibold flex items-center gap-2">
                        {diag.severity === 'error' ? 'Invariant Violation' : diag.severity === 'warning' ? 'Warning' : 'Contract Satisfied'}
                        {diag.line > 0 && <span className="text-[10px] opacity-75 font-mono">Line {diag.line}</span>}
                      </div>
                      <p className="mt-1">{diag.message}</p>
                      <div className="mt-1.5 text-[10px] font-mono text-slate-400">
                        Rule: {diag.rule}
                      </div>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
