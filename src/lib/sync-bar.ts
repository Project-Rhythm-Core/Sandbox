// src/lib/sync-bar.ts
import { Container, Graphics, Text, type Application } from "pixi.js";
import type { AudioClock } from "./audio-clock";

/** Duracion de una pasada de la barra. Las marcas caen cada segundo exacto. */
const SWEEP_MS = 10_000;
const BAR_WIDTH = 20;
const TICK_MS = 1000;

export interface SyncBar {
  view: Container;
  destroy(): void;
}

/**
 * Barra de comprobacion de sincronia: recorre la pantalla en 10 s y cruza una marca
 * cada segundo. Sirve para ver a ojo si la imagen va adelantada o atrasada respecto
 * al audio, y el HUD da las cifras exactas.
 */
export function attachSyncBar(app: Application, clock: AudioClock): SyncBar {
  const view = new Container();
  app.stage.addChild(view);

  const ticks = new Graphics();
  const bar = new Graphics();
  view.addChild(ticks, bar);

  const hud = new Text({
    text: "",
    style: {
      fontFamily: "monospace",
      fontSize: 14,
      fill: 0xa0e0ff,
      lineHeight: 19,
    },
  });
  hud.x = 10;
  hud.y = 34;
  view.addChild(hud);

  const barTop = () => app.screen.height / 2 - 60;
  const barHeight = 120;

  function drawTicks() {
    const usable = app.screen.width - BAR_WIDTH;
    ticks.clear();
    for (let ms = 0; ms <= SWEEP_MS; ms += TICK_MS) {
      const x = (ms / SWEEP_MS) * usable + BAR_WIDTH / 2;
      const major = ms % 5000 === 0;
      ticks
        .moveTo(x, barTop() - 14)
        .lineTo(x, barTop() + barHeight + 14)
        .stroke({ width: major ? 2 : 1, color: major ? 0x4a5a70 : 0x2a3340 });
    }
  }

  drawTicks();
  const onResize = () => drawTicks();
  window.addEventListener("resize", onResize);

  let lastLoggedSecond = -1;

  const tick = () => {
    const posMs = clock.positionMs();
    const usable = app.screen.width - BAR_WIDTH;

    // La barra da varias pasadas: asi se puede comprobar la sincronia en cualquier
    // punto de la cancion, no solo en los primeros 10 s.
    const progress = (posMs % SWEEP_MS) / SWEEP_MS;

    bar.clear();
    if (clock.isRunning) {
      bar
        .rect(progress * usable, barTop(), BAR_WIDTH, barHeight)
        .fill(0xff3b3b);
    }

    const info = clock.info;
    const stats = clock.stats;
    hud.text = [
      `pos       ${posMs.toFixed(1).padStart(9)} ms`,
      `sync err  ${clock.syncErrorMs.toFixed(2).padStart(9)} ms   (local vs nativo)`,
      `ipc rtt   ${clock.rttMs.toFixed(2).padStart(9)} ms`,
      `salida    ${(stats?.outputLatencyMs ?? 0).toFixed(2).padStart(9)} ms`,
      `offset    ${(stats?.offsetMs ?? 0).toFixed(1).padStart(9)} ms   ([ / ] para ajustar)`,
      info
        ? `device    ${info.deviceName} @ ${info.sampleRate} Hz, ${info.bufferFrames} frames (${info.bufferMs.toFixed(2)} ms)`
        : `device    sin cargar`,
    ].join("\n");

    const currentSecond = Math.floor(posMs / 1000);
    if (currentSecond !== lastLoggedSecond) {
      lastLoggedSecond = currentSecond;
      console.log(
        `pos=${posMs.toFixed(1)}ms syncErr=${clock.syncErrorMs.toFixed(2)}ms rtt=${clock.rttMs.toFixed(2)}ms`,
      );
    }
  };

  app.ticker.add(tick);

  return {
    view,
    destroy() {
      app.ticker.remove(tick);
      window.removeEventListener("resize", onResize);
      view.destroy({ children: true });
    },
  };
}
