// Microphone capture for the editor only. Export and Icon never import this file, so they
// cannot request a device. The stream, levels and unfinished audio stay in this object
// and are dropped with it; only a finished Blob leaves, to become an attachment.
import { AUDIO_BITS_PER_SECOND } from "./notes";

export type CaptureProblem = {
  kind: "denied" | "missing" | "busy" | "unsupported" | "failed";
  title: string;
  detail: string;
};

/** WebKit records mp4 (AAC), which the Mac can transcribe; Chromium offers WebM or Ogg. */
export function recordingType() {
  if (typeof MediaRecorder === "undefined") return undefined;
  for (const type of ["audio/mp4", "audio/mp4;codecs=mp4a.40.2", "audio/webm;codecs=opus", "audio/webm", "audio/ogg;codecs=opus"])
    if (MediaRecorder.isTypeSupported(type)) return type;
  return "";
}

export function describeProblem(error: unknown): CaptureProblem {
  const name = error instanceof DOMException || error instanceof Error ? error.name : "";
  const message = error instanceof Error ? error.message : String(error);
  if (name === "NotAllowedError" || name === "SecurityError")
    return { kind: "denied", title: "Microphone access is off", detail: "Allow microphone access for tinys in System Settings › Privacy & Security › Microphone (in a browser preview, in the site's settings), then press Record again." };
  if (name === "NotFoundError" || name === "OverconstrainedError")
    return { kind: "missing", title: "No microphone found", detail: "Connect a microphone or headset, then press Record again." };
  if (name === "NotReadableError" || name === "AbortError")
    return { kind: "busy", title: "The microphone couldn't start", detail: "Another app may be using it. Close that app, then press Record again." };
  if (name === "Unsupported")
    return { kind: "unsupported", title: "Recording isn't available here", detail: message };
  return { kind: "failed", title: "Recording couldn't start", detail: message || "Try again." };
}

function unsupported(message: string) {
  const error = new Error(message);
  error.name = "Unsupported";
  return error;
}

export class Recording {
  readonly startedAt = Date.now();
  readonly mimeType: string;
  private readonly began = performance.now();
  private readonly chunks: Blob[] = [];
  private smoothed = 0;
  private samples: Float32Array<ArrayBuffer>;
  private released = false;
  private readonly stream: MediaStream;
  private readonly recorder: MediaRecorder;
  private readonly audio: AudioContext | undefined;
  private readonly analyser: AnalyserNode | undefined;

  private constructor(stream: MediaStream, recorder: MediaRecorder, audio: AudioContext | undefined, analyser: AnalyserNode | undefined) {
    this.stream = stream;
    this.recorder = recorder;
    this.audio = audio;
    this.analyser = analyser;
    this.mimeType = recorder.mimeType;
    this.samples = new Float32Array(analyser?.fftSize ?? 0);
    recorder.ondataavailable = event => { if (event.data.size) this.chunks.push(event.data); };
  }

  /** Call from a click. Rejects with the browser's error; pass it to `describeProblem`. */
  static async start(onEnded: () => void): Promise<Recording> {
    if (!navigator.mediaDevices?.getUserMedia) throw unsupported("This window can't reach a microphone.");
    const type = recordingType();
    if (type === undefined) throw unsupported("This window can't record audio.");
    // Created inside the click so the level meter may run without another gesture.
    let audio: AudioContext | undefined;
    try { audio = new AudioContext(); } catch { audio = undefined; }
    let stream: MediaStream;
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true }, video: false });
    } catch (error) {
      void audio?.close();
      throw error;
    }
    let recorder: MediaRecorder;
    try {
      recorder = new MediaRecorder(stream, { ...(type ? { mimeType: type } : {}), audioBitsPerSecond: AUDIO_BITS_PER_SECOND });
    } catch (error) {
      for (const track of stream.getTracks()) track.stop();
      void audio?.close();
      throw error;
    }
    let analyser: AnalyserNode | undefined;
    if (audio) {
      analyser = audio.createAnalyser();
      analyser.fftSize = 1024;
      audio.createMediaStreamSource(stream).connect(analyser);
      void audio.resume().catch(() => undefined);
    }
    const recording = new Recording(stream, recorder, audio, analyser);
    for (const track of stream.getAudioTracks()) track.addEventListener("ended", onEnded, { once: true });
    recorder.start(1000);
    return recording;
  }

  get elapsed() {
    return performance.now() - this.began;
  }

  /** A smoothed input level from 0 to 1 (about -60 dBFS to 0). */
  level() {
    if (!this.analyser) return 0;
    this.analyser.getFloatTimeDomainData(this.samples);
    let sum = 0;
    for (const sample of this.samples) sum += sample * sample;
    const rms = Math.sqrt(sum / (this.samples.length || 1));
    const db = rms > 0 ? 20 * Math.log10(rms) : -100;
    const target = Math.min(1, Math.max(0, (db + 60) / 60));
    this.smoothed += (target - this.smoothed) * (target > this.smoothed ? 0.5 : 0.12);
    return this.smoothed;
  }

  /** Finishes the recording and resolves with the whole clip. */
  stop(): Promise<Blob> {
    return new Promise((resolve, reject) => {
      if (this.recorder.state === "inactive") { this.release(); resolve(new Blob(this.chunks, { type: this.mimeType })); return; }
      this.recorder.onstop = () => { this.release(); resolve(new Blob(this.chunks, { type: this.mimeType || this.chunks[0]?.type || "" })); };
      this.recorder.onerror = event => { this.release(); reject((event as Event & { error?: unknown }).error ?? new Error("The recorder failed.")); };
      this.recorder.stop();
    });
  }

  /** Drops the unfinished recording and turns the microphone off. */
  cancel() {
    this.recorder.ondataavailable = null;
    this.recorder.onstop = null;
    if (this.recorder.state !== "inactive") this.recorder.stop();
    this.chunks.length = 0;
    this.release();
  }

  private release() {
    if (this.released) return;
    this.released = true;
    for (const track of this.stream.getTracks()) track.stop();
    void this.audio?.close().catch(() => undefined);
  }
}
