/* Tipos del modulo nativo. Escritos a mano: la CLI de napi 3 no genera .d.ts
   para las macros de napi-derive 2. */

export interface AudioInfo {
  durationMs: number;
  /** Sample rate y canales con los que se abrio el dispositivo (puede diferir del
   *  archivo si hubo que remezclar o resamplear). */
  sampleRate: number;
  channels: number;
  deviceName: string;
  /** Frames por callback. 0 = lo eligio el backend. */
  bufferFrames: number;
  /** Latencia teorica de un buffer a ese sample rate, en ms. */
  bufferMs: number;
}

export interface PlaybackStats {
  positionMs: number;
  durationMs: number;
  /** Latencia de salida medida en el primer callback, en ms. */
  outputLatencyMs: number;
  /** Correccion manual aplicada a positionMs, en ms. */
  offsetMs: number;
  playing: boolean;
  /** El dispositivo esta en regimen: play() sonara de inmediato. */
  ready: boolean;
}

/** Decodifica el archivo y deja el dispositivo abierto sacando silencio.
 *  El coste de arranque se paga aqui, no en play(). */
export function loadAudio(path: string): Promise<AudioInfo>;

/** Arranca la reproduccion del audio cargado. */
export function play(): void;

/** Vuelve al principio sin reabrir el dispositivo. Instantaneo. */
export function restart(): void;

/** Posicion audible actual en ms: una lectura atomica y una resta. */
export function getPositionMs(): number;

export function getStats(): PlaybackStats;

/** True cuando el dispositivo esta en regimen. Antes de eso play() puede tardar
 *  ~200 ms en sonar porque ALSA sigue levantando el stream. */
export function isReady(): boolean;

/** Calibracion manual: positivo si el audio se oye mas tarde de lo que dice el reloj. */
export function setOffsetMs(offsetMs: number): void;

/** Cierra el stream y libera el dispositivo. */
export function stop(): void;
