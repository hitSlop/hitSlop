// Browser capture helpers for the editor only. Export and Icon never import this file:
// they render saved attachments and never ask for a device.

export type Device = "camera" | "microphone";
export type Problem = { title: string; detail: string };

/** A plain explanation of a getUserMedia or MediaRecorder failure. */
export function explain(error: unknown, device: Device): Problem {
  const name = error instanceof DOMException || error instanceof Error ? error.name : "";
  const noun = device === "camera" ? "camera" : "microphone";
  const Noun = device === "camera" ? "Camera" : "Microphone";
  switch (name) {
    case "NotAllowedError":
    case "SecurityError":
      return {
        title: `${Noun} access is off`,
        detail: `hitSlop isn't allowed to use the ${noun}. Turn it on in System Settings › Privacy & Security › ${Noun}, then try again.`,
      };
    case "NotFoundError":
    case "OverconstrainedError":
      return { title: `No ${noun} found`, detail: `Connect a ${noun}, or check that it is turned on, then try again.` };
    case "NotReadableError":
    case "AbortError":
      return { title: `The ${noun} is busy`, detail: `Another app may be using the ${noun}. Close it, then try again.` };
    case "Unsupported":
      return { title: `No ${noun} here`, detail: `This window can't use the ${noun}. Open the notebook in hitSlop to capture.` };
    default:
      return {
        title: `The ${noun} didn't start`,
        detail: error instanceof Error && error.message ? error.message : "Try again in a moment.",
      };
  }
}

function unsupported(): Error {
  const error = new Error("unsupported");
  error.name = "Unsupported";
  return error;
}

/** Asks for one device. Call only from a click: the person starts every capture. */
export async function open(device: Device): Promise<MediaStream> {
  if (!navigator.mediaDevices?.getUserMedia) throw unsupported();
  return navigator.mediaDevices.getUserMedia(
    device === "camera"
      ? { video: { width: { ideal: 1600 }, height: { ideal: 1200 } }, audio: false }
      : { audio: { echoCancellation: true, noiseSuppression: true }, video: false },
  );
}

/** Stops every track, which turns the device (and its indicator light) off. */
export function stop(stream: MediaStream | undefined | null) {
  for (const track of stream?.getTracks() ?? []) track.stop();
}

/** The recording format: m4a where WebKit offers it, otherwise WebM or Ogg Opus. */
export function recordingFormat(): { mimeType: string; ext: string } | undefined {
  if (typeof MediaRecorder === "undefined") return undefined;
  const choices = [
    { mimeType: "audio/mp4", ext: "m4a" },
    { mimeType: "audio/webm;codecs=opus", ext: "webm" },
    { mimeType: "audio/webm", ext: "webm" },
    { mimeType: "audio/ogg;codecs=opus", ext: "ogg" },
  ];
  return choices.find(choice => MediaRecorder.isTypeSupported(choice.mimeType)) ?? { mimeType: "", ext: "" };
}
export function extensionFor(mimeType: string): string {
  if (mimeType.startsWith("audio/mp4") || mimeType.startsWith("video/mp4")) return "m4a";
  if (mimeType.includes("webm")) return "webm";
  if (mimeType.includes("ogg")) return "ogg";
  return "audio";
}

/** Draws the current video frame, scaled to `longest` pixels at most, as a JPEG. */
export async function still(video: HTMLVideoElement, longest: number) {
  const sourceWidth = video.videoWidth, sourceHeight = video.videoHeight;
  if (!sourceWidth || !sourceHeight) throw new Error("The camera hasn't shown a picture yet. Try again in a moment.");
  const scale = Math.min(1, longest / Math.max(sourceWidth, sourceHeight));
  const width = Math.round(sourceWidth * scale), height = Math.round(sourceHeight * scale);
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("This window couldn't draw the photo.");
  context.drawImage(video, 0, 0, width, height);
  const blob = await new Promise<Blob | null>(resolve => canvas.toBlob(resolve, "image/jpeg", 0.86));
  if (!blob) throw new Error("This window couldn't make a JPEG from the camera picture.");
  return { blob, width, height };
}
