const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('audio', {
  /** Ruta del mp3 de prueba que empaqueta el proyecto. */
  testPath: () => ipcRenderer.invoke('audio:test-path'),
  /** Decodifica y deja el dispositivo abierto. Resuelve con la info del stream. */
  load: (path) => ipcRenderer.invoke('audio:load', path),
  play: () => ipcRenderer.invoke('audio:play'),
  restart: () => ipcRenderer.invoke('audio:restart'),
  stop: () => ipcRenderer.invoke('audio:stop'),
  isReady: () => ipcRenderer.invoke('audio:is-ready'),
  setOffsetMs: (ms) => ipcRenderer.invoke('audio:set-offset', ms),
  position: () => ipcRenderer.invoke('audio:position'),
  stats: () => ipcRenderer.invoke('audio:stats'),
});
