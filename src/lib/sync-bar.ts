import { Graphics, type Application } from "pixi.js";
import { invoke } from '@tauri-apps/api/core';

const TOTAL_DURATION_MS = 10000;

export function attachSyncBar(app: Application): { bar: Graphics; markStart: () => void } {
  const bar = new Graphics();
  bar.rect(0, 0, 20, 100).fill(0xff0000);
  bar.y = 200;
  app.stage.addChild(bar);

  let lastLoggedSecond = -1;
  let startTime: number | null = null;

  function markStart() {
    startTime = performance.now();
  }

  app.ticker.add(async () => {
    const posMs = await invoke<number>('get_position_ms');
    const screenWidth = app.screen.width;

    const progress = Math.min(posMs / TOTAL_DURATION_MS, 1);
    bar.x = progress * (screenWidth - 20);

    if (startTime === null) return;

    const expectedMs = performance.now() - startTime;
    const drift = posMs - expectedMs;

    const currentSecond = Math.floor(posMs / 1000);
    if (currentSecond !== lastLoggedSecond) {
      lastLoggedSecond = currentSecond;
      console.log(`posMs=${posMs.toFixed(1)}, drift=${drift.toFixed(1)}ms`);
    }
  });

  return { bar, markStart };
}