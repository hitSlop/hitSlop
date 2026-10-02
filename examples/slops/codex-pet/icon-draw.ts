const CELL_WIDTH = 192;
const CELL_HEIGHT = 208;
const RENDER_SIZE = 512;

function idleFrameBounds(image: HTMLImageElement): { x: number; y: number; width: number; height: number } {
  const scratch = document.createElement("canvas");
  scratch.width = CELL_WIDTH;
  scratch.height = CELL_HEIGHT;
  const context = scratch.getContext("2d", { willReadFrequently: true });
  if (!context) return { x: 0, y: 0, width: CELL_WIDTH, height: CELL_HEIGHT };
  context.drawImage(image, 0, 0, CELL_WIDTH, CELL_HEIGHT, 0, 0, CELL_WIDTH, CELL_HEIGHT);
  const rgba = context.getImageData(0, 0, CELL_WIDTH, CELL_HEIGHT).data;
  let minX = CELL_WIDTH, minY = CELL_HEIGHT, maxX = -1, maxY = -1;
  for (let y = 0; y < CELL_HEIGHT; y += 1) for (let x = 0; x < CELL_WIDTH; x += 1) {
    if ((rgba[(y * CELL_WIDTH + x) * 4 + 3] ?? 0) <= 8) continue;
    minX = Math.min(minX, x); minY = Math.min(minY, y);
    maxX = Math.max(maxX, x); maxY = Math.max(maxY, y);
  }
  if (maxX < minX || maxY < minY) return { x: 0, y: 0, width: CELL_WIDTH, height: CELL_HEIGHT };
  minX = Math.max(0, minX - 2); minY = Math.max(0, minY - 2);
  maxX = Math.min(CELL_WIDTH - 1, maxX + 2); maxY = Math.min(CELL_HEIGHT - 1, maxY + 2);
  return { x: minX, y: minY, width: maxX - minX + 1, height: maxY - minY + 1 };
}

export function drawPetIcon(target: HTMLCanvasElement, image: HTMLImageElement): void {
  target.width = RENDER_SIZE;
  target.height = RENDER_SIZE;
  const context = target.getContext("2d");
  if (!context) return;
  context.clearRect(0, 0, RENDER_SIZE, RENDER_SIZE);
  context.fillStyle = "#fff1d9";
  context.beginPath();
  context.roundRect(24, 24, 464, 464, 52);
  context.fill();
  context.strokeStyle = "#4b342b";
  context.lineWidth = 10;
  context.beginPath();
  context.roundRect(29, 29, 454, 454, 47);
  context.stroke();
  const bounds = idleFrameBounds(image);
  const safe = { x: 56, y: 56, size: 400 };
  const scale = Math.min(safe.size / bounds.width, safe.size / bounds.height);
  const width = Math.round(bounds.width * scale), height = Math.round(bounds.height * scale);
  const x = Math.round(safe.x + (safe.size - width) / 2), y = Math.round(safe.y + (safe.size - height) / 2);
  context.imageSmoothingEnabled = false;
  context.shadowColor = "rgba(54, 33, 25, .18)";
  context.shadowBlur = 7;
  context.shadowOffsetY = 5;
  context.drawImage(image, bounds.x, bounds.y, bounds.width, bounds.height, x, y, width, height);
}
