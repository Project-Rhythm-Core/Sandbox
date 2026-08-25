import { Application, RendererType } from "pixi.js";

export async function createPixiApp(container: HTMLElement): Promise<Application> {
    const app = new Application();
    await app.init({
        resizeTo: window,
        backgroundAlpha: 0,
        preference: 'webgpu',
    });
    container.appendChild(app.canvas);

    const rendererName = app.renderer.type === RendererType.WEBGPU ? 'webgpu' : 'webgl';
    console.log('Pixi renderer activo: ', rendererName);

    return app;
}