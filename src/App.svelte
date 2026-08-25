<script lang="ts">
  import { onMount } from "svelte";
  import { createPixiApp } from "./lib/pixi-setup";
  import { attachFpsCounter } from "./lib/fps-counter";
  import { invoke } from "@tauri-apps/api/core";

  let pixiContainer: HTMLDivElement;

  onMount(async() => {
    const app = await createPixiApp(pixiContainer);
    attachFpsCounter(app);
  });

  async function startAudio() {
    await invoke('play_test_audio');
  }

  async function logPosition() {
    const pos = await invoke<number>('get_position_ms');
    console.log('Posicion actual: ', pos, ' ms');
  }
</script>

<div bind:this={pixiContainer} style="width: 100%; heigth: 100vh;"></div>
<button on:click={startAudio} style="position: fixed; top: 40px; left: 10px; z-index: 999">Play</button>
<button on:click={logPosition} style="position: fixed; top: 70px; left: 10px; z-index: 999;">Log Posicion</button>

<style>
  :global(body) {
    margin: 0;
    overflow: hidden;
  }
</style>