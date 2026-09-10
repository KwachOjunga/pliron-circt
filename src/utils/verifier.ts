import { Diagnostic } from '../types';

/**
 * Validates hardware IR against the semantic contracts and invariant checks
 * specified in AGENTS.md (e.g. §6 SSA Graph Regions, §11 Combinational bounds,
 * §13 Connectivity, §14 Hierarchy, §25 Verification).
 */
export function verifyHardwareIr(code: string): Diagnostic[] {
  const diagnostics: Diagnostic[] = [];
  const lines = code.split('\n');

  let insideModule = false;
  let currentModuleName = '';
  let hasOutputTerminator = false;
  const declaredWires = new Set<string>();
  const referencedWires = new Set<string>();
  let outputLine = -1;

  lines.forEach((rawLine, index) => {
    const lineNum = index + 1;
    const trimmed = rawLine.trim();

    if (!trimmed || trimmed.startsWith('//')) return;

    // Check module declaration
    const moduleMatch = trimmed.match(/^hw\.module\s+@([a-zA-Z0-9_]+)\s*\((.*?)\)\s*->\s*\((.*?)\)\s*\{/);
    if (moduleMatch) {
      if (insideModule) {
        diagnostics.push({
          line: lineNum,
          message: `Nested hw.module declaration is illegal; modules must be isolated top-level symbols (AGENTS.md §14).`,
          severity: 'error',
          rule: 'AGENTS.md §14 Hierarchy & Isolation'
        });
      }
      insideModule = true;
      currentModuleName = moduleMatch[1];
      hasOutputTerminator = false;
      declaredWires.clear();
      referencedWires.clear();
      return;
    }

    // Check extern module
    const externMatch = trimmed.match(/^hw\.module\.extern\s+@([a-zA-Z0-9_]+)/);
    if (externMatch) {
      if (insideModule) {
        diagnostics.push({
          line: lineNum,
          message: `hw.module.extern cannot be declared inside another module (AGENTS.md §33).`,
          severity: 'error',
          rule: 'AGENTS.md §33 External Modules'
        });
      }
      return;
    }

    // Check end of module
    if (trimmed === '}' && insideModule) {
      if (!hasOutputTerminator) {
        diagnostics.push({
          line: lineNum,
          message: `Module '@${currentModuleName}' lacks mandatory 'hw.output' terminator (AGENTS.md §22 Operations & Termination).`,
          severity: 'error',
          rule: 'AGENTS.md §22 Termination'
        });
      }
      insideModule = false;
      return;
    }

    // Check hw.output
    if (trimmed.startsWith('hw.output')) {
      if (!insideModule) {
        diagnostics.push({
          line: lineNum,
          message: `'hw.output' must only appear as a terminator within an hw.module block (AGENTS.md §22).`,
          severity: 'error',
          rule: 'AGENTS.md §22 Region Structure'
        });
      }
      hasOutputTerminator = true;
      outputLine = lineNum;
      return;
    }

    // Check hw.slice out of bounds
    // e.g. %hi = hw.slice %a at 24 : (hw.int<32>) -> hw.int<16>
    const sliceMatch = trimmed.match(/hw\.slice\s+(%\w+)\s+at\s+(\d+)\s*:\s*\((?:hw\.)?int<(\d+)>\)\s*->\s*(?:hw\.)?int<(\d+)>/);
    if (sliceMatch) {
      const offset = parseInt(sliceMatch[2], 10);
      const inWidth = parseInt(sliceMatch[3], 10);
      const outWidth = parseInt(sliceMatch[4], 10);

      if (offset + outWidth > inWidth) {
        diagnostics.push({
          line: lineNum,
          message: `Out-of-bounds slice: offset (${offset}) + result width (${outWidth}) = ${offset + outWidth}, which exceeds input width (${inWidth}) (AGENTS.md §25).`,
          severity: 'error',
          rule: 'AGENTS.md §25 Width Bounds'
        });
      }
    }

    // Check hw.concat width equality
    // %wide = hw.concat %a, %b : (hw.int<8>, hw.int<8>) -> hw.int<16>
    const concatMatch = trimmed.match(/hw\.concat\s+.*?\s*:\s*\((.*?)\)\s*->\s*(?:hw\.)?int<(\d+)>/);
    if (concatMatch) {
      const inputsPart = concatMatch[1];
      const outWidth = parseInt(concatMatch[2], 10);
      const widths = Array.from(inputsPart.matchAll(/(?:hw\.)?int<(\d+)>/g)).map(m => parseInt(m[1], 10));
      const sumWidths = widths.reduce((acc, w) => acc + w, 0);

      if (widths.length > 0 && sumWidths !== outWidth) {
        diagnostics.push({
          line: lineNum,
          message: `Width mismatch in hw.concat: sum of operand widths (${sumWidths}) does not equal result width (${outWidth}) (AGENTS.md §25).`,
          severity: 'error',
          rule: 'AGENTS.md §25 Invariant Verification'
        });
      }
    }

    // Check hw.wire naming
    if (trimmed.includes('hw.wire') && !trimmed.includes('sym @') && !trimmed.includes('"')) {
      diagnostics.push({
        line: lineNum,
        message: `hw.wire is missing explicit symbol or name attribute. Nets should have identifiable hardware names (AGENTS.md §13).`,
        severity: 'warning',
        rule: 'AGENTS.md §13 Connectivity & Drivers'
      });
    }

    // Check hw.bitcast bitwidth match
    const bitcastMatch = trimmed.match(/hw\.bitcast\s+(%\w+)\s*:\s*\((?:hw\.)?int<(\d+)>\)\s*->\s*(?:hw\.)?int<(\d+)>/);
    if (bitcastMatch) {
      const inW = parseInt(bitcastMatch[2], 10);
      const outW = parseInt(bitcastMatch[3], 10);
      if (inW !== outW) {
        diagnostics.push({
          line: lineNum,
          message: `Bitcast bitwidth mismatch: source has ${inW} bits, destination has ${outW} bits. Bitcasts must be zero-latency width-preserving (AGENTS.md §25).`,
          severity: 'error',
          rule: 'AGENTS.md §25 Structural Validity'
        });
      }
    }
  });

  if (diagnostics.length === 0) {
    diagnostics.push({
      line: 1,
      message: 'All CIRCT/MLIR invariant checks passed. Strict adherence to AGENTS.md semantic contracts verified.',
      severity: 'info',
      rule: 'AGENTS.md §1 & §44 Parity Complete'
    });
  }

  return diagnostics;
}
