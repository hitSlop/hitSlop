// p5 2.x and p5.brush ship without type declarations on npm; declare the small surface this sketch uses.
declare module "p5" {
  class p5 {
    static disableFriendlyErrors: boolean;
    constructor(sketch: (p: p5) => void, node?: HTMLElement);
    readonly WEBGL: "webgl";
    readonly DEGREES: "degrees";
    readonly width: number;
    readonly height: number;
    readonly mouseX: number;
    readonly mouseY: number;
    readonly canvas: HTMLCanvasElement;
    readonly drawingContext: CanvasRenderingContext2D | WebGL2RenderingContext;
    setup: () => void;
    draw: () => void;
    mousePressed: (event?: MouseEvent) => void;
    mouseMoved: (event?: MouseEvent) => void;
    createCanvas(width: number, height: number, renderer?: "webgl"): { elt: HTMLCanvasElement };
    resizeCanvas(width: number, height: number): void;
    pixelDensity(density: number): void;
    frameRate(fps: number): void;
    angleMode(mode: "degrees"): void;
    loop(): void;
    noLoop(): void;
    redraw(): Promise<void>;
    remove(): void;
    clear(): void;
    background(color: string): void;
    push(): void;
    pop(): void;
    translate(x: number, y: number): void;
    scale(factor: number): void;
    random(low: number, high: number): number;
    randomSeed(seed: number): void;
    noise(x: number, y?: number): number;
    noiseSeed(seed: number): void;
    cos(angle: number): number;
    sin(angle: number): number;
  }
  export default p5;
}
declare module "p5.brush" {
  const brush: any;
  export = brush;
}
