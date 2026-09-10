<script lang="ts">
  /*
   * Mounts a React component inside Svelte (for Rare UI islands).
   * Re-renders the island whenever `props` changes; unmounts on destroy.
   */
  import { createElement, type ComponentType } from "react";
  import { createRoot, type Root } from "react-dom/client";

  let {
    component,
    props = {},
  }: {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    component: ComponentType<any>;
    props?: Record<string, unknown>;
  } = $props();

  let host: HTMLDivElement | null = null;
  let root: Root | null = null;

  $effect(() => {
    if (!host) return;
    const r = createRoot(host);
    root = r;
    return () => {
      r.unmount();
      root = null;
    };
  });

  $effect(() => {
    root?.render(createElement(component, props));
  });
</script>

<div bind:this={host} class="contents"></div>
