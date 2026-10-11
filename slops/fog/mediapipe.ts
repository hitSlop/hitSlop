// MediaPipe Tasks (@mediapipe/tasks-vision 1.1.0), bundled: the wasm loader, the wasm and
// both models are Vite assets of this slop, so nothing is downloaded at runtime. One
// runtime serves the hand landmarker and the face detector (plans/fog.md).
import { FaceDetector, HandLandmarker, type Detection, type NormalizedLandmark } from "@mediapipe/tasks-vision";
import wasmLoaderPath from "./assets/mediapipe/vision_wasm_internal.js?url";
import wasmBinaryPath from "./assets/mediapipe/vision_wasm_internal.wasm?url";
import handModel from "./assets/mediapipe/hand_landmarker.task?url";
import faceModel from "./assets/mediapipe/blaze_face_short_range.tflite?url";
import { TUNING, type Face } from "./gestures";

export type Delegate = "GPU" | "CPU";
export type Tracker = {
  /** Where the hand landmarker runs. */
  delegate: Delegate;
  /** The first hand's 21 landmarks, normalized to the unmirrored frame, or undefined. */
  hand(video: HTMLVideoElement, at: number): NormalizedLandmark[] | undefined;
  faces(video: HTMLVideoElement, at: number): Face[];
  close(): void;
};

// Only the SIMD build ships: hitSlop needs macOS 15.2, whose WebKit has WebAssembly SIMD.
const fileset = { wasmLoaderPath, wasmBinaryPath };

// tasks-vision 1.x posts anonymous usage metrics (task, timings, OS; no images) to Google
// every minute and on close, with no option to turn it off. That is accepted; the start
// screen says so. Camera frames and audio never leave the page.

/** The hand landmarker tries the GPU (WebGL2) and falls back to the CPU. The face detector
 * runs on the CPU: its 128-pixel model is faster there than a GPU round trip, and it needs
 * no second WebGL context. Tasks are created one at a time: each loads the wasm loader
 * script, which hands over its module through one global. */
export async function createTracker(): Promise<Tracker> {
  const handsOn = (delegate: Delegate) => HandLandmarker.createFromOptions(fileset, {
    baseOptions: { modelAssetPath: handModel, delegate },
    runningMode: "VIDEO",
    numHands: 1,
    minHandDetectionConfidence: TUNING.handDetection,
    minHandPresenceConfidence: TUNING.handPresence,
    minTrackingConfidence: TUNING.handTracking,
  });
  let delegate: Delegate = "GPU";
  let hands: HandLandmarker;
  try {
    hands = await handsOn("GPU");
  } catch (error) {
    console.warn("Fog: hand tracking could not use the GPU, using the CPU", error);
    delegate = "CPU";
    hands = await handsOn("CPU");
  }
  try {
    const face = await FaceDetector.createFromOptions(fileset, {
      baseOptions: { modelAssetPath: faceModel, delegate: "CPU" },
      runningMode: "VIDEO",
      minDetectionConfidence: TUNING.faceDetection,
    });
    return trackerOf(hands, face, delegate);
  } catch (error) {
    hands.close();
    throw error;
  }
}

function trackerOf(hands: HandLandmarker, face: FaceDetector, delegate: Delegate): Tracker {
  let open = true;
  return {
    delegate,
    hand: (video, at) => hands.detectForVideo(video, at).landmarks[0],
    faces: (video, at) => face.detectForVideo(video, at).detections.map(toFace),
    close() {
      if (!open) return;
      open = false;
      hands.close();
      face.close();
    },
  };
}

const toFace = (d: Detection): Face => ({
  box: d.boundingBox && { originX: d.boundingBox.originX, originY: d.boundingBox.originY, width: d.boundingBox.width, height: d.boundingBox.height },
  keypoints: d.keypoints.map((k) => ({ x: k.x, y: k.y })),
});
