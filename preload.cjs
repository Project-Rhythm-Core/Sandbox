const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('electronAPI', {
  playTestAudio: (path) => ipcRenderer.invoke('play-test-audio', path),
  getPositionMs: () => ipcRenderer.invoke('get-position-ms'),
});