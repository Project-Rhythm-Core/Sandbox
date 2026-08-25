<script lang="ts">
  import { onMount } from "svelte";
  import { createPixiApp } from "./lib/pixi-setup";
  import type { Application } from "pixi.js";
  import { attachFpsCounter } from "./lib/fps-counter";

  let pixiContainer: HTMLDivElement;
  let app: Application;

  onMount(async() => {
    app = await createPixiApp(pixiContainer);
    attachFpsCounter(app);
  });
</script>

<div bind:this={pixiContainer} style="width: 100%; heigth: 100vh;"></div>

<style>
  :global(body) {
    margin: 0;
    overflow: hidden;
  }
</style>