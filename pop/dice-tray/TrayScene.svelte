<script lang="ts">
  import { T, useTask, useThrelte } from "@threlte/core";
  import * as THREE from "three";
  import { onDestroy, untrack } from "svelte";
  import {
    layout, restPose, seeded, tumble, tumblePose, visibleFloor,
    type DiceKit, type Palette, type Placement, type ShownRoll, type Tumble,
  } from "./dice3d";

  let { roll, kit, palette, reduced, onRolling, onSettled }: {
    roll: ShownRoll | undefined;
    kit: DiceKit;
    palette: Palette;
    reduced: boolean;
    onRolling?: (id: string) => void;
    onSettled?: (id: string) => void;
  } = $props();

  const { renderer, size, invalidate } = useThrelte();

  // The felt: a soft seeded noise multiplied by the theme colour.
  const feltCanvas = document.createElement("canvas");
  feltCanvas.width = feltCanvas.height = 128;
  {
    const g = feltCanvas.getContext("2d")!;
    const pixels = g.createImageData(128, 128);
    const random = seeded(1729);
    for (let i = 0; i < pixels.data.length; i += 4) {
      const v = 214 + Math.floor(random() * 41);
      pixels.data[i] = pixels.data[i + 1] = pixels.data[i + 2] = v;
      pixels.data[i + 3] = 255;
    }
    g.putImageData(pixels, 0, 0);
  }
  const feltTexture = new THREE.CanvasTexture(feltCanvas);
  feltTexture.wrapS = feltTexture.wrapT = THREE.RepeatWrapping;
  feltTexture.repeat.set(14, 14);
  feltTexture.colorSpace = THREE.SRGBColorSpace;

  const dice = new THREE.Group();
  let camera = $state.raw<THREE.PerspectiveCamera>();

  interface Live {
    mesh: THREE.Mesh;
    rest: Placement;
    resting: { quaternion: THREE.Quaternion; height: number };
    reach: number;
    flight?: Tumble;
  }
  let live: Live[] = [];
  let shownId: string | undefined;
  const seen = new Set<string>();
  let animating = $state(false);
  let startedAt = 0;

  function place(target: ShownRoll | undefined, animate: boolean) {
    dice.clear();
    live = [];
    if (!camera || !target) { invalidate(); return; }
    const { width, height } = $size;
    if (!width || !height) return;
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
    const rect = visibleFloor(camera);
    const places = layout(target.dice.length, target.seed, rect);
    live = target.dice.map((die, index) => {
      const { shape, material } = kit.get(die.kind);
      const mesh = new THREE.Mesh(shape.geometry, material);
      mesh.castShadow = true;
      const rest = places[index]!;
      mesh.scale.setScalar(rest.scale);
      const resting = restPose(shape, die.value, seeded((target.seed + index * 31) >>> 0)() * 2 - 1);
      const entry: Live = { mesh, rest, resting, reach: shape.reach, flight: animate ? tumble(target.seed, index, rest, rect) : undefined };
      if (entry.flight) tumblePose(entry.flight, rest, resting, shape.reach, 0, mesh.position, mesh.quaternion);
      else {
        mesh.position.set(rest.x, resting.height * rest.scale, rest.z);
        mesh.quaternion.copy(resting.quaternion);
      }
      dice.add(mesh);
      return entry;
    });
    invalidate();
  }

  // A roll the tray has not shown yet tumbles in; the one present on open, or one revisited
  // by undo, simply rests. Resizing re-seats the dice at rest (landing a tumble early).
  let first = true;
  $effect(() => {
    const id = roll?.id;
    const { width, height } = $size;
    if (!camera || !width || !height) return;
    untrack(() => show(id));
  });

  function show(id: string | undefined) {
    const target = roll;
    const fresh = id !== shownId;
    const animate = fresh && !!target && !first && !reduced && !seen.has(target.id);
    first = false;
    shownId = id;
    if (target) seen.add(target.id);
    const interrupted = animating;
    animating = false;
    place(target, animate);
    if (animate && target) {
      startedAt = performance.now();
      animating = true;
      onRolling?.(target.id);
    } else if (target && (fresh || interrupted)) {
      onSettled?.(target.id);
    }
  }

  $effect(() => {
    if (reduced && untrack(() => animating)) untrack(() => show(roll?.id));
  });

  $effect(() => {
    void palette;
    invalidate();
  });

  useTask(() => {
    const elapsed = performance.now() - startedAt;
    let done = true;
    for (const entry of live) {
      if (!entry.flight) continue;
      if (!tumblePose(entry.flight, entry.rest, entry.resting, entry.reach, elapsed, entry.mesh.position, entry.mesh.quaternion)) done = false;
    }
    if (done) {
      animating = false;
      if (roll) onSettled?.(roll.id);
    }
  }, { running: () => animating });

  onDestroy(() => {
    dice.clear();
    feltTexture.dispose();
    // Give the GL context back now rather than waiting for garbage collection.
    renderer.forceContextLoss();
  });
</script>

<T.PerspectiveCamera
  makeDefault
  fov={32}
  near={0.5}
  far={60}
  oncreate={(ref) => {
    // Nearly overhead, so the face that came up is the one you read.
    ref.position.set(0, 15.5, 3.8);
    ref.lookAt(0, 0, 0.3);
    camera = ref;
  }}
/>
<T.HemisphereLight args={["#fff4e2", palette.feltDeep, 1.35]} />
<T.DirectionalLight
  position={[-4.5, 10, 5.5]}
  intensity={2.1}
  castShadow
  shadow.mapSize={[1024, 1024]}
  shadow.camera.left={-10}
  shadow.camera.right={10}
  shadow.camera.top={10}
  shadow.camera.bottom={-10}
  shadow.camera.far={30}
  shadow.bias={-0.0008}
  shadow.radius={5}
/>
<T.Mesh rotation.x={-Math.PI / 2} receiveShadow>
  <T.PlaneGeometry args={[60, 60]} />
  <T.MeshStandardMaterial color={palette.felt} map={feltTexture} roughness={1} metalness={0} />
</T.Mesh>
<T is={dice} dispose={false} />
