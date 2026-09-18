<script lang="ts">
  /*
   * Mounts a React component inside Svelte (for Rare UI islands).
   * The root is created exactly once per host element; re-renders happen
   * only when the serialized props signature changes. Function props are
   * compared by source text (not identity): inline arrows recreated every
   * Svelte render would otherwise re-render the island every time, while
   * closures over changed state with identical source are (knowingly) missed
   * — keep island callbacks stable or derive them from changing data props.
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

  // $state so the mount effect tracks it: bind:this assigns after first
  // render, and a plain `let` would leave the effect seeing null forever.
  let host = $state<HTMLDivElement | null>(null);
  let root = $state<Root | null>(null);
  let lastSignature = "";

  function signature(): string {
    return JSON.stringify(props, (_key, value: unknown) =>
      typeof value === "function" ? (value as () => void).toString() : value,
    );
  }

  $effect(() => {
    if (!host) return;
    const r = createRoot(host);
    root = r;
    lastSignature = "";
    return () => {
      r.unmount();
      root = null;
    };
  });

  $effect(() => {
    const r = root;
    // Track the component identity: a swap must re-render even when the
    // props signature is unchanged.
    void component;
    if (!r) return;
    const next = signature();
    if (next === lastSignature) return;
    lastSignature = next;
    r.render(createElement(component, props));
  });
</script>

<div bind:this={host} class="contents"></div>
