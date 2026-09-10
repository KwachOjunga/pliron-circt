import React, { useState } from 'react';
import { NetlistNode, NetlistEdge } from '../types';
import { Network, Info, Eye } from 'lucide-react';

interface NetlistVisualizerProps {
  nodes: NetlistNode[];
  edges: NetlistEdge[];
  moduleName: string;
}

export const NetlistVisualizer: React.FC<NetlistVisualizerProps> = ({ nodes, edges, moduleName }) => {
  const [selectedNode, setSelectedNode] = useState<NetlistNode | null>(null);

  // Compute node bounding box or positions
  const maxX = Math.max(...nodes.map(n => n.x + 160), 960);
  const maxY = Math.max(...nodes.map(n => n.y + 120), 440);

  const getNodeColor = (kind: NetlistNode['kind']) => {
    switch (kind) {
      case 'port_in': return 'stroke-emerald-500/80 fill-emerald-950/40 text-emerald-300';
      case 'port_out': return 'stroke-amber-500/80 fill-amber-950/40 text-amber-300';
      case 'instance': return 'stroke-purple-500/80 fill-purple-950/40 text-purple-300';
      case 'comb_op': return 'stroke-blue-500/80 fill-blue-950/40 text-blue-300';
      case 'array_op': return 'stroke-cyan-500/80 fill-cyan-950/40 text-cyan-300';
      case 'struct_op': return 'stroke-rose-500/80 fill-rose-950/40 text-rose-300';
      case 'wire': return 'stroke-indigo-500/80 fill-indigo-950/40 text-indigo-300';
      default: return 'stroke-slate-600 fill-slate-900 text-slate-300';
    }
  };

  return (
    <div className="space-y-4">
      {/* Header and Legend */}
      <div className="flex flex-wrap items-center justify-between gap-3 bg-slate-900/60 p-3 rounded-xl border border-slate-800 text-xs">
        <div className="flex items-center gap-2 font-medium text-slate-200">
          <Network className="w-4 h-4 text-blue-400" />
          <span>Schematic Netlist View:</span>
          <span className="font-mono text-blue-400 bg-blue-500/10 px-2 py-0.5 rounded border border-blue-500/20">
            @{moduleName}
          </span>
        </div>

        {/* Legend */}
        <div className="flex flex-wrap items-center gap-3 text-[11px] text-slate-400">
          <span className="inline-flex items-center gap-1.5">
            <span className="w-2.5 h-2.5 rounded-sm bg-emerald-500/40 border border-emerald-500" /> Input Ports
          </span>
          <span className="inline-flex items-center gap-1.5">
            <span className="w-2.5 h-2.5 rounded-sm bg-amber-500/40 border border-amber-500" /> Output Ports
          </span>
          <span className="inline-flex items-center gap-1.5">
            <span className="w-2.5 h-2.5 rounded-sm bg-purple-500/40 border border-purple-500" /> Submodule Instances
          </span>
          <span className="inline-flex items-center gap-1.5">
            <span className="w-2.5 h-2.5 rounded-sm bg-blue-500/40 border border-blue-500" /> Slice/Concat Slices
          </span>
          <span className="inline-flex items-center gap-1.5">
            <span className="w-2.5 h-2.5 rounded-sm bg-cyan-500/40 border border-cyan-500" /> Arrays & Muxes
          </span>
          <span className="inline-flex items-center gap-1.5">
            <span className="w-2.5 h-2.5 rounded-sm bg-rose-500/40 border border-rose-500" /> Struct Aggregates
          </span>
        </div>
      </div>

      {/* Interactive SVG Diagram */}
      <div className="relative rounded-xl border border-slate-800 bg-slate-950 overflow-x-auto min-h-[440px] p-4 flex justify-center">
        <svg
          viewBox={`0 0 ${maxX} ${maxY}`}
          className="w-full max-w-5xl h-auto"
          style={{ minWidth: '760px', height: '420px' }}
        >
          <defs>
            <marker
              id="arrow"
              viewBox="0 0 10 10"
              refX="8"
              refY="5"
              markerWidth="6"
              markerHeight="6"
              orient="auto-start-reverse"
            >
              <path d="M 0 1 L 10 5 L 0 9 z" fill="#64748b" />
            </marker>
          </defs>

          {/* Render Wire Edges */}
          {edges.map(edge => {
            const src = nodes.find(n => n.id === edge.sourceNodeId);
            const dst = nodes.find(n => n.id === edge.targetNodeId);
            if (!src || !dst) return null;

            const x1 = src.x + 130;
            const y1 = src.y + 30;
            const x2 = dst.x;
            const y2 = dst.y + 30;
            const dx = Math.max(Math.abs(x2 - x1) * 0.5, 30);

            const path = `M ${x1} ${y1} C ${x1 + dx} ${y1}, ${x2 - dx} ${y2}, ${x2} ${y2}`;

            return (
              <g key={edge.id} className="group">
                <path
                  d={path}
                  fill="none"
                  stroke="#334155"
                  strokeWidth="6"
                  className="opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer"
                />
                <path
                  d={path}
                  fill="none"
                  stroke={edge.width && edge.width > 1 ? '#38bdf8' : '#94a3b8'}
                  strokeWidth={edge.width && edge.width > 1 ? 2.5 : 1.5}
                  strokeDasharray={edge.width && edge.width > 1 ? 'none' : '4 2'}
                  markerEnd="url(#arrow)"
                />
                {/* Bus width badge */}
                {edge.width && edge.width > 1 && (
                  <g transform={`translate(${(x1 + x2) / 2}, ${(y1 + y2) / 2 - 8})`}>
                    <rect x="-14" y="-8" width="28" height="15" rx="3" fill="#0f172a" stroke="#38bdf8" strokeWidth="0.8" />
                    <text
                      x="0"
                      y="3"
                      fill="#38bdf8"
                      fontSize="9"
                      fontFamily="monospace"
                      textAnchor="middle"
                    >
                      /{edge.width}
                    </text>
                  </g>
                )}
              </g>
            );
          })}

          {/* Render Hardware Nodes */}
          {nodes.map(node => {
            const isSelected = selectedNode?.id === node.id;
            const colorClass = getNodeColor(node.kind);

            return (
              <g
                key={node.id}
                id={`netlist-node-${node.id}`}
                transform={`translate(${node.x}, ${node.y})`}
                onClick={() => setSelectedNode(node)}
                className="cursor-pointer group"
              >
                {/* Node Box */}
                <rect
                  x="0"
                  y="0"
                  width="130"
                  height="60"
                  rx="8"
                  className={`${colorClass} transition-all duration-200 stroke-1.5 ${
                    isSelected ? 'ring-2 ring-blue-400 stroke-blue-400' : 'hover:stroke-blue-400'
                  }`}
                />
                {/* Label */}
                <text
                  x="12"
                  y="26"
                  fill="#f8fafc"
                  fontSize="11"
                  fontWeight="600"
                  fontFamily="system-ui, sans-serif"
                >
                  {node.label.length > 15 ? node.label.substring(0, 14) + '...' : node.label}
                </text>
                <text
                  x="12"
                  y="44"
                  fill="#94a3b8"
                  fontSize="9.5"
                  fontFamily="monospace"
                >
                  {node.op}
                </text>

                {/* Ports */}
                {node.ports.map((port, pIdx) => {
                  const isOut = port.direction === 'out';
                  const px = isOut ? 130 : 0;
                  const py = 30 + (pIdx - (node.ports.length - 1) / 2) * 14;
                  return (
                    <circle
                      key={port.id}
                      cx={px}
                      cy={py}
                      r="4"
                      className={`${isOut ? 'fill-blue-400 stroke-slate-900' : 'fill-emerald-400 stroke-slate-900'} stroke-2`}
                    />
                  );
                })}
              </g>
            );
          })}
        </svg>

        {/* Node Inspection Drawer */}
        {selectedNode && (
          <div className="absolute right-4 bottom-4 w-72 bg-slate-900/95 border border-slate-700 rounded-xl p-3 shadow-xl backdrop-blur-sm text-xs space-y-2">
            <div className="flex items-center justify-between border-b border-slate-800 pb-2">
              <span className="font-semibold text-slate-100 flex items-center gap-1.5">
                <Info className="w-3.5 h-3.5 text-blue-400" />
                Node Details
              </span>
              <button
                onClick={() => setSelectedNode(null)}
                className="text-slate-400 hover:text-slate-200 text-xs px-1.5 py-0.5 rounded bg-slate-800"
              >
                Close
              </button>
            </div>
            <div className="space-y-1 font-mono text-[11px]">
              <div><span className="text-slate-400 font-sans">Op:</span> <span className="text-blue-300">{selectedNode.op}</span></div>
              <div><span className="text-slate-400 font-sans">Label:</span> <span className="text-slate-200">{selectedNode.label}</span></div>
              <div><span className="text-slate-400 font-sans">Category:</span> <span className="text-amber-300">{selectedNode.kind}</span></div>
            </div>
            <div className="border-t border-slate-800 pt-2">
              <span className="text-[11px] font-semibold text-slate-400">Ports ({selectedNode.ports.length}):</span>
              <div className="mt-1 space-y-1">
                {selectedNode.ports.map(p => (
                  <div key={p.id} className="flex items-center justify-between text-[10.5px] font-mono bg-slate-950 px-2 py-1 rounded">
                    <span className={p.direction === 'in' ? 'text-emerald-400' : 'text-blue-400'}>{p.name} [{p.direction}]</span>
                    <span className="text-slate-400">{p.type}</span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
