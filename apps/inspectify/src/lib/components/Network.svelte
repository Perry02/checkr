<script lang="ts">
  import { mirage } from 'ayu';
  import { onMount } from 'svelte';
  import type { Network } from 'vis-network/esnext';

  interface Props {
    dot: string;
  }

  let { dot }: Props = $props();

  let container: HTMLDivElement | undefined = $state();
  let network: Network | undefined = $state();

  let redraw = $derived(async () => {
    let preDot = dot;
    const vis = await import('vis-network/esnext');
    if (preDot != dot) return;
    const data = vis.parseDOTNetwork(dot);

    data.nodes.forEach((node: any) => {
      if (node.color) {
        const c = typeof node.color === 'string' ? node.color : node.color.background;
        node.color = {
          background: c,
          border: c,
          highlight: { background: c, border: c },
        };
      }
    });
    data.edges.forEach((edge: any) => {
      if (edge.color) {
        const c = typeof edge.color === 'string' ? edge.color : edge.color.color;
        edge.color = {
          color: c,
          highlight: c,
        };
      }
    });

    if (network) {
      network.setData(data);
    } else {
      if (!container) return;

      network = new vis.Network(container, data, {
        // interaction: { zoomView: false },
        nodes: {
          color: {
            background: mirage.ui.fg.hex(),
            border: mirage.ui.fg.hex(),
            highlight: {
              background: mirage.ui.fg.hex(),
              border: mirage.ui.fg.hex(),
            },
          },
          font: {
            color: 'white',
          },
          borderWidth: 1,
          shape: 'circle',
          size: 30,
        },
        edges: {
          color: {
            color: mirage.syntax.constant.hex(),
            highlight: mirage.syntax.constant.hex(),
          },
          font: {
            color: 'white',
            strokeColor: '#200020',
            face: 'Menlo, Monaco, "Courier New", monospace',
          },
        },
        autoResize: true,
      });
    }
  });

  onMount(() => {
    if (!container) return;
    const observer = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        if (network) {
          network.fit({ animation: false, maxZoomLevel: 20 });
          network.redraw();
        }
      });
    });
    observer.observe(container);
    return () => observer?.disconnect();
  });

  onMount(() => {
    redraw();
  });

  $effect(() => {
    dot && network && redraw();
  });
</script>

<div class="relative h-full w-full">
  <div class="absolute inset-0" bind:this={container}></div>
</div>
