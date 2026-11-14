**Yes. In fact, I think mold's architectural philosophy is highly applicable to a SystemVerilog front end, particularly one targeting Pliron-CIRCT.** However, I would adapt its data-parallel architecture rather than copy its linker passes literally.

The key insight from Rui Ueyama's paper is that parallelism should influence the design of the intermediate data structures and processing phases from the beginning. Mold achieves this by separating parsing from global symbol resolution and making subsequent passes parallel loops over homogeneous collections of data.&#x20;

[image](https://www.google.com/s2/favicons?domain=https://arxiv.org\&sz=32)

arXiv+1



For SystemVerilog, I would make one architectural distinction:

- **Compilation:** Understand individual source constructs and their relationships.
- **Elaboration:** Resolve parameters, hierarchy, generate constructs and concrete hardware structure.
- **Lowering:** Translate the resulting hardware model into Pliron-CIRCT.

These stages have different dependency structures. SystemVerilog also has compilation-unit and package-ordering rules that prevent treating every file as completely independent.&#x20;

[image](https://www.google.com/s2/favicons?domain=https://iccircle.com\&sz=32)

iccircle.com



## **1. A mold-inspired SystemVerilog architecture**

\#chatgpt-mermaid-\_r_id\_{font-family:-apple-system-body,ui-sans-serif,-apple-system,system-ui,Segoe UI,Helvetica,Apple Color Emoji,Arial,sans-serif,Segoe UI Emoji,Segoe UI Symbol;font-size:16px;fill:rgb(237, 237, 237);}@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}#chatgpt-mermaid-\_r_id\_ .edge-animation-slow{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}#chatgpt-mermaid-\_r_id\_ .edge-animation-fast{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}#chatgpt-mermaid-\_r_id\_ .error-icon{fill:rgb(48, 48, 48);}#chatgpt-mermaid-\_r_id\_ .error-text{fill:rgb(237, 237, 237);stroke:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_id\_ .edge-thickness-normal{stroke-width:1px;}#chatgpt-mermaid-\_r_id\_ .edge-thickness-thick{stroke-width:3.5px;}#chatgpt-mermaid-\_r_id\_ .edge-pattern-solid{stroke-dasharray:0;}#chatgpt-mermaid-\_r_id\_ .edge-thickness-invisible{stroke-width:0;fill:none;}#chatgpt-mermaid-\_r_id\_ .edge-pattern-dashed{stroke-dasharray:3;}#chatgpt-mermaid-\_r_id\_ .edge-pattern-dotted{stroke-dasharray:2;}#chatgpt-mermaid-\_r_id\_ .marker{fill:rgb(175, 175, 175);stroke:rgb(175, 175, 175);}#chatgpt-mermaid-\_r_id\_ .marker.cross{stroke:rgb(175, 175, 175);}#chatgpt-mermaid-\_r_id\_ svg{font-family:-apple-system-body,ui-sans-serif,-apple-system,system-ui,Segoe UI,Helvetica,Apple Color Emoji,Arial,sans-serif,Segoe UI Emoji,Segoe UI Symbol;font-size:16px;}#chatgpt-mermaid-\_r_id\_ p{margin:0;}#chatgpt-mermaid-\_r_id\_ .label{font-family:-apple-system-body,ui-sans-serif,-apple-system,system-ui,Segoe UI,Helvetica,Apple Color Emoji,Arial,sans-serif,Segoe UI Emoji,Segoe UI Symbol;color:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_id\_ .cluster-label text{fill:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_id\_ .cluster-label span{color:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_id\_ .cluster-label span p{background-color:transparent;}#chatgpt-mermaid-\_r_id\_ .label text,#chatgpt-mermaid-\_r_id\_ span{fill:rgb(237, 237, 237);color:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_id\_ .node rect,#chatgpt-mermaid-\_r_id\_ .node circle,#chatgpt-mermaid-\_r_id\_ .node ellipse,#chatgpt-mermaid-\_r_id\_ .node polygon,#chatgpt-mermaid-\_r_id\_ .node path{fill:rgb(9, 23, 44);stroke:rgb(31, 78, 148);stroke-width:1px;}#chatgpt-mermaid-\_r_id\_ .rough-node .label text,#chatgpt-mermaid-\_r_id\_ .node .label text,#chatgpt-mermaid-\_r_id\_ .image-shape .label,#chatgpt-mermaid-\_r_id\_ .icon-shape .label{text-anchor:middle;}#chatgpt-mermaid-\_r_id\_ .node .katex path{fill:#000;stroke:#000;stroke-width:1px;}#chatgpt-mermaid-\_r_id\_ .rough-node .label,#chatgpt-mermaid-\_r_id\_ .node .label,#chatgpt-mermaid-\_r_id\_ .image-shape .label,#chatgpt-mermaid-\_r_id\_ .icon-shape .label{text-align:center;}#chatgpt-mermaid-\_r_id\_ .node.clickable{cursor:pointer;}#chatgpt-mermaid-\_r_id\_ .root .anchor path{fill:rgb(175, 175, 175)!important;stroke-width:0;stroke:rgb(175, 175, 175);}#chatgpt-mermaid-\_r_id\_ .arrowheadPath{fill:rgb(175, 175, 175);}#chatgpt-mermaid-\_r_id\_ .edgePath .path{stroke:rgb(175, 175, 175);stroke-width:1px;}#chatgpt-mermaid-\_r_id\_ .flowchart-link{stroke:rgb(175, 175, 175);fill:none;}#chatgpt-mermaid-\_r_id\_ .edgeLabel{background-color:rgb(0, 0, 0);text-align:center;}#chatgpt-mermaid-\_r_id\_ .edgeLabel p{background-color:rgb(0, 0, 0);}#chatgpt-mermaid-\_r_id\_ .edgeLabel rect{opacity:0.5;background-color:rgb(0, 0, 0);fill:rgb(0, 0, 0);}#chatgpt-mermaid-\_r_id\_ .labelBkg{background-color:rgba(0, 0, 0, 0.5);}#chatgpt-mermaid-\_r_id\_ .cluster rect{fill:rgb(48, 48, 48);stroke:rgba(255, 255, 255, 0.15);stroke-width:1px;}#chatgpt-mermaid-\_r_id\_ .cluster text{fill:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_id\_ .cluster span{color:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_id\_ div.mermaidTooltip{position:absolute;text-align:center;max-width:200px;padding:2px;font-family:-apple-system-body,ui-sans-serif,-apple-system,system-ui,Segoe UI,Helvetica,Apple Color Emoji,Arial,sans-serif,Segoe UI Emoji,Segoe UI Symbol;font-size:12px;background:rgb(48, 48, 48);border:1px solid rgba(255, 255, 255, 0.15);border-radius:2px;pointer-events:none;z-index:100;}#chatgpt-mermaid-\_r_id\_ .flowchartTitleText{text-anchor:middle;font-size:18px;fill:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_id\_ rect.text{fill:none;stroke-width:0;}#chatgpt-mermaid-\_r_id\_ .icon-shape,#chatgpt-mermaid-\_r_id\_ .image-shape{background-color:rgb(0, 0, 0);text-align:center;}#chatgpt-mermaid-\_r_id\_ .icon-shape p,#chatgpt-mermaid-\_r_id\_ .image-shape p{background-color:rgb(0, 0, 0);padding:2px;}#chatgpt-mermaid-\_r_id\_ .icon-shape .label rect,#chatgpt-mermaid-\_r_id\_ .image-shape .label rect{opacity:0.5;background-color:rgb(0, 0, 0);fill:rgb(0, 0, 0);}#chatgpt-mermaid-\_r_id\_ .label-icon{display:inline-block;height:1em;overflow:visible;vertical-align:-0.125em;}#chatgpt-mermaid-\_r_id\_ .node .label-icon path{fill:currentColor;stroke:revert;stroke-width:revert;}#chatgpt-mermaid-\_r_id\_ .node .neo-node{stroke:rgb(31, 78, 148);}#chatgpt-mermaid-\_r_id\_ [data-look="neo"].node rect,#chatgpt-mermaid-\_r_id\_ [data-look="neo"].cluster rect,#chatgpt-mermaid-\_r_id\_ [data-look="neo"].node polygon{stroke:url(#chatgpt-mermaid-\_r_id\_-gradient);filter:drop-shadow( 1px 2px 2px rgba(185,185,185,1));}#chatgpt-mermaid-\_r_id\_ [data-look="neo"].swimlane.cluster rect{filter:none;}#chatgpt-mermaid-\_r_id\_ [data-look="neo"].node path{stroke:url(#chatgpt-mermaid-\_r_id\_-gradient);stroke-width:1px;}#chatgpt-mermaid-\_r_id\_ [data-look="neo"].node .outer-path{filter:drop-shadow( 1px 2px 2px rgba(185,185,185,1));}#chatgpt-mermaid-\_r_id\_ [data-look="neo"].node .neo-line path{stroke:rgb(31, 78, 148);filter:none;}#chatgpt-mermaid-\_r_id\_ [data-look="neo"].node circle{stroke:url(#chatgpt-mermaid-\_r_id\_-gradient);filter:drop-shadow( 1px 2px 2px rgba(185,185,185,1));}#chatgpt-mermaid-\_r_id\_ [data-look="neo"].node circle .state-start{fill:#000000;}#chatgpt-mermaid-\_r_id\_ [data-look="neo"].icon-shape .icon{fill:url(#chatgpt-mermaid-\_r_id\_-gradient);filter:drop-shadow( 1px 2px 2px rgba(185,185,185,1));}#chatgpt-mermaid-\_r_id\_ [data-look="neo"].icon-shape .icon-neo path{stroke:url(#chatgpt-mermaid-\_r_id\_-gradient);filter:drop-shadow( 1px 2px 2px rgba(185,185,185,1));}#chatgpt-mermaid-\_r_id\_ .node text{font-size:14px;font-weight:600;letter-spacing:normal;fill:rgb(153, 206, 255);}#chatgpt-mermaid-\_r_id\_ .edgeLabels text{font-size:13px;font-weight:600;letter-spacing:-0.08px;fill:rgb(153, 206, 255);}#chatgpt-mermaid-\_r_id\_ .node tspan[font-weight="normal"],#chatgpt-mermaid-\_r_id\_ .edgeLabels tspan[font-weight="normal"]{font-weight:600;}#chatgpt-mermaid-\_r_id\_ .edgeLabel .label rect{opacity:1;rx:13px;ry:13px;fill:rgb(0, 14, 26);stroke:rgb(26, 62, 95);stroke-width:1px;}#chatgpt-mermaid-\_r_id\_ .node rect,#chatgpt-mermaid-\_r_id\_ .node circle,#chatgpt-mermaid-\_r_id\_ .node ellipse,#chatgpt-mermaid-\_r_id\_ .node polygon,#chatgpt-mermaid-\_r_id\_ .node path{fill:rgb(0, 40, 77);stroke:rgba(255, 255, 255, 0.1);stroke-width:1px;}#chatgpt-mermaid-\_r_id\_ .node rect{rx:16px;ry:16px;}#chatgpt-mermaid-\_r_id\_ .mermaid-decision .label-container{fill:rgb(0, 14, 26);stroke:rgb(26, 62, 95);stroke-dasharray:2px,2px;}#chatgpt-mermaid-\_r_id\_ .edgePaths .flowchart-link{stroke:rgb(175, 175, 175);stroke-width:1px;stroke-linecap:round;stroke-linejoin:round;}#chatgpt-mermaid-\_r_id\_ .marker{fill:rgb(175, 175, 175);stroke:rgb(175, 175, 175);}#chatgpt-mermaid-\_r_id\_ :root{--mermaid-font-family:-apple-system-body,ui-sans-serif,-apple-system,system-ui,Segoe UI,Helvetica,Apple Color Emoji,Arial,sans-serif,Segoe UI Emoji,Segoe UI Symbol;}SystemVerilog ProjectInput DiscoveryPreprocessingParallel LexingParallel ParsingSyntax IR / ASTDeclaration CollectionGlobal Symbol ResolutionType and Semantic AnalysisElaborationHardware Dependency GraphReachability AnalysisStructural AnalysisPliron-CIRCT ConstructionVerification andCanonicalization

The important architectural feature is that the front end does not immediately construct the final hardware IR while parsing individual files.

Instead, it first collects information into representations that make global analysis possible.

## **2. How I would adapt individual mold techniques**

This is where the adaptation becomes interesting.

| Mold technique             | SystemVerilog equivalent                                   | Potential benefit                   |
| -------------------------- | ---------------------------------------------------------- | ----------------------------------- |
| Eager input parsing        | Parse preprocessed compilation units upfront               | Expose parallelism early            |
| Symbol interning           | Intern identifiers, package names and type names           | Reduce string comparisons           |
| Atomic symbol resolution   | Parallel declaration indexing and deterministic resolution | Scale large symbol tables           |
| Archive liveness traversal | Module and instance reachability                           | Remove irrelevant design components |
| Relocation graph analysis  | Signal and module dependency analysis                      | Expose hardware relationships       |
| Identical code folding     | Structural hardware equivalence analysis                   | Identify reusable structures        |
| Parallel prefix sums       | Assign IDs, offsets and storage ranges                     | Efficient bulk construction         |
| Parallel output generation | Construct independent module IR fragments                  | Reduce serialization bottlenecks    |

There are some important differences, though.

### **A. Parsing: parallelise after preprocessing**

This is probably the first place I would be careful.

SystemVerilog preprocessing is stateful.

Consider:

```
`define DATA_WIDTH 32
`include "registers.sv"
```

The included file inherits the macro environment of its inclusion context.

Consequently, you cannot safely preprocess every file independently.

I would instead introduce a compilation-unit abstraction:

```

struct CompilationUnit {
    files: Vec<SourceFileId>,
    macro_environment: MacroEnvironment,
    include_dependencies: Vec<SourceFileId>,
}

```

Then:

1. Establish compilation-unit boundaries.
2. Process preprocessing directives according to their required ordering.
3. Produce preprocessed source buffers with source-location mappings.
4. Parse independent compilation units concurrently.
5. Store their syntax trees in independent arenas.

This is an excellent opportunity for parallel parsing without violating preprocessing semantics.

### **B. Symbol resolution: borrow mold's most important idea**

Suppose your project contains:

```
// alu.sv
module alu;
endmodule

// cpu.sv
module cpu;
    alu alu_inst();
endmodule

// system.sv
module system;
    cpu cpu_inst();
endmodule
```

A conventional implementation might discover declarations and resolve references while traversing files sequentially.

Instead, use two distinct passes.

**Pass 1: Declaration collection**

Every parsed compilation unit contributes declarations to a global index.

```
Module: alu
Module: cpu
Module: system
Instance: cpu.alu_inst → alu
Instance: system.cpu_inst → cpu
```

**Pass 2: Reference resolution**

Resolve references against the completed declaration index.

The resulting graph is:

\#chatgpt-mermaid-\_r_j8\_{font-family:-apple-system-body,ui-sans-serif,-apple-system,system-ui,Segoe UI,Helvetica,Apple Color Emoji,Arial,sans-serif,Segoe UI Emoji,Segoe UI Symbol;font-size:16px;fill:rgb(237, 237, 237);}@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}#chatgpt-mermaid-\_r_j8\_ .edge-animation-slow{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}#chatgpt-mermaid-\_r_j8\_ .edge-animation-fast{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}#chatgpt-mermaid-\_r_j8\_ .error-icon{fill:rgb(48, 48, 48);}#chatgpt-mermaid-\_r_j8\_ .error-text{fill:rgb(237, 237, 237);stroke:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_j8\_ .edge-thickness-normal{stroke-width:1px;}#chatgpt-mermaid-\_r_j8\_ .edge-thickness-thick{stroke-width:3.5px;}#chatgpt-mermaid-\_r_j8\_ .edge-pattern-solid{stroke-dasharray:0;}#chatgpt-mermaid-\_r_j8\_ .edge-thickness-invisible{stroke-width:0;fill:none;}#chatgpt-mermaid-\_r_j8\_ .edge-pattern-dashed{stroke-dasharray:3;}#chatgpt-mermaid-\_r_j8\_ .edge-pattern-dotted{stroke-dasharray:2;}#chatgpt-mermaid-\_r_j8\_ .marker{fill:rgb(175, 175, 175);stroke:rgb(175, 175, 175);}#chatgpt-mermaid-\_r_j8\_ .marker.cross{stroke:rgb(175, 175, 175);}#chatgpt-mermaid-\_r_j8\_ svg{font-family:-apple-system-body,ui-sans-serif,-apple-system,system-ui,Segoe UI,Helvetica,Apple Color Emoji,Arial,sans-serif,Segoe UI Emoji,Segoe UI Symbol;font-size:16px;}#chatgpt-mermaid-\_r_j8\_ p{margin:0;}#chatgpt-mermaid-\_r_j8\_ .label{font-family:-apple-system-body,ui-sans-serif,-apple-system,system-ui,Segoe UI,Helvetica,Apple Color Emoji,Arial,sans-serif,Segoe UI Emoji,Segoe UI Symbol;color:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_j8\_ .cluster-label text{fill:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_j8\_ .cluster-label span{color:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_j8\_ .cluster-label span p{background-color:transparent;}#chatgpt-mermaid-\_r_j8\_ .label text,#chatgpt-mermaid-\_r_j8\_ span{fill:rgb(237, 237, 237);color:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_j8\_ .node rect,#chatgpt-mermaid-\_r_j8\_ .node circle,#chatgpt-mermaid-\_r_j8\_ .node ellipse,#chatgpt-mermaid-\_r_j8\_ .node polygon,#chatgpt-mermaid-\_r_j8\_ .node path{fill:rgb(9, 23, 44);stroke:rgb(31, 78, 148);stroke-width:1px;}#chatgpt-mermaid-\_r_j8\_ .rough-node .label text,#chatgpt-mermaid-\_r_j8\_ .node .label text,#chatgpt-mermaid-\_r_j8\_ .image-shape .label,#chatgpt-mermaid-\_r_j8\_ .icon-shape .label{text-anchor:middle;}#chatgpt-mermaid-\_r_j8\_ .node .katex path{fill:#000;stroke:#000;stroke-width:1px;}#chatgpt-mermaid-\_r_j8\_ .rough-node .label,#chatgpt-mermaid-\_r_j8\_ .node .label,#chatgpt-mermaid-\_r_j8\_ .image-shape .label,#chatgpt-mermaid-\_r_j8\_ .icon-shape .label{text-align:center;}#chatgpt-mermaid-\_r_j8\_ .node.clickable{cursor:pointer;}#chatgpt-mermaid-\_r_j8\_ .root .anchor path{fill:rgb(175, 175, 175)!important;stroke-width:0;stroke:rgb(175, 175, 175);}#chatgpt-mermaid-\_r_j8\_ .arrowheadPath{fill:rgb(175, 175, 175);}#chatgpt-mermaid-\_r_j8\_ .edgePath .path{stroke:rgb(175, 175, 175);stroke-width:1px;}#chatgpt-mermaid-\_r_j8\_ .flowchart-link{stroke:rgb(175, 175, 175);fill:none;}#chatgpt-mermaid-\_r_j8\_ .edgeLabel{background-color:rgb(0, 0, 0);text-align:center;}#chatgpt-mermaid-\_r_j8\_ .edgeLabel p{background-color:rgb(0, 0, 0);}#chatgpt-mermaid-\_r_j8\_ .edgeLabel rect{opacity:0.5;background-color:rgb(0, 0, 0);fill:rgb(0, 0, 0);}#chatgpt-mermaid-\_r_j8\_ .labelBkg{background-color:rgba(0, 0, 0, 0.5);}#chatgpt-mermaid-\_r_j8\_ .cluster rect{fill:rgb(48, 48, 48);stroke:rgba(255, 255, 255, 0.15);stroke-width:1px;}#chatgpt-mermaid-\_r_j8\_ .cluster text{fill:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_j8\_ .cluster span{color:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_j8\_ div.mermaidTooltip{position:absolute;text-align:center;max-width:200px;padding:2px;font-family:-apple-system-body,ui-sans-serif,-apple-system,system-ui,Segoe UI,Helvetica,Apple Color Emoji,Arial,sans-serif,Segoe UI Emoji,Segoe UI Symbol;font-size:12px;background:rgb(48, 48, 48);border:1px solid rgba(255, 255, 255, 0.15);border-radius:2px;pointer-events:none;z-index:100;}#chatgpt-mermaid-\_r_j8\_ .flowchartTitleText{text-anchor:middle;font-size:18px;fill:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_j8\_ rect.text{fill:none;stroke-width:0;}#chatgpt-mermaid-\_r_j8\_ .icon-shape,#chatgpt-mermaid-\_r_j8\_ .image-shape{background-color:rgb(0, 0, 0);text-align:center;}#chatgpt-mermaid-\_r_j8\_ .icon-shape p,#chatgpt-mermaid-\_r_j8\_ .image-shape p{background-color:rgb(0, 0, 0);padding:2px;}#chatgpt-mermaid-\_r_j8\_ .icon-shape .label rect,#chatgpt-mermaid-\_r_j8\_ .image-shape .label rect{opacity:0.5;background-color:rgb(0, 0, 0);fill:rgb(0, 0, 0);}#chatgpt-mermaid-\_r_j8\_ .label-icon{display:inline-block;height:1em;overflow:visible;vertical-align:-0.125em;}#chatgpt-mermaid-\_r_j8\_ .node .label-icon path{fill:currentColor;stroke:revert;stroke-width:revert;}#chatgpt-mermaid-\_r_j8\_ .node .neo-node{stroke:rgb(31, 78, 148);}#chatgpt-mermaid-\_r_j8\_ [data-look="neo"].node rect,#chatgpt-mermaid-\_r_j8\_ [data-look="neo"].cluster rect,#chatgpt-mermaid-\_r_j8\_ [data-look="neo"].node polygon{stroke:url(#chatgpt-mermaid-\_r_j8\_-gradient);filter:drop-shadow( 1px 2px 2px rgba(185,185,185,1));}#chatgpt-mermaid-\_r_j8\_ [data-look="neo"].swimlane.cluster rect{filter:none;}#chatgpt-mermaid-\_r_j8\_ [data-look="neo"].node path{stroke:url(#chatgpt-mermaid-\_r_j8\_-gradient);stroke-width:1px;}#chatgpt-mermaid-\_r_j8\_ [data-look="neo"].node .outer-path{filter:drop-shadow( 1px 2px 2px rgba(185,185,185,1));}#chatgpt-mermaid-\_r_j8\_ [data-look="neo"].node .neo-line path{stroke:rgb(31, 78, 148);filter:none;}#chatgpt-mermaid-\_r_j8\_ [data-look="neo"].node circle{stroke:url(#chatgpt-mermaid-\_r_j8\_-gradient);filter:drop-shadow( 1px 2px 2px rgba(185,185,185,1));}#chatgpt-mermaid-\_r_j8\_ [data-look="neo"].node circle .state-start{fill:#000000;}#chatgpt-mermaid-\_r_j8\_ [data-look="neo"].icon-shape .icon{fill:url(#chatgpt-mermaid-\_r_j8\_-gradient);filter:drop-shadow( 1px 2px 2px rgba(185,185,185,1));}#chatgpt-mermaid-\_r_j8\_ [data-look="neo"].icon-shape .icon-neo path{stroke:url(#chatgpt-mermaid-\_r_j8\_-gradient);filter:drop-shadow( 1px 2px 2px rgba(185,185,185,1));}#chatgpt-mermaid-\_r_j8\_ .node text{font-size:14px;font-weight:600;letter-spacing:normal;fill:rgb(153, 206, 255);}#chatgpt-mermaid-\_r_j8\_ .edgeLabels text{font-size:13px;font-weight:600;letter-spacing:-0.08px;fill:rgb(153, 206, 255);}#chatgpt-mermaid-\_r_j8\_ .node tspan[font-weight="normal"],#chatgpt-mermaid-\_r_j8\_ .edgeLabels tspan[font-weight="normal"]{font-weight:600;}#chatgpt-mermaid-\_r_j8\_ .edgeLabel .label rect{opacity:1;rx:13px;ry:13px;fill:rgb(0, 14, 26);stroke:rgb(26, 62, 95);stroke-width:1px;}#chatgpt-mermaid-\_r_j8\_ .node rect,#chatgpt-mermaid-\_r_j8\_ .node circle,#chatgpt-mermaid-\_r_j8\_ .node ellipse,#chatgpt-mermaid-\_r_j8\_ .node polygon,#chatgpt-mermaid-\_r_j8\_ .node path{fill:rgb(0, 40, 77);stroke:rgba(255, 255, 255, 0.1);stroke-width:1px;}#chatgpt-mermaid-\_r_j8\_ .node rect{rx:16px;ry:16px;}#chatgpt-mermaid-\_r_j8\_ .mermaid-decision .label-container{fill:rgb(0, 14, 26);stroke:rgb(26, 62, 95);stroke-dasharray:2px,2px;}#chatgpt-mermaid-\_r_j8\_ .edgePaths .flowchart-link{stroke:rgb(175, 175, 175);stroke-width:1px;stroke-linecap:round;stroke-linejoin:round;}#chatgpt-mermaid-\_r_j8\_ .marker{fill:rgb(175, 175, 175);stroke:rgb(175, 175, 175);}#chatgpt-mermaid-\_r_j8\_ :root{--mermaid-font-family:-apple-system-body,ui-sans-serif,-apple-system,system-ui,Segoe UI,Helvetica,Apple Color Emoji,Arial,sans-serif,Segoe UI Emoji,Segoe UI Symbol;}systemcpualucpu_instalu_inst

This resembles mold's separation of input parsing and symbol resolution.

You can parallelise declaration extraction, then perform global resolution over collected references.

One caveat: SystemVerilog has different namespaces and visibility rules for packages, modules, types and other declarations. A single flat string-to-symbol map is insufficient.

I would use scoped symbol tables with canonical identifiers.

### **C. Elaboration: use graph processing**

This is where the hardware domain becomes particularly interesting.

Consider parameterised modules:

```
module fifo #(
    parameter WIDTH = 32,
    parameter DEPTH = 16
);
endmodule
```

And instances:

```
fifo #(.WIDTH(32), .DEPTH(16)) fifo_a();

fifo #(.WIDTH(64), .DEPTH(32)) fifo_b();
```

These are two distinct elaborated specialisations.

I would represent them using a specialization key:

```
struct SpecializationKey {
    module: ModuleId,
    parameters: CanonicalParameterSet,
}
```

The elaborator can then:

1. Resolve parameter expressions.
2. Construct specialization keys.
3. Deduplicate identical specialisations.
4. Build instance dependencies.
5. Elaborate independent specialisations concurrently.

This is conceptually similar to mold's interning and graph-oriented processing.

However, recursive instantiation, parameter dependencies and generate constructs require explicit cycle detection and language-specific diagnostics.

## **3. Where data parallelism becomes especially valuable**

I would identify four major collections that should be designed for bulk processing.

**1. Declaration arrays**

Modules, packages, interfaces, typedefs, parameters and functions.

Parallel declaration indexing and symbol registration.

**2. Reference arrays**

Identifier references, type references, instance references and package imports.

Parallel reference resolution where dependencies permit.

**3. Elaboration worklists**

Module specialisations, generate blocks and instance hierarchies.

Parallel elaboration of independent work items.

**4. Hardware operation arrays**

Combinational operations, sequential operations, ports, nets and connections.

Parallel IR construction and subsequent analyses.

The key is to avoid making every worker mutate one enormous shared data structure.

I would use worker-local buffers, stable IDs, immutable intermediate results and explicit merge barriers.

This is particularly important if you intend to use Pliron's Rust APIs: establish the thread-safety properties of its `Context`, operation allocation and pointer types before designing concurrent IR mutation around them.

## **4. Three techniques I would not copy blindly**

There are important differences between linking and hardware compilation.

**1. Atomic symbol ownership**

Mold can use atomic ownership operations because its symbol-resolution rules can be encoded around competing definitions.

SystemVerilog has more complicated scope, package visibility, compilation-unit ordering and declaration rules.

I would favour deterministic symbol-table construction and parallel resolution over indiscriminate atomic ownership.

**2. Identical Code Folding**

Mold's graph-based equivalence algorithm is interesting, but hardware equivalence is considerably more complicated.

Two combinational circuits may be functionally equivalent despite having different structures. Conversely, two structurally identical-looking sequential circuits may differ in reset, clock or state-transition semantics.

Structural hashing is useful for finding candidates, but it is not a substitute for semantic equivalence checking.

**3. Parallel IR construction**

Parallel construction is attractive, but you must establish whether your IR framework supports concurrent mutation.

A safer initial architecture is:

- Each worker constructs a module-local representation.
- Module-local results are verified.
- Results are merged into the project-level IR.
- Cross-module references are resolved at explicit synchronization boundaries.

This also gives you a useful separation between frontend correctness and IR correctness.

## **5. My proposed implementation architecture**

For your SystemVerilog-to-Pliron-CIRCT project, I would actually implement the following:

| Phase                | Execution model                                   | Output                         |
| -------------------- | ------------------------------------------------- | ------------------------------ |
| Input discovery      | Parallel I/O                                      | Compilation-unit manifest      |
| Preprocessing        | Ordered within dependent units                    | Preprocessed buffers           |
| Lexing and parsing   | Parallel                                          | Syntax trees                   |
| Declaration indexing | Parallel                                          | Declaration database           |
| Symbol resolution    | Parallel with deterministic merging               | Resolved syntax representation |
| Type analysis        | Parallel where dependencies permit                | Typed representation           |
| Elaboration          | Dependency-driven parallel worklist               | Elaborated design graph        |
| Hardware analysis    | Parallel graph algorithms                         | Validated hardware model       |
| IR construction      | Parallel module-local construction                | Pliron-CIRCT modules           |
| Verification         | Parallel module checks, followed by global checks | Verified IR                    |

I would also keep the frontend's semantic representation independent of Pliron initially. That way, the parser and elaborator do not become coupled to the implementation details of the target IR.

## **6. What I would read alongside mold**

These resources would help connect its architecture to hardware compilation:

- **mold: A Massively Parallel Linker** — [Original paper](https://arxiv.org/abs/2608.23228?utm_source=chatgpt.com). Focus on the separation of parsing and symbol resolution, concurrent algorithms and data-parallel passes.
- **IEEE 1800 SystemVerilog** — [Compilation and elaboration rules](https://iccircle.com/static/upload/img20240319175450.pdf?utm_source=chatgpt.com). Particularly important for understanding why independent file processing has limitations.
- **Surelog** and UHDM — useful existing frontend architecture to study.
- **Synlig** — [Architecture overview](https://www.mdpi.com/2079-9292/15/5/1048?utm_source=chatgpt.com). Illustrates the separation between SystemVerilog elaboration and downstream synthesis representations.

## **My final recommendation**

I would make one architectural principle central to your implementation:

**Design every compiler pass around a well-defined collection of inputs, explicit invariants and independently processable elements.**

For example:

```
ParsedDesign
    ↓
DeclarationDatabase
    ↓
ResolvedDesign
    ↓
TypedDesign
    ↓
ElaboratedDesign
    ↓
HardwareGraph
    ↓
Pliron-CIRCT
```

Each transition should have a clear contract.

This gives you three advantages:

1. You can parallelise individual passes without making the entire compiler concurrent.
2. You can test each intermediate representation independently.
3. You can eventually change the frontend implementation or target IR without redesigning the whole pipeline.

I would consider this a particularly strong foundation for a Rust-based hardware compiler. The real opportunity is not simply to build a faster SystemVerilog parser; it is to build a frontend whose entire architecture is designed around scalable analysis and transformation of hardware descriptions.
