<script lang="ts">
  import { browser } from '$app/environment';
  import { GCL, type Interpreter } from '$lib/api';
  import Env from '$lib/components/Env.svelte';
  import Network from '$lib/components/Network.svelte';
  import StandardInput from '$lib/components/StandardInput.svelte';
  import { Io } from '$lib/io.svelte';
  import { toSubscript } from '$lib/fmt';
  import ParsedInput from './ParsedInput.svelte';
  import InputOptions from '$lib/components/InputOptions.svelte';
  import InputOption from '$lib/components/InputOption.svelte';
  import DeterminismInput from '$lib/components/DeterminismInput.svelte';
  import { showReference } from '$lib/jobs.svelte';

  const io = new Io('Interpreter', {
    commands: 'skip',
    determinism: GCL.DETERMINISM[0],
    assignment: { variables: {}, arrays: {} },
    trace_length: 10,
  });
  let vars = $derived(io.meta ?? []);

  const highlightDot = (
    dot: string,
    initialNode: string,
    trace: Interpreter.Step[],
    termination: Interpreter.TerminationState,
  ) => {
    const color = termination === 'Stuck' ? '#ef4444' : '#34d399';
    let highlightedDot = dot;
    const pathNodes = new Set([initialNode]);
    const pathEdges: { from: string; to: string; label: string }[] = [];

    let currentNode = initialNode;
    for (const step of trace) {
      pathNodes.add(step.node);
      pathEdges.push({ from: currentNode, to: step.node, label: step.action });
      currentNode = step.node;
    }

    const nodeMap: Record<string, string> = {
      'q▷': 'qStart',
      'q◀': 'qFinal',
    };
    const getId = (node: string) => nodeMap[node] || node;
    const escapeRegex = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

    // Highlight nodes
    for (const node of pathNodes) {
      const id = getId(node);
      const escapedId = escapeRegex(id);
      const nodeRegex = new RegExp(`(?<!->\\s*)("${escapedId}"|\\b${escapedId}\\b)\\s*\\[`, 'g');
      if (highlightedDot.match(nodeRegex)) {
        highlightedDot = highlightedDot.replace(nodeRegex, `$1 [color="${color}", penwidth=3, `);
      } else {
        highlightedDot = highlightedDot.replace(
          new RegExp(`(?<!->\\s*)("${escapedId}"|\\b${escapedId}\\b)\\s*;`, 'g'),
          `$1 [color="${color}", penwidth=3];`,
        );
      }
    }

    // Highlight edges
    for (const edge of pathEdges) {
      const fromId = escapeRegex(getId(edge.from));
      const toId = escapeRegex(getId(edge.to));
      const escapedLabel = escapeRegex(edge.label);
      const edgeRegex = new RegExp(
        `("${fromId}"|\\b${fromId}\\b)\\s*->\\s*("${toId}"|\\b${toId}\\b)\\s*\\[(?=[^\\]]*label\\s*=\\s*\\"${escapedLabel}\\")`,
        'g',
      );
      highlightedDot = highlightedDot.replace(edgeRegex, `$1 -> $2 [color="${color}", penwidth=3, `);
    }

    return highlightedDot;
  };

  $effect.pre(() => {
    if (browser) {
      for (const v of vars) {
        if (v.kind == 'Variable') {
          if (typeof io.input.assignment.variables[v.name] != 'number') {
            io.input.assignment.variables[v.name] = 0;
          }
        } else if (v.kind == 'Array') {
          if (!Array.isArray(io.input.assignment.arrays[v.name])) {
            io.input.assignment.arrays[v.name] = [0];
          }
        }
      }
    }
  });

  let highlightedTraceIndices = $state(new Set<number>());

  $effect(() => {
    io.results.output;
    highlightedTraceIndices = new Set();
  });

  const onGraphClick = (params: {
    nodes: string[];
    edges: { from: string; to: string; label: string }[];
  }) => {
    const indices = new Set<number>();

    const reverseNodeMap: Record<string, string> = {
      qStart: 'q▷',
      qFinal: 'q◀',
    };
    const fromId = (id: string) => reverseNodeMap[id] || id;

    const output = io.results.output;
    if (!output) return;

    const fullTrace = [
      { action: '', node: output.initial_node, memory: io.results.input.assignment },
      ...output.trace,
    ];

    if (params.nodes.length > 0) {
      const node = fromId(params.nodes[0]);
      fullTrace.forEach((step, i) => {
        if (step.node === node) indices.add(i);
      });
    } else if (params.edges.length > 0) {
      const edge = params.edges[0];
      const from = fromId(edge.from);
      const to = fromId(edge.to);
      const label = edge.label;

      fullTrace.forEach((step, i) => {
        if (i === 0) return;
        const prevStep = fullTrace[i - 1];
        if (prevStep.node === from && step.node === to && step.action === label) {
          indices.add(i);
        }
      });
    }

    highlightedTraceIndices = indices;
  };
</script>

<Env {io}>
  {#snippet inputView()}
    <StandardInput analysis="Interpreter" code="commands" {io}>
      <InputOptions title="Initialization of variables and arrays">
        <div class="col-span-full grid grid-cols-[max-content_1fr] items-center gap-y-2 px-1 py-1">
          {#each vars.slice().sort((a, b) => (a.name > b.name ? 1 : -1)) as v}
            <div class="px-4 py-0.5 font-mono text-sm">
              {v.name}
            </div>
            <div class="w-full font-mono">
              {#if v.kind == 'Array'}
                <ParsedInput type="array" bind:value={io.input.assignment.arrays[v.name]} />
              {:else}
                <ParsedInput type="int" bind:value={io.input.assignment.variables[v.name]} />
              {/if}
            </div>
          {/each}
        </div>
      </InputOptions>
      <InputOptions>
        <InputOption title="Number of steps">
          <div class="w-full font-mono">
            <ParsedInput type="int" bind:value={io.input.trace_length} />
          </div>
        </InputOption>
        <DeterminismInput input={io.input} />
      </InputOptions>
    </StandardInput>
  {/snippet}
  {#snippet outputView({ input: cachedInput, output, meta })}
    <div class="grid min-h-0 grid-cols-[auto_1fr]">
      <div class="flex min-h-0 flex-col border-r border-t bg-slate-900">
        <div class="flex items-center justify-between border-b border-slate-700 px-4 py-2">
          <div class="font-mono text-sm font-semibold uppercase tracking-wider text-slate-200">
            Trace
          </div>
          {#if cachedInput.determinism == 'NonDeterministic'}
            <button
              type="button"
              class="rounded border border-slate-500 px-3 py-1 text-xs font-semibold uppercase tracking-wide text-slate-100 transition hover:bg-slate-800 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-200"
              onclick={() => io.rerun()}
            >
              Change path
            </button>
          {/if}
        </div>
        <div class="overflow-auto">
          <div
            class="grid gap-x-4 px-4 py-2"
            style="grid-template-columns: max-content min-content repeat({Math.max(
              meta.length,
              1,
            )}, max-content);"
          >
          <div></div>
          <div></div>
          <div
            class="border-b text-left font-mono font-bold"
            style="grid-column: span {meta.length}"
          >
            Memory
          </div>
          {#each ['Action', 'Node'] as name}
            <div class="text-left font-mono font-bold">
              {name}
            </div>
          {/each}

          {#if meta.length == 0}
            <div></div>
          {/if}
          {#each meta as v}
            <div class="text-center font-mono font-bold">
              {v.name}
            </div>
          {/each}

          {#each [{ action: '', node: output.initial_node, memory: cachedInput.assignment }, ...output.trace] as step, i}
            <div
              class="line-clamp-1 max-w-[25ch] text-sm {highlightedTraceIndices.has(i)
                ? 'bg-white/10'
                : ''}"
            >
              <code>{step.action}</code>
            </div>
            <div class="text-center {highlightedTraceIndices.has(i) ? 'bg-white/10' : ''}">
              {toSubscript(step.node)}
            </div>
            {#if meta.length == 0}
              <div class={highlightedTraceIndices.has(i) ? 'bg-white/10' : ''}></div>
            {/if}
            {#each meta as v}
              <div
                class="px-1 text-right font-mono text-slate-300 {highlightedTraceIndices.has(i)
                  ? 'bg-white/10'
                  : ''}"
              >
                {v.kind == 'Array'
                  ? JSON.stringify(step.memory.arrays[v.name])
                  : step.memory.variables[v.name]}
              </div>
            {/each}
          {/each}
          <div class="flex">
            {#if output.termination == 'Running'}
              <div class="my-1 rounded-sm bg-blue-500 px-2 py-1 font-bold text-white">
                Stopped after {output.trace.length} steps
              </div>
            {:else if output.termination == 'Terminated'}
              <div class="my-1 rounded-sm bg-green-500 px-2 py-1 font-bold text-white">
                Terminated
              </div>
            {:else if output.termination == 'Stuck'}
              <div class="my-1 rounded-sm bg-red-500 px-2 py-1 font-bold text-white">Stuck</div>
            {/if}
          </div>
        </div>
        </div>
      </div>

      <div class="relative">
        <div class="absolute inset-0 grid overflow-auto">
          <Network
            dot={showReference.show
              ? output.dot
              : highlightDot(output.dot, output.initial_node, output.trace, output.termination)}
            onclick={onGraphClick}
          />
        </div>
      </div>
    </div>
  {/snippet}
</Env>
