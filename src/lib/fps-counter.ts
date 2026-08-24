import { type Application, Text } from "pixi.js";

export function attachFpsCounter(app: Application): Text {
    const fpsText = new Text({
        text: 'FPS: 0',
        style: {
            fontFamily: 'monospace',
            fontSize: 16,
            fill: 0x00ff00,
        },
    });

    fpsText.x = 10;
    fpsText.y = 10;
    fpsText.zIndex = 9999;

    app.stage.addChild(fpsText);

    let elapsed = 0;
    let frameCount = 0;

    app.ticker.add((ticker) => {
        elapsed += ticker.deltaMS;
        frameCount++;

        if (elapsed >= 1000) {
            fpsText.text = `FPS: ${Math.round((frameCount * 1000) / elapsed)}`;
            elapsed = 0;
            frameCount = 0;
        }
    });

    return fpsText;
}