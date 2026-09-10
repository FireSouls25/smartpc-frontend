<script lang="ts">
  /*
   * Ported to Svelte from "Matrix Orb" by Rare UI (rareui.com/matrixorb).
   * Free to use and modify (personal + commercial); attribution appreciated.
   * Original: React + canvas dot-matrix orb, states idle | listening | thinking.
   * This port keeps the algorithm (blended state weights, spring scale,
   * amplitude attack/release, orbiters, reduced-motion) dependency-free.
   *
   * Note: the public prop is `state`, but internally it is aliased to
   * `orbState` — otherwise `$state(...)` would parse as a store subscription.
   */
  export type MatrixOrbState = "idle" | "listening" | "thinking";

  let {
    state: orbState = "idle",
    level = undefined,
    size = 200,
    color = "#8839ef",
    dots = 11,
    labels = {},
  }: {
    state?: MatrixOrbState;
    level?: number;
    size?: number;
    color?: string;
    dots?: number;
    labels?: Partial<Record<MatrixOrbState, string>>;
  } = $props();

  const TAU = Math.PI * 2;
  const STATES: MatrixOrbState[] = ["idle", "listening", "thinking"];
  const DEFAULT_LABELS: Record<MatrixOrbState, string> = {
    idle: "Idle",
    listening: "Listening",
    thinking: "Thinking",
  };
  const SCALE: Record<MatrixOrbState, number> = {
    idle: 0.88,
    listening: 1,
    thinking: 0.92,
  };
  const STIFFNESS = 180;
  const DAMPING = 26;
  const ATTACK = 0.22;
  const RELEASE = 0.08;
  const BLEND = 0.16;
  const ORBITERS = [
    { radius: 0.62, speed: 2.2, phase: 0, spread: 0.42 },
    { radius: 0.4, speed: -1.7, phase: 2.1, spread: 0.36 },
    { radius: 0.8, speed: 1.15, phase: 4, spread: 0.34 },
  ];

  function envelope(t: number): number {
    const slow = 0.5 + 0.5 * Math.sin(t * 0.62 + 0.4);
    const fast = 0.5 + 0.5 * Math.sin(t * 1.9 + 1.1);
    return 0.22 + 0.78 * (0.45 + 0.55 * slow) * fast;
  }

  function intensityOf(
    s: MatrixOrbState,
    d: number,
    nx: number,
    ny: number,
    t: number,
    amplitude: number,
  ): number {
    if (s === "listening") {
      const ripple = 0.5 + 0.5 * Math.sin(d * 4.2 - t * 3);
      return 0.32 + amplitude * (0.34 + 0.38 * ripple);
    }
    if (s === "thinking") {
      let heat = 0;
      for (const o of ORBITERS) {
        const a = t * o.speed + o.phase;
        const dx = nx - Math.cos(a) * o.radius;
        const dy = ny - Math.sin(a) * o.radius;
        heat += Math.exp(-(dx * dx + dy * dy) / (o.spread * o.spread));
      }
      return 0.26 + 0.8 * Math.min(1, heat);
    }
    return 0.62 + 0.12 * Math.sin(t * 1.05 - d * 2.4);
  }

  let canvas: HTMLCanvasElement | null = null;
  // Latest props for the rAF loop (it retargets, never restarts on state change).
  // Initialized neutrally on purpose: the sync effect below owns the values.
  const live: { state: MatrixOrbState; level: number | undefined } = {
    state: "idle",
    level: undefined,
  };
  let redraw: (() => void) | null = null;
  let dprTick = $state(0);

  // Sync without restarting the animation.
  $effect(() => {
    live.state = orbState;
    live.level = level;
    redraw?.();
  });

  // (Re)build the loop when geometry-affecting props or DPR change.
  $effect(() => {
    dprTick;
    const el = canvas;
    const ctx = el?.getContext("2d");
    if (!el || !ctx) return;

    const s = size;
    const c = color;
    const dpr = Math.min(window.devicePixelRatio || 1, 4);
    const buffer = Math.round(s * dpr);
    el.width = buffer;
    el.height = buffer;
    ctx.setTransform(buffer / s, 0, 0, buffer / s, 0, 0);
    ctx.fillStyle = c;

    const grid = Math.max(3, Math.round(dots));
    const half = (grid - 1) / 2;
    const spacing = (s * 0.74) / (grid - 1);
    const maxRadius = spacing * 0.6;
    const center = s / 2;

    const weights: Record<MatrixOrbState, number> = {
      idle: 0,
      listening: 0,
      thinking: 0,
    };
    weights[live.state] = 1;

    const levelAt = (t: number): number => {
      const v = live.level;
      return v === undefined || !Number.isFinite(v)
        ? envelope(t)
        : Math.min(1, Math.max(0, v));
    };

    const draw = (t: number, amplitude: number, scale: number): void => {
      ctx.clearRect(0, 0, s, s);
      for (let iy = 0; iy < grid; iy++) {
        for (let ix = 0; ix < grid; ix++) {
          const nx = (ix - half) / half;
          const ny = (iy - half) / half;
          const d = Math.hypot(nx, ny);
          if (d > 1.12) continue;
          let blended = 0;
          for (const st of STATES) {
            if (weights[st] < 0.001) continue;
            blended += weights[st] * intensityOf(st, d, nx, ny, t, amplitude);
          }
          const intensity = Math.min(1, Math.max(0, blended));
          const radius = maxRadius * Math.exp(-d * d * 1.7) * intensity * scale;
          if (radius * dpr < 0.5) continue;
          ctx.beginPath();
          ctx.arc(
            center + (ix - half) * spacing * scale,
            center + (iy - half) * spacing * scale,
            radius,
            0,
            TAU,
          );
          ctx.fill();
        }
      }
    };

    const onResize = (): void => {
      dprTick += 1;
    };
    window.addEventListener("resize", onResize);

    if (
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches
    ) {
      redraw = () => {
        const current = live.state;
        for (const st of STATES) weights[st] = st === current ? 1 : 0;
        draw(0, levelAt(0), SCALE[current]);
      };
      redraw();
      return () => {
        redraw = null;
        window.removeEventListener("resize", onResize);
      };
    }

    let t = 0;
    let amplitude = 0;
    let scale = SCALE[live.state];
    let velocity = 0;
    let last = performance.now();
    let raf = 0;

    const frame = (now: number): void => {
      const dt = Math.min((now - last) / 1000, 0.05);
      last = now;
      t += dt;
      const current = live.state;
      const target = levelAt(t);
      const rate = target > amplitude ? ATTACK : RELEASE;
      amplitude += (target - amplitude) * (1 - Math.pow(1 - rate, dt * 60));
      const step = 1 - Math.pow(1 - BLEND, dt * 60);
      for (const st of STATES) {
        weights[st] += ((st === current ? 1 : 0) - weights[st]) * step;
      }
      velocity += (-STIFFNESS * (scale - SCALE[current]) - DAMPING * velocity) * dt;
      scale += velocity * dt;
      draw(t, amplitude, scale);
      raf = requestAnimationFrame(frame);
    };
    raf = requestAnimationFrame(frame);

    return () => {
      cancelAnimationFrame(raf);
      redraw = null;
      window.removeEventListener("resize", onResize);
    };
  });
</script>

<div
  data-slot="matrix-orb"
  data-state={orbState}
  class="flex flex-col items-center gap-2"
>
  <canvas
    bind:this={canvas}
    aria-hidden="true"
    class="block"
    style="width: {size}px; height: {size}px;"
  ></canvas>
  <span role="status" aria-live="polite" class="faint text-xs font-medium">
    {labels[orbState] ?? DEFAULT_LABELS[orbState]}
  </span>
</div>
