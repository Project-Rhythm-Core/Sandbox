// src/lib/audio-clock.ts
import type { AudioInfo, PlaybackStats } from "./audio-bridge";

/** Muestras del sondeo inicial. Nos quedamos con la de menor ida y vuelta. */
const HANDSHAKE_SAMPLES = 8;
/** Cada cuanto se vuelve a comparar con el reloj nativo. */
const RESYNC_INTERVAL_MS = 500;
/** Velocidad maxima a la que se corrige el reloj local, en ms de correccion por segundo.
 *  Mantenerla baja hace que los reajustes sean invisibles en pantalla. */
const SLEW_MS_PER_SEC = 20;
/** Por encima de esto asumimos un salto real (restart, seek) y saltamos de golpe. */
const SNAP_THRESHOLD_MS = 80;
/** Margen para que el primer sample sea audible. Con el dispositivo en regimen son
 *  unos pocos ms; en frio, hasta ~250. Pasado esto es que algo va mal. */
const START_TIMEOUT_MS = 3000;

interface Sample {
  rtt: number;
  origin: number;
}

/**
 * Reloj de reproduccion local al renderer.
 *
 * El reloj bueno vive en el hilo de audio del proceso principal, pero consultarlo por
 * IPC en cada frame cuesta una ida y vuelta (y con `await` la barra se dibuja siempre
 * un frame tarde). En vez de eso estimamos una sola vez el instante de `performance.now()`
 * en el que la cancion estaba en 0 y a partir de ahi la posicion es una resta local.
 * Cada medio segundo se recompara en segundo plano y la diferencia se corrige poco a poco.
 */
export class AudioClock {
  /** Valor de performance.now() en el que la posicion de audio era 0. */
  private origin = 0;
  private targetOrigin = 0;
  private lastSlewAt = 0;
  private running = false;
  private resyncTimer: ReturnType<typeof setInterval> | null = null;

  info: AudioInfo | null = null;
  stats: PlaybackStats | null = null;
  /** Ultima diferencia observada entre el reloj local y el nativo, en ms. */
  syncErrorMs = 0;
  /** Ida y vuelta del ultimo sondeo aceptado, en ms. */
  rttMs = 0;

  /** Decodifica y deja el dispositivo caliente. Hasta que no resuelve, `play` tarda. */
  async load(path?: string): Promise<AudioInfo> {
    this.info = await window.audio.load(path);
    return this.info;
  }

  /** True cuando el dispositivo esta en regimen y arrancar sonara al instante. */
  async isReady(): Promise<boolean> {
    return window.audio.isReady();
  }

  /**
   * Espera a que el dispositivo entre en regimen. Levantar el stream tarda ~250 ms y
   * es la causa de que un `play()` en frio suene tarde; hacerlo aqui lo saca del
   * camino critico. Devuelve false si se agota el margen.
   */
  async waitUntilReady(timeoutMs = 5000): Promise<boolean> {
    const deadline = performance.now() + timeoutMs;
    while (!(await window.audio.isReady())) {
      if (performance.now() > deadline) return false;
      await new Promise((r) => setTimeout(r, 20));
    }
    return true;
  }

  async play(): Promise<void> {
    await window.audio.play();
    await this.beginTracking();
  }

  /** Reinicia sin reabrir el dispositivo. */
  async restart(): Promise<void> {
    await window.audio.restart();
    await this.beginTracking();
  }

  async stop(): Promise<void> {
    this.running = false;
    if (this.resyncTimer !== null) {
      clearInterval(this.resyncTimer);
      this.resyncTimer = null;
    }
    await window.audio.stop();
  }

  /**
   * Posicion audible actual en ms. Se llama una vez por frame y no hace IPC:
   * es una resta mas el arrastre pendiente del ultimo reajuste.
   */
  positionMs(): number {
    if (!this.running) return 0;

    const now = performance.now();
    this.applySlew(now);

    const pos = now - this.origin;
    const duration = this.info?.durationMs ?? Infinity;
    return Math.min(Math.max(pos, 0), duration);
  }

  get isRunning(): boolean {
    return this.running;
  }

  /** Espera a que el audio empiece a sonar de verdad y engancha el reloj local. */
  private async beginTracking(): Promise<void> {
    // Tras `play()` el nativo devuelve 0 hasta que el primer sample es audible.
    // Enganchar antes de eso daria un origen adelantado.
    const deadline = performance.now() + START_TIMEOUT_MS;
    while ((await window.audio.position()) <= 0) {
      if (performance.now() > deadline) {
        throw new Error("el audio no arranco: el dispositivo no esta reproduciendo");
      }
      await new Promise((r) => requestAnimationFrame(() => r(null)));
    }

    const best = await this.probe(HANDSHAKE_SAMPLES);
    this.origin = best.origin;
    this.targetOrigin = best.origin;
    this.rttMs = best.rtt;
    this.syncErrorMs = 0;
    this.lastSlewAt = performance.now();
    this.running = true;

    if (this.resyncTimer === null) {
      this.resyncTimer = setInterval(() => void this.resync(), RESYNC_INTERVAL_MS);
    }
  }

  /**
   * Sondea el reloj nativo `count` veces y devuelve la muestra con menor ida y vuelta.
   * Con la mas rapida el instante real de la lectura es el que menos incertidumbre tiene,
   * que es el mismo truco que usa NTP.
   */
  private async probe(count: number): Promise<Sample> {
    let best: Sample = { rtt: Infinity, origin: 0 };

    for (let i = 0; i < count; i++) {
      const t0 = performance.now();
      const pos = await window.audio.position();
      const t1 = performance.now();

      const rtt = t1 - t0;
      if (rtt < best.rtt) {
        // La lectura corresponde a algun punto entre t0 y t1; el punto medio es la
        // mejor apuesta y su error no pasa de rtt/2.
        best = { rtt, origin: (t0 + t1) / 2 - pos };
      }
    }

    return best;
  }

  /** Recomparacion periodica en segundo plano. No bloquea el bucle de dibujado. */
  private async resync(): Promise<void> {
    if (!this.running) return;

    const sample = await this.probe(3);
    // Una muestra mucho mas lenta que la del enganche esta contaminada por el jitter
    // del IPC: descartarla es mejor que corregir con ella.
    if (sample.rtt > Math.max(this.rttMs * 4, 8)) return;

    this.syncErrorMs = sample.origin - this.origin;

    if (Math.abs(this.syncErrorMs) > SNAP_THRESHOLD_MS) {
      this.origin = sample.origin;
      this.targetOrigin = sample.origin;
    } else {
      this.targetOrigin = sample.origin;
    }

    this.stats = await window.audio.stats();
  }

  /** Acerca `origin` a `targetOrigin` a ritmo limitado, para que no se vean saltos. */
  private applySlew(now: number): void {
    const dt = (now - this.lastSlewAt) / 1000;
    this.lastSlewAt = now;

    const diff = this.targetOrigin - this.origin;
    if (diff === 0) return;

    const step = Math.min(Math.abs(diff), SLEW_MS_PER_SEC * dt);
    this.origin += Math.sign(diff) * step;
  }
}
