// Page-only camera helpers. Streams, canvases and blobs never enter the document;
// only a finished JPEG does, as an attachment.

/** Longest edge of a kept frame, in pixels. */
export const frameEdge = 1280;
const quality = 0.84;

/** Opens the camera. Call only from a person's click. */
export async function openCamera(deviceId?: string) {
  if (!navigator.mediaDevices?.getUserMedia) {
    throw Object.assign(new Error("This window can't use a camera."), { name: "NotSupportedError" });
  }
  return navigator.mediaDevices.getUserMedia({
    audio: false,
    video: deviceId
      ? { deviceId: { exact: deviceId }, width: { ideal: 1920 }, height: { ideal: 1440 } }
      : { width: { ideal: 1920 }, height: { ideal: 1440 } },
  });
}

export function stopStream(stream: MediaStream | null | undefined) {
  for (const track of stream?.getTracks() ?? []) track.stop();
}

export async function listCameras() {
  try {
    return (await navigator.mediaDevices.enumerateDevices())
      .filter(device => device.kind === "videoinput")
      .map((device, index) => ({ value: device.deviceId, label: device.label || `Camera ${index + 1}` }));
  } catch {
    return [];
  }
}

/** What went wrong, in words a person can act on. Never a black rectangle. */
export function explainCameraError(error: unknown): { title: string; detail: string } {
  const name = error instanceof Error || error instanceof DOMException ? error.name : "";
  switch (name) {
    case "NotAllowedError":
    case "SecurityError":
      return {
        title: "Camera access is off",
        detail: "Allow tinys to use the camera in System Settings › Privacy & Security › Camera, then try again. You can also add today's frame from a photo file.",
      };
    case "NotFoundError":
    case "OverconstrainedError":
      return { title: "No camera found", detail: "Connect a camera and try again, or add today's frame from a photo file." };
    case "NotReadableError":
    case "AbortError":
      return { title: "The camera is busy", detail: "Another app may be using it. Close that app and try again." };
    case "NotSupportedError":
      return { title: "No camera here", detail: "This window can't use a camera. Add today's frame from a photo file instead." };
    default:
      return { title: "The camera didn't start", detail: error instanceof Error && error.message ? error.message : "Try again in a moment." };
  }
}

/** Draws `source` scaled so its longest edge is at most `frameEdge`, as a JPEG. */
export async function toJpeg(source: CanvasImageSource, width: number, height: number) {
  if (!width || !height) throw new Error("The picture is empty.");
  const scale = Math.min(1, frameEdge / Math.max(width, height));
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(width * scale);
  canvas.height = Math.round(height * scale);
  const context = canvas.getContext("2d");
  if (!context) throw new Error("This window can't draw the picture.");
  context.imageSmoothingQuality = "high";
  context.drawImage(source, 0, 0, canvas.width, canvas.height);
  const blob = await new Promise<Blob | null>(resolve => canvas.toBlob(resolve, "image/jpeg", quality));
  if (!blob) throw new Error("The picture couldn't be saved as a JPEG.");
  return { blob, width: canvas.width, height: canvas.height };
}

/** Reads a chosen image file and re-encodes it like a camera frame (EXIF rotation applied). */
export async function fileToJpeg(file: File) {
  const url = URL.createObjectURL(file);
  try {
    const image = new Image();
    image.src = url;
    await image.decode();
    return await toJpeg(image, image.naturalWidth, image.naturalHeight);
  } finally {
    URL.revokeObjectURL(url);
  }
}
