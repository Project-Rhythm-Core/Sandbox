const { app, BrowserWindow, ipcMain } = require('electron');
const path = require('path');
const native = require('./electron-audio-native/index.node');

function createWindow() {
  const win = new BrowserWindow({
    width: 1280,
    height: 720,
webPreferences: {
  preload: path.join(__dirname, 'preload.cjs'),
},
  });
  win.loadFile('dist/index.html');
}

ipcMain.handle('play-test-audio', (event, filePath) => {
  native.playTestAudio(filePath);
});

ipcMain.handle('get-position-ms', () => {
  return native.getPositionMs();
});

app.whenReady().then(createWindow);