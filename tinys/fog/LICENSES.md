# Third-party licenses

- **@mediapipe/tasks-vision 1.1.0**, Apache License 2.0, Copyright Google LLC / The
  MediaPipe Authors. The library code is bundled into the page; `vision_wasm_internal.js`
  and `vision_wasm_internal.wasm` from its `wasm/` folder are shipped unmodified in
  `assets/mediapipe/`.
- **MediaPipe models**, Apache License 2.0, Google LLC, downloaded once from MediaPipe's
  official model storage and shipped unmodified in `assets/mediapipe/`:
  - `hand_landmarker.task` (float16, version 1; hand detector + 21-point landmark model):
    https://storage.googleapis.com/mediapipe-models/hand_landmarker/hand_landmarker/float16/1/hand_landmarker.task
    (7,819,105 bytes, sha256 `fbc2a30080c3c557093b5ddfc334698132eb341044ccee322ccf8bcf3607cde1`)
  - `blaze_face_short_range.tflite` (BlazeFace short range, float16, version 1):
    https://storage.googleapis.com/mediapipe-models/face_detector/blaze_face_short_range/float16/1/blaze_face_short_range.tflite
    (229,746 bytes, sha256 `b4578f35940bf5a1a655214a1cce5cab13eba73c1297cd78e1a04c2380b0152f`)
- The license text is in `assets/mediapipe/LICENSE-Apache-2.0.txt`.
- **svelte** (MIT) and the **tinys** SDK.

No fonts are bundled (system `ui-rounded`). Nothing is downloaded at runtime. tasks-vision
posts anonymous usage metrics to `odml.pa.googleapis.com` (no images or audio); this is accepted.
