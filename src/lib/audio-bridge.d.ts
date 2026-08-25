/** Puente que expone preload.cjs en el renderer. */
export interface AudioInfo {
  durationMs: number;
  sampleRate: number;
  channels: number;
  deviceName: string;
  bufferFrames: number;
  bufferMs: number;
}

export interface PlaybackStats {
  positionMs: number;
  durationMs: number;
  outputLatencyMs: number;
  offsetMs: number;
  playing: boolean;
  ready: boolean;
}

export interface AudioBridge {
  testPath(): Promise<string>;
  load(path?: string): Promise<AudioInfo>;
  play(): Promise<void>;
  restart(): Promise<void>;
  stop(): Promise<void>;
  isReady(): Promise<boolean>;
  setOffsetMs(ms: number): Promise<void>;
  position(): Promise<number>;
  stats(): Promise<PlaybackStats>;
}

declare global {
  interface Window {
    audio: AudioBridge;
  }
}
