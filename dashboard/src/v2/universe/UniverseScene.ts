/**
 * The universe canvas: three.js, mounted imperatively.
 *
 * Deliberately not `@react-three/fiber`. The scene is a handful of instanced-free meshes, one line-segment
 * set per layer and a camera; there is no React state in it, and R3F would put a reconciler on top of a
 * dependency that is already the single largest thing in this bundle. `docs/design/dashboard-v2.md` records
 * the measurement behind that choice.
 *
 * What it draws, and why each is drawn the way it is:
 *   * **volumes** — a derived namespace group, translucent shell plus wireframe, area from its members;
 *   * **packages** — one solid each: height = log(members), footprint = sqrt(functions) clamped to local
 *     spacing, coloured by whether the package shares its community;
 *   * **buckets** (`(other)`, `root`, absolute paths) — in a band of their own, wireframe, never inside a
 *     volume: they belong to no namespace because they are not namespaces;
 *   * **the web** — every edge between real packages, brightness = log(weight) on an additive blend. Edges
 *     that touch a bucket are excluded and the room says how many and why.
 */
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { footprintScale, heightScale, hueFor, layoutVolumes, sunflower } from "../architectureLayers";
import type { ArchitectureView, CommunityGroup, PackageNode } from "../architectureTypes";

export type UniverseLayer = "packages" | "communities";

/** One drawable line: both ends and the weight its brightness is taken from. */
type Segment = [[number, number, number], [number, number, number], number];

export interface UniverseInput {
  view: ArchitectureView;
  layer: UniverseLayer;
  edges: boolean;
  height: (size: number) => number;
  footprint: (functions: number) => number;
}

export interface HoverEntry {
  kind: "package" | "volume" | "bucket" | "community";
  title: string;
  lines: string[];
}

const SINGLE = 0x67788c; // a package alone in its community — a finding, not a failure

export class UniverseScene {
  private readonly scene = new THREE.Scene();
  private readonly camera: THREE.PerspectiveCamera;
  private readonly renderer: THREE.WebGLRenderer;
  private readonly controls: OrbitControls;
  private readonly world = new THREE.Group();
  private readonly raycaster = new THREE.Raycaster();
  private readonly pointer = new THREE.Vector2();
  private readonly pickables: { mesh: THREE.Mesh; entry: HoverEntry }[] = [];
  private readonly disposables: { dispose(): void }[] = [];
  private frame = 0;
  private mounted = true;
  private reducedMotion = false;
  private onHover: (entry: HoverEntry | null) => void = () => {};
  private onPick: (entry: HoverEntry | null) => void = () => {};

  constructor(container: HTMLElement) {
    this.reducedMotion = globalThis.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    this.scene.background = new THREE.Color(0x060a12);
    this.scene.fog = new THREE.Fog(0x060a12, 2200, 7200);

    this.camera = new THREE.PerspectiveCamera(44, 1, 1, 90000);
    this.camera.position.set(760, 620, 900);

    this.renderer = new THREE.WebGLRenderer({ antialias: true });
    this.renderer.setPixelRatio(Math.min(globalThis.devicePixelRatio ?? 1, 2));
    container.appendChild(this.renderer.domElement);

    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    Object.assign(this.controls, {
      enableDamping: true, dampingFactor: 0.08, minDistance: 120, maxDistance: 12000,
      autoRotate: true, autoRotateSpeed: 0.28, maxPolarAngle: Math.PI * 0.495,
    });

    this.scene.add(new THREE.HemisphereLight(0x9dbfe6, 0x121a26, 0.95));
    const key = new THREE.DirectionalLight(0xffffff, 1.1);
    key.position.set(600, 900, 500);
    this.scene.add(key);
    const fill = new THREE.DirectionalLight(0x8fb4ff, 0.55);
    fill.position.set(-700, 400, -500);
    this.scene.add(fill);
    this.scene.add(this.world);

    this.resize(container);
    this.loop();
  }

  onHoverChange(fn: (entry: HoverEntry | null) => void): void {
    this.onHover = fn;
  }

  onPickChange(fn: (entry: HoverEntry | null) => void): void {
    this.onPick = fn;
  }

  resize(container: HTMLElement): void {
    const width = Math.max(1, container.clientWidth);
    const height = Math.max(1, container.clientHeight);
    this.camera.aspect = width / height;
    this.camera.updateProjectionMatrix();
    // `updateStyle` must stay on (the default). With `false`, three sets the *backing store* to width x dpr
    // and never writes `style.width/height`, so the element's CSS size falls back to its attribute size: on a
    // 2x display the canvas renders at twice the size of its container, is clipped by the panel's
    // `overflow: hidden`, and the model ends up parked in the bottom-right of the viewport.
    this.renderer.setSize(width, height);
  }

  /** Rebuild the scene for a new view. Cheap enough (256 solids, 6,524 segments) to do on every toggle. */
  update(input: UniverseInput): void {
    this.clear();
    this.pickables.length = 0;
    if (input.layer === "packages") this.drawPackages(input);
    else this.drawCommunities(input);
    this.frameWorld();
  }

  private drawPackages(input: UniverseInput): void {
    const { view, height, footprint } = input;
    const seat = (n: number) => (20 / 2.4) * Math.sqrt(n);
    const layout = layoutVolumes(view.volumes, view.standalone.length, { seatRadius: seat, pad: 16 });
    const positions = new Map<number, [number, number, number]>();

    for (const volume of view.volumes) {
      const placed = layout.volumes.find((v) => v.key === volume.key);
      if (!placed) continue;
      const top = Math.max(...volume.packages.map((p) => height(p.size))) + 26;
      const box = new THREE.BoxGeometry(placed.radius * 2 + 40, top, placed.radius * 2 + 40);
      const shell = new THREE.Mesh(box, new THREE.MeshBasicMaterial({
        color: 0x8fd0ff, transparent: true, opacity: 0.05, depthWrite: false,
      }));
      shell.position.set(placed.centre[0], top / 2 - 4, placed.centre[1]);
      this.add(shell);
      this.coreObjects.push(shell);
      const wire = new THREE.LineSegments(
        new THREE.EdgesGeometry(box),
        new THREE.LineBasicMaterial({ color: 0xa8dcff, transparent: true, opacity: 0.5 }),
      );
      wire.position.copy(shell.position);
      this.add(wire);
      this.pickables.push({
        mesh: shell,
        entry: {
          kind: "volume",
          title: volume.key,
          lines: [
            `namespace group · ${volume.packages.length} packages`,
            `${volume.members.toLocaleString()} members · ${volume.functions.toLocaleString()} functions`,
            "derived from the package labels — not an exported object",
          ],
        },
      });

      volume.packages.forEach((pkg, index) => {
        positions.set(pkg.id, sunflower(placed.centre, seat(volume.packages.length), index, volume.packages.length));
      });
    }

    view.standalone.forEach((pkg, index, all) => {
      const angle = (index / Math.max(1, all.length)) * Math.PI * 2;
      positions.set(pkg.id, [Math.cos(angle) * layout.artRadius, 0, Math.sin(angle) * layout.artRadius]);
    });
    const buckets = [...(view.remainder ? [view.remainder] : []), ...view.artifacts];
    buckets.forEach((pkg, index) => {
      const angle = Math.PI * 0.5 + (index - buckets.length / 2) * 0.06;
      positions.set(pkg.id, [
        Math.cos(angle) * layout.artifactRadius, 0, Math.sin(angle) * layout.artifactRadius,
      ]);
    });

    const drawn = [...view.packages, ...view.standalone, ...buckets];
    for (const pkg of drawn) {
      const pos = positions.get(pkg.id);
      if (!pos) continue;
      this.solid(pkg, pos, height(pkg.size), footprint(pkg.functions));
    }

    if (input.edges) this.web(drawn, positions, view);
  }

  private drawCommunities(input: UniverseInput): void {
    const { view } = input;
    const groups = view.communities;
    // The aggregated layer has its own distribution — a community holds the *sum* of its packages' members,
    // so reusing the package height scale would push every solid past the top of it.
    const height = heightScale(groups.map((g) => g.members));
    const footprint = footprintScale(groups.map((g) => g.functions));
    const positions = new Map<number, [number, number, number]>();
    const extent = 70 * Math.sqrt(Math.max(1, groups.length));

    groups.forEach((group, index) => {
      const radius = 60 * Math.sqrt(index + 0.6);
      const angle = index * 2.39996;
      const pos: [number, number, number] = [Math.cos(angle) * radius, 0, Math.sin(angle) * radius];
      positions.set(group.id, pos);
      const side = footprint(group.functions);
      const geo = new THREE.BoxGeometry(side, height(group.members), side);
      const mesh = new THREE.Mesh(geo, new THREE.MeshStandardMaterial({
        color: new THREE.Color().setHSL(hueFor(index) / 360, group.shared ? 0.72 : 0.08, group.shared ? 0.58 : 0.42),
        roughness: 0.55, metalness: 0.12,
      }));
      mesh.position.set(pos[0], height(group.members) / 2, pos[2]);
      this.add(mesh);
      this.coreObjects.push(mesh);
      this.pickables.push({ mesh, entry: this.communityEntry(group) });
    });

    if (input.edges) {
      // Same anchoring rule as the package layer: leave each solid part-way up, or the web hides at the
      // base of the very solids it connects.
      const lift = new Map(groups.map((g) => [g.id, Math.min(height(g.members) * 0.5, 46)]));
      const pairs = this.communityPairs(view);
      this.webSegments(pairs.flatMap(([a, b, weight]) => {
        const from = positions.get(a);
        const to = positions.get(b);
        if (!from || !to) return [];
        const segment: Segment = [
          [from[0], lift.get(a) ?? 6, from[2]],
          [to[0], lift.get(b) ?? 6, to[2]],
          weight,
        ];
        return [segment];
      }));
    }
    void extent;
  }

  /**
   * The aggregate layer's edges: the exported package edges, collapsed to community pairs.
   *
   * Read from the exported edge list rather than from `incidence`, which only the package layer fills — so
   * this layer no longer depends on the other one having been drawn first (opening the room on the
   * aggregated view drew no edges at all).
   */
  private communityPairs(view: ArchitectureView): [number, number, number][] {
    const byId = new Map(view.packages.map((p) => [p.id, p]));
    const pairs = new Map<string, number>();
    for (const edge of view.edgeList) {
      const a = byId.get(edge.source);
      const b = byId.get(edge.target);
      if (!a || !b || a.communityId === b.communityId) continue;
      const key = a.communityId < b.communityId
        ? `${a.communityId}|${b.communityId}` : `${b.communityId}|${a.communityId}`;
      pairs.set(key, (pairs.get(key) ?? 0) + edge.weight);
    }
    return [...pairs.entries()].map(([key, weight]) => {
      const [from, to] = key.split("|").map(Number);
      return [from, to, weight] as [number, number, number];
    });
  }

  /** Filled by `web()` so the aggregate layer can reuse the same edges. */
  private readonly incidence = new Map<number, { id: number; weight: number }[]>();

  /** Solid height per node, so an edge can leave a solid part-way up instead of at the floor. */
  private readonly heights = new Map<number, number>();

  /**
   * What the camera frames: the mapped structure, not the artifact band.
   *
   * The band holds the producer's buckets and the path fallbacks — on a repo whose labels all resolve to
   * absolute paths it is the *majority* of the geometry, and framing it would shrink the architecture to a
   * thumbnail in the middle of its own viewport.
   */
  private readonly coreObjects: THREE.Object3D[] = [];

  private solid(
    pkg: PackageNode, pos: [number, number, number], height: number, side: number,
  ): void {
    const isBucket = pkg.kind !== "package";
    const colour = isBucket
      ? new THREE.Color(0x6b5f7a)
      : pkg.sharesCommunity
        ? new THREE.Color().setHSL(hueFor(pkg.communityId % 97) / 360, 0.72, 0.58)
        : new THREE.Color(SINGLE);
    const geo = new THREE.BoxGeometry(side, height, side);
    const mesh = new THREE.Mesh(geo, new THREE.MeshStandardMaterial({
      color: colour, roughness: 0.55, metalness: 0.12,
    }));
    mesh.position.set(pos[0], height / 2, pos[2]);
    this.add(mesh);
    this.heights.set(pkg.id, height);
    if (pkg.kind === "package") this.coreObjects.push(mesh);
    if (isBucket) {
      const wire = new THREE.LineSegments(
        new THREE.EdgesGeometry(geo),
        new THREE.LineBasicMaterial({ color: 0xffcf7a, transparent: true, opacity: 0.85 }),
      );
      wire.position.copy(mesh.position);
      this.add(wire);
    }
    this.pickables.push({
      mesh,
      entry: {
        kind: isBucket ? "bucket" : "package",
        title: pkg.label,
        lines: [
          isBucket
            ? "not a package — a producer bucket"
            : `package · ${pkg.size.toLocaleString()} members · ${pkg.functions.toLocaleString()} functions`,
          isBucket
            ? `${pkg.size.toLocaleString()} members · ${pkg.functions.toLocaleString()} functions`
            : `namespace ${pkg.namespace} · community ${pkg.communityId}`,
          isBucket
            ? "its coupling is aggregated over unrelated files, so it is not drawn"
            : pkg.sharesCommunity ? "shares its community" : "alone in its community",
        ],
      },
    });
  }

  private communityEntry(group: CommunityGroup): HoverEntry {
    return {
      kind: "community",
      title: group.label,
      lines: [
        `community · ${group.members.toLocaleString()} members over ${group.packageCount} node${group.packageCount === 1 ? "" : "s"}`,
        group.shared ? "holds more than one node" : "holds exactly one node",
        group.containsBucket ? "includes a producer bucket, not only packages" : "packages only",
      ],
    };
  }

  private web(
    nodes: PackageNode[], positions: Map<number, [number, number, number]>, view: ArchitectureView,
  ): void {
    void view;
    this.incidence.clear();
    const segments: Segment[] = [];
    const edges = this.edgeList;
    for (const edge of edges) {
      const from = positions.get(edge.source);
      const to = positions.get(edge.target);
      if (!from || !to) continue;
      const a = this.byId.get(edge.source);
      const b = this.byId.get(edge.target);
      if (!a || !b) continue;
      const pair = this.incidence.get(edge.source) ?? [];
      pair.push({ id: edge.target, weight: edge.weight });
      this.incidence.set(edge.source, pair);
      // Leave each solid part-way up, capped: an edge at y=0 hides behind the solids it connects, and one
      // anchored to the top of a 126-unit tower sweeps the whole scene.
      const ha = Math.min((this.heights.get(edge.source) ?? 10) * 0.5, 46);
      const hb = Math.min((this.heights.get(edge.target) ?? 10) * 0.5, 46);
      segments.push([[from[0], ha, from[2]], [to[0], hb, to[2]], edge.weight]);
    }
    void nodes;
    this.webSegments(segments);
  }

  private webSegments(segments: Segment[]): void {
    if (segments.length === 0) return;
    const weights = segments.map((s) => s[2]).sort((a, b) => a - b);
    const low = Math.log(weights[0] || 1);
    const high = Math.log(weights[weights.length - 1] || 1);
    const span = Math.max(1e-6, high - low);

    const position = new Float32Array(segments.length * 6);
    const colour = new Float32Array(segments.length * 6);
    const tint = new THREE.Color();
    segments.forEach(([from, to, weight], index) => {
      const t = (Math.log(weight) - low) / span;
      position.set([from[0], from[1], from[2], to[0], to[1], to[2]], index * 6);
      tint.setHSL(0.09 + 0.03 * t, 0.85, 0.1 + 0.3 * t);
      colour.set([tint.r, tint.g, tint.b, tint.r, tint.g, tint.b], index * 6);
    });
    const geo = new THREE.BufferGeometry();
    geo.setAttribute("position", new THREE.BufferAttribute(position, 3));
    geo.setAttribute("color", new THREE.BufferAttribute(colour, 3));
    const material = new THREE.LineBasicMaterial({
      vertexColors: true, transparent: true, opacity: 0.18,
      blending: THREE.AdditiveBlending, depthWrite: false,
    });
    this.add(new THREE.LineSegments(geo, material));
  }

  /** Edges between real packages, filled by the room before `update`. */
  private edgeList: { source: number; target: number; weight: number }[] = [];
  private readonly byId = new Map<number, PackageNode>();

  setEdges(edges: { source: number; target: number; weight: number }[], byId: Map<number, PackageNode>): void {
    this.edgeList = edges;
    this.byId.clear();
    for (const [id, pkg] of byId) this.byId.set(id, pkg);
  }

  flyTo(position: [number, number, number]): void {
    const target = new THREE.Vector3(position[0], position[1] + 20, position[2]);
    const camera = this.camera;
    const distance = 260;
    const dir = new THREE.Vector3().subVectors(camera.position, this.controls.target).normalize();
    const goal = target.clone().addScaledVector(dir, distance);
    if (this.reducedMotion) {
      camera.position.copy(goal);
      this.controls.target.copy(target);
      this.controls.update();
      return;
    }
    const fromPosition = camera.position.clone();
    const fromTarget = this.controls.target.clone();
    const started = performance.now();
    const step = () => {
      const t = Math.min(1, (performance.now() - started) / 420);
      const eased = 1 - Math.pow(1 - t, 3);
      camera.position.lerpVectors(fromPosition, goal, eased);
      this.controls.target.lerpVectors(fromTarget, target, eased);
      this.controls.update();
      if (t < 1 && this.mounted) requestAnimationFrame(step);
    };
    step();
  }

  private add(object: THREE.Object3D): void {
    this.world.add(object);
    object.traverse((child: THREE.Object3D) => {
      const geometry = (child as THREE.Mesh).geometry;
      if (geometry) this.disposables.push(geometry);
      const material = (child as THREE.Mesh).material;
      if (Array.isArray(material)) this.disposables.push(...material);
      else if (material) this.disposables.push(material);
    });
  }

  private clear(): void {
    for (const child of [...this.world.children]) this.world.remove(child);
    for (const item of this.disposables) item.dispose();
    this.disposables.length = 0;
    this.pickables.length = 0;
    this.incidence.clear();
    this.heights.clear();
    this.coreObjects.length = 0;
  }

  /**
   * Frame what was actually built.
   *
   * This used to frame a disc of radius `core` centred on the origin — an assumption about the geometry
   * rather than a measurement of it. It was wrong in two ways at once: the aggregated layer's solids are
   * as tall as their *summed* members, which is far outside the height scale the packages were fitted to,
   * so the model grew upward past the assumption; and a camera placed on an un-normalised direction vector
   * sat 1.19x further out than the solve said. The visible result was a map parked in the lower-right of
   * the viewport.
   *
   * `Box3.setFromObject` costs one traversal of ~260 meshes and is immune to both.
   */
  private frameWorld(): void {
    const box = new THREE.Box3();
    for (const object of this.coreObjects) box.expandByObject(object);
    if (box.isEmpty()) box.setFromObject(this.world);
    if (box.isEmpty()) return;
    // A tight bound, not the box's circumscribed sphere: the diagonal of a wide, flat box is up to sqrt(3)
    // longer than the geometry it holds, which framed this disc at about two-thirds of the panel height and
    // read as an empty screen. Each object's own sphere, measured from the box centre, is the real extent.
    const centre = box.getCenter(new THREE.Vector3());
    let radius = 0;
    for (const object of this.coreObjects) {
      const own = new THREE.Box3().setFromObject(object).getBoundingSphere(new THREE.Sphere());
      if (Number.isFinite(own.radius)) radius = Math.max(radius, own.center.distanceTo(centre) + own.radius);
    }
    radius = Math.max(radius, 1);

    this.controls.target.copy(centre);
    // OrbitControls clamps maxDistance silently, so the ceiling has to admit this framing
    this.controls.maxDistance = Math.max(9000, radius * 10);

    // 1.25, not 1.0: a flat disc seen from above needs room toward its near edge, and a sphere fit does not
    // know about the tilt — at 1.02 the near rim touched the bottom of the panel.
    const fov = (this.camera.fov * Math.PI) / 180;
    const distance = (radius / Math.sin(fov / 2)) * 1.25;
    const direction = new THREE.Vector3(0.55, 0.62, 0.75).normalize();
    this.camera.position.copy(centre).addScaledVector(direction, distance);
    this.camera.near = Math.max(1, radius / 500);
    this.camera.far = Math.max(90000, radius * 40);
    this.camera.updateProjectionMatrix();
    this.controls.update();
  }

  attachPointer(target: HTMLElement): void {
    const move = (event: PointerEvent) => {
      const rect = target.getBoundingClientRect();
      this.pointer.set(
        ((event.clientX - rect.left) / rect.width) * 2 - 1,
        -((event.clientY - rect.top) / rect.height) * 2 + 1,
      );
      this.raycaster.setFromCamera(this.pointer, this.camera);
      const hit = this.raycaster.intersectObjects(this.pickables.map((p) => p.mesh), false)[0];
      const found = hit ? this.pickables.find((p) => p.mesh === hit.object) : undefined;
      this.onHover(found?.entry ?? null);
    };
    const leave = () => this.onHover(null);
    const click = () => {
      this.raycaster.setFromCamera(this.pointer, this.camera);
      const hit = this.raycaster.intersectObjects(this.pickables.map((p) => p.mesh), false)[0];
      const found = hit ? this.pickables.find((p) => p.mesh === hit.object) : undefined;
      this.onPick(found?.entry ?? null);
      if (hit) this.flyTo([hit.object.position.x, hit.object.position.y, hit.object.position.z]);
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerleave", leave);
    target.addEventListener("pointerdown", click);
  }

  private loop = (): void => {
    if (!this.mounted) return;
    this.frame = requestAnimationFrame(this.loop);
    this.controls.update();
    this.renderer.render(this.scene, this.camera);
  };

  dispose(): void {
    this.mounted = false;
    cancelAnimationFrame(this.frame);
    this.clear();
    this.controls.dispose();
    this.renderer.dispose();
    this.renderer.domElement.remove();
  }
}

/** Whether this browser can give the canvas a context — checked before constructing the scene. */
export function webglAvailable(): boolean {
  try {
    const probe = document.createElement("canvas");
    return Boolean(
      probe.getContext("webgl2") ?? probe.getContext("webgl") ?? probe.getContext("experimental-webgl"),
    );
  } catch {
    return false;
  }
}
