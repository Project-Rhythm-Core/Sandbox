# Project Rhythm Core — Sandbox

Banco de pruebas de sincronia audio/video: Svelte 5 + Pixi (WebGPU) en el renderer y un
modulo nativo de Rust (cpal + symphonia) para el audio, dentro de Electron.

El audio no pasa por la Web Audio API a proposito: en un juego de ritmo hace falta saber
con precision de milisegundos que sample esta sonando, y eso lo da el hilo de audio nativo.

## Arrancar

```bash
pnpm install
pnpm run build:native     # compila electron-audio-native (necesita cargo)
pnpm run electron         # build de vite + Electron
```

Para iterar en la interfaz, con recarga en caliente:

```bash
pnpm run dev              # en un terminal: servidor de vite
pnpm run electron:dev     # en otro
```

En la ventana: **espacio** reproduce o reinicia, **[** y **]** ajustan el offset de
calibracion en pasos de 5 ms.

## Como se mide el tiempo

Tres piezas, cada una resolviendo un retardo distinto:

1. **`electron-audio-native`** decodifica el archivo entero a PCM en un worker y abre el
   dispositivo pidiendo buffers de 256 frames (~5.8 ms). El stream arranca ya, sacando
   silencio, asi que levantar ALSA (~250 ms) se paga al cargar y no al pulsar play.
   `isReady()` dice cuando ha entrado en regimen.

2. **El reloj de reproduccion** se ancla al instante en que la muestra 0 es *audible*,
   no a cuantas muestras se han copiado al buffer. ALSA a traves de PipeWire reporta
   latencia 0, asi que el modulo la calcula por su cuenta a partir de los frames ya
   entregados. Despues sigue la deriva del reloj del hardware con una media movil.

3. **`AudioClock`** (renderer) no consulta el nativo en cada frame: estima una vez, con
   varios sondeos y quedandose con el de menor ida y vuelta, el `performance.now()` en el
   que la cancion valia 0. A partir de ahi la posicion es una resta local, sin IPC ni
   jitter. Cada 500 ms recompara en segundo plano y corrige poco a poco.

Medido en este equipo: `play()` suena a los ~5 ms, `restart()` es instantaneo, la latencia
de salida son ~3.5 ms y el reloj del renderer se desvia menos de 0.1 ms del nativo.

## Calibracion

Ninguna API sabe la latencia real de los altavoces (bluetooth, DSP del amplificador).
Para eso esta `setOffsetMs()`: positivo si el audio se oye mas tarde de lo que dice el
reloj. El HUD muestra el valor en vigor.
