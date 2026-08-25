<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { createPixiApp } from "./lib/pixi-setup";
  import { attachFpsCounter } from "./lib/fps-counter";
  import { attachSyncBar, type SyncBar } from "./lib/sync-bar";
  import { AudioClock } from "./lib/audio-clock";

  let pixiContainer: HTMLDivElement;
  let clock = new AudioClock();
  let syncBar: SyncBar | null = null;

  let ready = $state(false);
  let playing = $state(false);
  let error = $state<string | null>(null);
  let offsetMs = $state(0);

  onMount(async () => {
    const app = await createPixiApp(pixiContainer);
    attachFpsCounter(app);
    syncBar = attachSyncBar(app, clock);

    // Cargar ya, sin esperar al boton: decodificar y levantar el dispositivo tarda
    // ~300 ms y es justo lo que hacia que el primer play sonara tarde.
    try {
      const info = await clock.load();
      console.log("audio cargado:", info);
    } catch (e) {
      error = String(e);
      return;
    }

    // El stream nace frio; hasta que ALSA no lo tiene girando, play() tarda en sonar.
    if (!(await clock.waitUntilReady())) {
      error = "el dispositivo de audio no llego a arrancar";
      return;
    }
    ready = true;
  });

  onDestroy(() => {
    syncBar?.destroy();
    void clock.stop();
  });

  async function start() {
    error = null;
    try {
      if (playing) await clock.restart();
      else await clock.play();
      playing = true;
    } catch (e) {
      error = String(e);
    }
  }

  async function nudgeOffset(delta: number) {
    offsetMs = Math.round((offsetMs + delta) * 10) / 10;
    await window.audio.setOffsetMs(offsetMs);
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === "[") void nudgeOffset(-5);
    else if (event.key === "]") void nudgeOffset(5);
    else if (event.key === " ") {
      event.preventDefault();
      void start();
    }
  }
</script>

<svelte:window on:keydown={onKey} />

<div bind:this={pixiContainer} class="stage"></div>

<div class="controls">
  <button onclick={start} disabled={!ready}>
    {#if !ready}Cargando…{:else if playing}Reiniciar{:else}Play{/if}
  </button>
  <span class="hint">espacio = play/reiniciar · [ / ] = offset {offsetMs} ms</span>
</div>

{#if error}
  <p class="error">{error}</p>
{/if}

<style>
  :global(body) {
    margin: 0;
    overflow: hidden;
    background: #101014;
  }

  .stage {
    width: 100%;
    height: 100vh;
  }

  .controls {
    position: fixed;
    bottom: 24px;
    left: 24px;
    z-index: 999;
    display: flex;
    align-items: center;
    gap: 12px;
    font-family: monospace;
    color: #7c8aa0;
  }

  button {
    font-family: monospace;
    font-size: 14px;
    padding: 8px 18px;
    border: 1px solid #3a4658;
    border-radius: 6px;
    background: #1e2430;
    color: #d8e4f0;
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .error {
    position: fixed;
    bottom: 70px;
    left: 24px;
    z-index: 999;
    font-family: monospace;
    color: #ff6b6b;
  }
</style>
