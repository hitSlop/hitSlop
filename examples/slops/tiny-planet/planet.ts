import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { SEA, coordinates, direction, radiusAt } from "./terrain";
import type { Kind, Season } from "./schema";

export interface Palette {
  ocean: string; sand: string; grass: string; forest: string; rock: string; snow: string; autumn: string;
  blossom: string; wall: string; roof: string; trunk: string; crystal: string;
}
export interface PropData { id: string; kind: Kind; lat: number; lon: number }
export type Tool = Kind | "remove";

interface Options {
  host: HTMLElement;
  palette: Palette;
  seed: number;
  season: Season;
  reducedMotion: boolean;
  onPlace: (kind: Kind, lat: number, lon: number) => void;
  onRemove: (id: string) => void;
  onMessage: (text: string) => void;
}

const PROP_SIZE = 0.075;
const GROW_MS = 260;

/** A low-poly planet you orbit. The document holds a seed, a season and where things stand; everything here is derived. */
export class PlanetScene {
  private readonly renderer: THREE.WebGLRenderer;
  private readonly scene = new THREE.Scene();
  private readonly camera = new THREE.PerspectiveCamera(38, 1, 0.1, 50);
  private readonly controls: OrbitControls;
  private readonly world = new THREE.Group();
  private readonly ocean: THREE.Mesh;
  private terrain: THREE.Mesh;
  private readonly propGroup = new THREE.Group();
  private readonly holders = new Map<string, { holder: THREE.Group; data: PropData; born: number }>();
  private readonly geometries: THREE.BufferGeometry[] = [];
  private readonly materials = {
    leaf: new THREE.MeshStandardMaterial({ flatShading: true }),
    trunk: new THREE.MeshStandardMaterial({ flatShading: true }),
    wall: new THREE.MeshStandardMaterial({ flatShading: true }),
    roof: new THREE.MeshStandardMaterial({ flatShading: true }),
    rock: new THREE.MeshStandardMaterial({ flatShading: true }),
    crystal: new THREE.MeshStandardMaterial({ flatShading: true, emissive: new THREE.Color("#000000"), transparent: true, opacity: 0.92 }),
  };
  private readonly raycaster = new THREE.Raycaster();
  private readonly resizeObserver: ResizeObserver;
  private readonly intersection: IntersectionObserver;
  private palette: Palette;
  private seed: number;
  private season: Season;
  private tool: Tool = "tree";
  private reduced: boolean;
  private visible = true;
  private loaded = false;
  private frame = 0;
  private resumeSpin: ReturnType<typeof setTimeout> | undefined;
  private down: { x: number; y: number; at: number } | null = null;
  private destroyed = false;

  constructor(private readonly options: Options) {
    this.palette = options.palette;
    this.seed = options.seed;
    this.season = options.season;
    this.reduced = options.reducedMotion;

    this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    this.renderer.domElement.className = "planet-canvas";
    options.host.appendChild(this.renderer.domElement);

    this.scene.add(new THREE.HemisphereLight("#cfe6ff", "#2a2540", 1.1));
    const sun = new THREE.DirectionalLight("#fff3d6", 2.2);
    sun.position.set(3, 4, 5);
    this.scene.add(sun);

    this.ocean = new THREE.Mesh(
      new THREE.SphereGeometry(SEA, 48, 32),
      new THREE.MeshStandardMaterial({ color: this.palette.ocean, transparent: true, opacity: 0.9, roughness: 0.35 }),
    );
    this.terrain = this.buildTerrain();
    this.world.add(this.ocean, this.terrain, this.propGroup);
    this.scene.add(this.world);

    const [cx, cy, cz] = direction(-0.45, 0);
    this.camera.position.set(cx * 4.8, cy * 4.8, cz * 4.8);
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enablePan = false;
    this.controls.enableDamping = true;
    this.controls.minDistance = 2.4;
    this.controls.maxDistance = 7.5;
    this.controls.autoRotate = !this.reduced;
    this.controls.autoRotateSpeed = 0.7;
    this.controls.addEventListener("start", () => {
      clearTimeout(this.resumeSpin);
      this.controls.autoRotate = false;
    });
    this.controls.addEventListener("end", () => {
      clearTimeout(this.resumeSpin);
      if (!this.reduced) this.resumeSpin = setTimeout(() => (this.controls.autoRotate = true), 3500);
    });

    this.applyPalette();
    const canvas = this.renderer.domElement;
    canvas.addEventListener("pointerdown", this.onDown);
    canvas.addEventListener("pointerup", this.onUp);

    this.resizeObserver = new ResizeObserver(() => this.resize());
    this.resizeObserver.observe(options.host);
    this.intersection = new IntersectionObserver(([entry]) => {
      this.visible = !!entry?.isIntersecting;
      this.visible ? this.start() : this.stop();
    });
    this.intersection.observe(options.host);
    document.addEventListener("visibilitychange", this.onVisibility);
    this.resize();
    this.start();
  }

  setTool(tool: Tool): void { this.tool = tool; }

  setSeed(seed: number): void {
    if (seed === this.seed) return;
    this.seed = seed;
    this.world.remove(this.terrain);
    this.disposeMesh(this.terrain);
    this.terrain = this.buildTerrain();
    this.world.add(this.terrain);
    for (const entry of this.holders.values()) this.seat(entry.holder, entry.data);
  }

  setSeason(season: Season): void {
    if (season === this.season) return;
    this.season = season;
    this.applyPalette();
  }

  setPalette(palette: Palette): void {
    this.palette = palette;
    this.applyPalette();
  }

  setReducedMotion(reduced: boolean): void {
    this.reduced = reduced;
    this.controls.autoRotate = !reduced;
  }

  setProps(props: PropData[]): void {
    const ids = new Set(props.map((prop) => prop.id));
    for (const [id, entry] of this.holders) {
      if (ids.has(id)) continue;
      this.propGroup.remove(entry.holder);
      this.holders.delete(id);
    }
    for (const data of props) {
      const known = this.holders.get(data.id);
      if (known && known.data.kind === data.kind) {
        if (known.data.lat !== data.lat || known.data.lon !== data.lon) this.seat(known.holder, data);
        known.data = data;
        continue;
      }
      if (known) this.propGroup.remove(known.holder);
      const holder = new THREE.Group();
      holder.userData.id = data.id;
      holder.add(this.model(data.kind, data.id));
      this.seat(holder, data);
      const born = this.reduced || !this.loaded ? -Infinity : performance.now();
      if (born !== -Infinity) holder.scale.setScalar(0.001);
      this.propGroup.add(holder);
      this.holders.set(data.id, { holder, data, born });
    }
    this.loaded = true;
  }

  /** The planet as an image, for the export view. */
  snapshot(): string {
    this.renderer.render(this.scene, this.camera);
    return this.renderer.domElement.toDataURL("image/png");
  }

  destroy(): void {
    this.destroyed = true;
    this.stop();
    clearTimeout(this.resumeSpin);
    const canvas = this.renderer.domElement;
    canvas.removeEventListener("pointerdown", this.onDown);
    canvas.removeEventListener("pointerup", this.onUp);
    document.removeEventListener("visibilitychange", this.onVisibility);
    this.resizeObserver.disconnect();
    this.intersection.disconnect();
    this.controls.dispose();
    this.disposeMesh(this.terrain);
    this.disposeMesh(this.ocean);
    for (const geometry of this.geometries) geometry.dispose();
    for (const material of Object.values(this.materials)) material.dispose();
    this.renderer.dispose();
    canvas.remove();
  }

  private readonly onVisibility = () => {
    if (document.hidden) this.stop(); else if (this.visible) this.start();
  };

  private start(): void {
    if (this.frame || this.destroyed) return;
    const loop = (now: number) => {
      this.frame = requestAnimationFrame(loop);
      this.grow(now);
      this.controls.update();
      this.renderer.render(this.scene, this.camera);
    };
    this.frame = requestAnimationFrame(loop);
  }

  private stop(): void {
    cancelAnimationFrame(this.frame);
    this.frame = 0;
  }

  private resize(): void {
    const { clientWidth: width, clientHeight: height } = this.options.host;
    if (!width || !height) return;
    this.renderer.setSize(width, height, false);
    this.camera.aspect = width / height;
    this.camera.updateProjectionMatrix();
  }

  private grow(now: number): void {
    for (const entry of this.holders.values()) {
      if (entry.born === -Infinity) continue;
      const t = Math.min(1, (now - entry.born) / GROW_MS);
      const eased = 1 - (1 - t) ** 3;
      entry.holder.scale.setScalar(Math.max(0.001, eased * (1 + 0.18 * Math.sin(t * Math.PI))));
      if (t >= 1) { entry.holder.scale.setScalar(1); entry.born = -Infinity; }
    }
  }

  private aim(event: PointerEvent): void {
    const rect = this.renderer.domElement.getBoundingClientRect();
    this.raycaster.setFromCamera(new THREE.Vector2(((event.clientX - rect.left) / rect.width) * 2 - 1, -((event.clientY - rect.top) / rect.height) * 2 + 1), this.camera);
  }

  private readonly onDown = (event: PointerEvent) => {
    this.down = { x: event.clientX, y: event.clientY, at: performance.now() };
  };

  private readonly onUp = (event: PointerEvent) => {
    const down = this.down;
    this.down = null;
    if (!down || Math.hypot(event.clientX - down.x, event.clientY - down.y) > 5 || performance.now() - down.at > 600) return;
    this.aim(event);
    if (this.tool === "remove") {
      const hit = this.raycaster.intersectObject(this.propGroup, true)[0];
      let node: THREE.Object3D | null = hit?.object ?? null;
      while (node && node.userData.id === undefined) node = node.parent;
      if (node) this.options.onRemove(node.userData.id as string);
      else this.options.onMessage("Tap a tree, house or rock to remove it.");
      return;
    }
    const hit = this.raycaster.intersectObject(this.terrain, false)[0];
    if (!hit) return;
    const point = this.world.worldToLocal(hit.point.clone());
    const unit = point.clone().normalize();
    if (radiusAt(unit.x, unit.y, unit.z, this.seed) <= SEA + 0.004) {
      this.options.onMessage("That’s open water. Try the land.");
      return;
    }
    const { lat, lon } = coordinates(unit.x, unit.y, unit.z);
    this.options.onPlace(this.tool, lat, lon);
  };

  private buildTerrain(): THREE.Mesh {
    const geometry = new THREE.IcosahedronGeometry(1, 9);
    const position = geometry.getAttribute("position");
    const vertex = new THREE.Vector3();
    for (let i = 0; i < position.count; i++) {
      vertex.fromBufferAttribute(position, i).normalize();
      vertex.multiplyScalar(radiusAt(vertex.x, vertex.y, vertex.z, this.seed));
      position.setXYZ(i, vertex.x, vertex.y, vertex.z);
    }
    geometry.setAttribute("color", new THREE.BufferAttribute(new Float32Array(position.count * 3), 3));
    geometry.computeVertexNormals();
    const mesh = new THREE.Mesh(geometry, new THREE.MeshStandardMaterial({ vertexColors: true, flatShading: true, roughness: 0.9 }));
    this.paint(mesh);
    return mesh;
  }

  /** Colour each face by its height and the season. */
  private paint(mesh: THREE.Mesh): void {
    const geometry = mesh.geometry;
    const position = geometry.getAttribute("position");
    const color = geometry.getAttribute("color") as THREE.BufferAttribute;
    const p = this.palette;
    const ground = new THREE.Color(p.grass);
    if (this.season === "summer") ground.lerp(new THREE.Color(p.forest), 0.35);
    if (this.season === "autumn") ground.lerp(new THREE.Color(p.autumn), 0.55);
    if (this.season === "winter") ground.set(p.snow).lerp(new THREE.Color(p.rock), 0.12);
    const sand = new THREE.Color(p.sand);
    const rock = new THREE.Color(p.rock);
    const snow = new THREE.Color(p.snow);
    const face = new THREE.Color();
    const a = new THREE.Vector3(), b = new THREE.Vector3(), c = new THREE.Vector3();
    for (let i = 0; i < position.count; i += 3) {
      a.fromBufferAttribute(position, i); b.fromBufferAttribute(position, i + 1); c.fromBufferAttribute(position, i + 2);
      const height = (a.length() + b.length() + c.length()) / 3;
      if (height < SEA + 0.012) face.copy(sand);
      else if (height < SEA + 0.065) face.copy(ground);
      else if (height < SEA + 0.095) face.copy(rock);
      else face.copy(snow);
      // A little per-face variation keeps the facets readable.
      face.offsetHSL(0, 0, ((i * 2654435761) % 1000) / 1000 * 0.06 - 0.03);
      for (let k = 0; k < 3; k++) color.setXYZ(i + k, face.r, face.g, face.b);
    }
    color.needsUpdate = true;
  }

  private applyPalette(): void {
    const p = this.palette;
    (this.ocean.material as THREE.MeshStandardMaterial).color.set(p.ocean);
    this.paint(this.terrain);
    const leaf = new THREE.Color(p.forest);
    if (this.season === "spring") leaf.set(p.blossom);
    if (this.season === "autumn") leaf.set(p.autumn);
    if (this.season === "winter") leaf.set(p.forest).lerp(new THREE.Color(p.snow), 0.7);
    this.materials.leaf.color.copy(leaf);
    this.materials.trunk.color.set(p.trunk);
    this.materials.wall.color.set(p.wall);
    this.materials.roof.color.set(this.season === "winter" ? p.snow : p.roof);
    this.materials.rock.color.set(p.rock);
    this.materials.crystal.color.set(p.crystal);
    this.materials.crystal.emissive.set(p.crystal).multiplyScalar(0.35);
  }

  /** Stand a prop on the surface, upright, turned by a stable per-prop angle. */
  private seat(holder: THREE.Group, data: PropData): void {
    const [x, y, z] = direction(data.lat, data.lon);
    const radius = Math.max(radiusAt(x, y, z, this.seed), SEA);
    holder.position.set(x * radius, y * radius, z * radius);
    holder.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), new THREE.Vector3(x, y, z));
  }

  private geometry<T extends THREE.BufferGeometry>(geometry: T): T {
    this.geometries.push(geometry);
    return geometry;
  }

  private model(kind: Kind, id: string): THREE.Group {
    const group = new THREE.Group();
    const m = this.materials;
    let seedValue = 0;
    for (const char of id) seedValue = (seedValue * 31 + char.charCodeAt(0)) >>> 0;
    const turn = (seedValue % 628) / 100;
    const mesh = (geometry: THREE.BufferGeometry, material: THREE.Material, y: number) => {
      const part = new THREE.Mesh(this.geometry(geometry), material);
      part.position.y = y;
      group.add(part);
      return part;
    };
    if (kind === "tree") {
      mesh(new THREE.CylinderGeometry(0.12, 0.16, 0.5, 6), m.trunk, 0.25);
      mesh(new THREE.ConeGeometry(0.5, 0.8, 7), m.leaf, 0.85);
      mesh(new THREE.ConeGeometry(0.38, 0.65, 7), m.leaf, 1.25);
    } else if (kind === "house") {
      mesh(new THREE.BoxGeometry(0.9, 0.62, 0.7), m.wall, 0.31);
      const roof = mesh(new THREE.ConeGeometry(0.72, 0.5, 4), m.roof, 0.87);
      roof.rotation.y = Math.PI / 4;
    } else if (kind === "rock") {
      const rock = mesh(new THREE.DodecahedronGeometry(0.42, 0), m.rock, 0.22);
      rock.scale.set(1.1, 0.7, 0.9);
    } else {
      mesh(new THREE.OctahedronGeometry(0.34, 0), m.crystal, 0.5).scale.y = 1.8;
      const small = mesh(new THREE.OctahedronGeometry(0.2, 0), m.crystal, 0.22);
      small.position.x = 0.32;
      small.scale.y = 1.5;
    }
    group.scale.setScalar(PROP_SIZE);
    group.rotation.y = turn;
    return group;
  }

  private disposeMesh(mesh: THREE.Mesh): void {
    mesh.geometry.dispose();
    (mesh.material as THREE.Material).dispose();
  }
}
