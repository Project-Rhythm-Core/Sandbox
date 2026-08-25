const { app, BrowserWindow, ipcMain } = require('electron');
const path = require('path');
const native = require('./electron-audio-native/index.node');

// Chromium frena rAF y los timers de las ventanas que no tienen el foco. En un juego
// de ritmo eso desincroniza la imagen del audio en cuanto alt-tabeas.
app.commandLine.appendSwitch('disable-background-timer-throttling');
app.commandLine.appendSwitch('disable-renderer-backgrounding');
app.commandLine.appendSwitch('disable-backgrounding-occluded-windows');

// El servidor de vite si estamos en desarrollo; si no, el build de dist/.
const DEV_SERVER_URL = process.env.VITE_DEV_SERVER_URL;

/** Ruta del audio de prueba, resuelta aqui para que el renderer no lleve rutas absolutas. */
const TEST_AUDIO = path.join(__dirname, 'src-tauri', 'test-audio.mp3');

function createWindow() {
  const win = new BrowserWindow({
    width: 1280,
    height: 720,
    // Evita el flash blanco: se muestra cuando el contenido esta listo para pintar.
    show: false,
    backgroundColor: '#101014',
    webPreferences: {
      preload: path.join(__dirname, 'preload.cjs'),
      contextIsolation: true,
      nodeIntegration: false,
      backgroundThrottling: false,
    },
  });

  win.once('ready-to-show', () => win.show());

  if (DEV_SERVER_URL) {
    win.loadURL(DEV_SERVER_URL);
    win.webContents.openDevTools({ mode: 'detach' });
  } else {
    win.loadFile(path.join(__dirname, 'dist', 'index.html'));
  }

  return win;
}

ipcMain.handle('audio:test-path', () => TEST_AUDIO);

// Decodifica y abre el dispositivo. Devuelve la info real del stream.
ipcMain.handle('audio:load', (_event, filePath) => native.loadAudio(filePath || TEST_AUDIO));

ipcMain.handle('audio:play', () => native.play());
ipcMain.handle('audio:restart', () => native.restart());
ipcMain.handle('audio:stop', () => native.stop());
ipcMain.handle('audio:is-ready', () => native.isReady());
ipcMain.handle('audio:set-offset', (_event, ms) => native.setOffsetMs(ms));

ipcMain.handle('audio:position', () => native.getPositionMs());
ipcMain.handle('audio:stats', () => native.getStats());

app.whenReady().then(createWindow);

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) createWindow();
});

app.on('window-all-closed', () => {
  app.quit();
});

// Cerrar el stream a mano: si no, el dispositivo puede quedarse pillado al salir.
app.on('will-quit', () => {
  native.stop();
});
