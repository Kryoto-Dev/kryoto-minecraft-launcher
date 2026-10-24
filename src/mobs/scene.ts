import * as THREE from 'three'
import type { Mob, MobCube, MobPart } from '@/lib/api'

/**
 * Baby mobs walking along the bottom of the window, in 3D.
 *
 * Each mob's parts, boxes and poses come from its model class in the
 * player's installed game (crates/kryoto-mc/src/mobs), and its texture from
 * the same jar; the launcher ships none of Mojang's files. Boxes use
 * Minecraft's box UV layout and its per-face shading, rotations its Z-Y-X
 * order, so they look the way they do in game. Movement is ours: legs, arms,
 * heads, tails and wings are found by their part names and animated.
 *
 * Model space is the game's: y points down, feet at y = 24, front towards -z.
 * World space here: y up, x mirrored, as the game draws it.
 */

type V3 = [number, number, number]

function quad(pos: number[], uv: number[], col: number[], c: V3[], r: [number, number, number, number], flipU: boolean, tw: number, th: number, shade: number) {
  const [u, v, w, h] = r
  const left = flipU ? u + w : u
  const right = flipU ? u : u + w
  const uvs = [
    [left, v],
    [right, v],
    [right, v + h],
    [left, v + h],
  ].map(([a, b]) => [a! / tw, 1 - b! / th])
  for (const i of [0, 1, 2, 0, 2, 3]) {
    pos.push(...c[i]!)
    uv.push(...uvs[i]!)
    col.push(shade, shade, shade)
  }
}

/** One box in its part's world frame: Minecraft's box UVs, its face shading, inflation and mirroring. */
function cubeGeometry(c: MobCube, tw: number, th: number): THREE.BufferGeometry {
  const [w, h, d] = c.size
  const [gx, gy, gz] = c.inflate
  const [x, y, z] = [c.from[0] - gx, c.from[1] - gy, c.from[2] - gz]
  const [W, H, D] = [w + 2 * gx, h + 2 * gy, d + 2 * gz]
  const [u, v] = c.uv
  const X0 = -(x + W),
    X1 = -x,
    Y0 = -(y + H),
    Y1 = -y,
    Z0 = z,
    Z1 = z + D
  const pos: number[] = []
  const uv: number[] = []
  const col: number[] = []
  const m = c.mirror
  const side = (a: [number, number, number, number], b: [number, number, number, number]) => (m ? [b, a] : [a, b])
  const [px, nx] = side([u, v + d, d, h], [u + d + w, v + d, d, h])
  const q = (corners: V3[], rect: [number, number, number, number], shade: number) => quad(pos, uv, col, corners, rect, m, tw, th, shade)
  if (d === 0) q([[X1, Y1, Z0], [X0, Y1, Z0], [X0, Y0, Z0], [X1, Y0, Z0]], [u + d, v + d, w, h], 0.8)
  else if (h === 0) q([[X1, Y1, Z1], [X0, Y1, Z1], [X0, Y1, Z0], [X1, Y1, Z0]], [u + d, v, w, d], 1)
  else if (w === 0) q([[X1, Y1, Z1], [X1, Y1, Z0], [X1, Y0, Z0], [X1, Y0, Z1]], px!, 0.6)
  else {
    q([[X1, Y1, Z1], [X0, Y1, Z1], [X0, Y1, Z0], [X1, Y1, Z0]], [u + d, v, w, d], 1)
    q([[X1, Y0, Z0], [X0, Y0, Z0], [X0, Y0, Z1], [X1, Y0, Z1]], [u + d + w, v, w, d], 0.5)
    q([[X1, Y1, Z0], [X0, Y1, Z0], [X0, Y0, Z0], [X1, Y0, Z0]], [u + d, v + d, w, h], 0.8)
    q([[X0, Y1, Z1], [X1, Y1, Z1], [X1, Y0, Z1], [X0, Y0, Z1]], [u + 2 * d + w, v + d, w, h], 0.8)
    q([[X1, Y1, Z1], [X1, Y1, Z0], [X1, Y0, Z0], [X1, Y0, Z1]], px!, 0.6)
    q([[X0, Y1, Z0], [X0, Y1, Z1], [X0, Y0, Z1], [X0, Y0, Z0]], nx!, 0.6)
  }
  const g = new THREE.BufferGeometry()
  g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3))
  g.setAttribute('uv', new THREE.Float32BufferAttribute(uv, 2))
  g.setAttribute('color', new THREE.Float32BufferAttribute(col, 3))
  return g
}

/** A part and what its rest pose was, so animation adds to it. */
type Joint = { name: string; group: THREE.Group; rest: V3 }

/** Model rotations (x, y, z) to world ones: mirroring x and y flips the x and y angles. */
function setRotation(group: THREE.Group, r: V3) {
  group.rotation.set(-r[0], -r[1], r[2], 'ZYX')
}

function buildPart(part: MobPart, material: THREE.Material, tw: number, th: number, joints: Joint[]): THREE.Group {
  const group = new THREE.Group()
  group.position.set(-part.pivot[0], -part.pivot[1], part.pivot[2])
  setRotation(group, part.rotation)
  if (part.scale && part.scale !== 1) group.scale.setScalar(part.scale)
  for (const c of part.cubes) group.add(new THREE.Mesh(cubeGeometry(c, tw, th), material))
  for (const child of part.children) group.add(buildPart(child, material, tw, th, joints))
  joints.push({ name: part.name.toLowerCase(), group, rest: part.rotation })
  return group
}

const PX = 5 // screen pixels per model pixel
const GROUND = 1.6 // model pixels between the canvas bottom and the ground
const MAX_HEIGHT = 24 // taller babies are drawn smaller

type State = 'idle' | 'walk' | 'graze' | 'look'

class Walker {
  root = new THREE.Group()
  body = new THREE.Group()
  joints: Joint[] = []
  width = 8
  height = 8
  scale = 1
  x: number
  z: number
  targetX: number
  yaw = Math.PI
  targetYaw = Math.PI
  state: State = 'idle'
  timer: number
  walkPos = Math.random() * 10
  walkSpeed = 0
  hopY = 0
  hopV = 0
  flap = 0
  flapTimer = 0
  graze = 0
  speed: number

  constructor(
    readonly mob: Mob,
    material: THREE.Material,
    x: number,
    z: number,
  ) {
    const { textureWidth: tw, textureHeight: th, root } = mob.model
    const model = buildPart(root, material, tw || 64, th || 64, this.joints)
    // Feet on the ground: the model's y = 24 is the floor.
    model.position.y += 24
    this.body.add(model)
    const box = new THREE.Box3().setFromObject(this.body)
    const size = box.getSize(new THREE.Vector3())
    this.scale = size.y > MAX_HEIGHT ? MAX_HEIGHT / size.y : 1
    this.body.scale.setScalar(this.scale)
    this.body.position.y = -box.min.y * this.scale
    this.width = Math.max(size.x, size.z) * this.scale
    this.height = size.y * this.scale
    const shadow = new THREE.Mesh(new THREE.CircleGeometry(1, 24), new THREE.MeshBasicMaterial({ color: 0x000000, transparent: true, opacity: 0.26, depthWrite: false }))
    shadow.rotation.x = -Math.PI / 2
    shadow.position.y = 0.02
    shadow.scale.set(Math.max(2, size.x * this.scale * 0.6), Math.max(1.6, size.z * this.scale * 0.45), 1)
    this.root.add(this.body, shadow)
    this.x = x
    this.z = z
    this.targetX = x
    this.timer = 0.5 + Math.random() * 2.5
    this.speed = mob.gait === 'hop' ? 9 : 5 + Math.random() * 2.5
  }

  find(test: (n: string) => boolean) {
    return this.joints.filter((j) => test(j.name))
  }

  get zombieArms() {
    return this.mob.mean && /zombie|drowned/.test(this.mob.id)
  }
}

export type MobScene = { dispose: () => void; tap: (x: number, y: number) => boolean }

export async function startMobs(canvas: HTMLCanvasElement, mobs: Mob[], still: boolean): Promise<MobScene> {
  const loader = new THREE.TextureLoader()
  const renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: false, powerPreference: 'low-power' })
  renderer.setPixelRatio(Math.min(2, window.devicePixelRatio || 1))
  renderer.outputColorSpace = THREE.SRGBColorSpace
  const scene = new THREE.Scene()
  const camera = new THREE.OrthographicCamera()
  const tilt = 0.3
  camera.position.set(0, Math.sin(tilt) * 80, Math.cos(tilt) * 80)
  camera.lookAt(0, 0, 0)

  let width = 0
  let halfSpan = 0
  const resize = () => {
    width = canvas.clientWidth
    const height = canvas.clientHeight
    renderer.setSize(width, height, false)
    camera.left = -width / 2 / PX
    camera.right = width / 2 / PX
    camera.bottom = -GROUND
    camera.top = height / PX - GROUND
    camera.near = 1
    camera.far = 200
    camera.updateProjectionMatrix()
    halfSpan = Math.max(6, width / 2 / PX - 10)
  }
  resize()
  const observer = new ResizeObserver(resize)
  observer.observe(canvas)

  const textures: THREE.Texture[] = []
  const walkers: Walker[] = []
  for (let i = 0; i < mobs.length; i++) {
    const mob = mobs[i]!
    const texture = await loader.loadAsync(mob.texture)
    texture.magFilter = THREE.NearestFilter
    texture.minFilter = THREE.NearestFilter
    texture.colorSpace = THREE.SRGBColorSpace
    texture.generateMipmaps = false
    textures.push(texture)
    const material = new THREE.MeshBasicMaterial({ map: texture, vertexColors: true, alphaTest: 0.5, side: THREE.DoubleSide })
    // Spread out across the window, each on its own lane.
    const slot = mobs.length === 1 ? 0 : (i / (mobs.length - 1)) * 2 - 1
    const w = new Walker(mob, material, slot * halfSpan * 0.8 + (Math.random() - 0.5) * 6, (i % 3) * 3 - 3)
    walkers.push(w)
    scene.add(w.root)
  }

  const angle = (a: number) => Math.atan2(Math.sin(a), Math.cos(a))
  const facing = (dir: number) => (dir > 0 ? -Math.PI / 2 - 0.45 : Math.PI / 2 + 0.45)

  const choose = (w: Walker) => {
    const r = Math.random()
    if (r < 0.55) {
      const span = 10 + Math.random() * 40
      let target = w.x + (Math.random() < 0.5 ? -span : span)
      if (target < -halfSpan || target > halfSpan) target = w.x - (target - w.x)
      w.targetX = Math.max(-halfSpan, Math.min(halfSpan, target))
      w.state = 'walk'
    } else if (r < 0.78) {
      w.state = 'graze'
      w.timer = 1 + Math.random() * 1.5
    } else {
      w.state = 'look'
      w.targetYaw = Math.PI + (Math.random() - 0.5) * 0.7
      w.timer = 1.2 + Math.random() * 1.6
    }
  }

  const animate = (w: Walker, dt: number, now: number) => {
    if (!still && !document.hidden) {
      if (w.state === 'walk') {
        const dir = Math.sign(w.targetX - w.x)
        w.targetYaw = facing(dir)
        const hopping = w.mob.gait === 'hop'
        if (!hopping || w.hopY > 0) w.x += dir * w.speed * dt
        else if (w.hopY === 0) {
          w.hopV = 9
          w.hopY = 0.01
        }
        w.walkSpeed = Math.min(1, w.walkSpeed + dt * 4)
        if (Math.abs(w.targetX - w.x) < 0.4 || w.x <= -halfSpan || w.x >= halfSpan) {
          w.x = Math.max(-halfSpan, Math.min(halfSpan, w.x))
          w.state = 'idle'
          w.timer = 0.8 + Math.random() * 3
        }
      } else {
        w.walkSpeed = Math.max(0, w.walkSpeed - dt * 5)
        w.timer -= dt
        if (w.timer <= 0) {
          if (w.state === 'idle') choose(w)
          else {
            w.state = 'idle'
            w.timer = 0.8 + Math.random() * 3
          }
        }
      }
      w.walkPos += dt * 9 * w.walkSpeed
      if (w.hopY > 0 || w.hopV > 0) {
        w.hopV -= 60 * dt
        w.hopY = Math.max(0, w.hopY + w.hopV * dt)
        if (w.hopY === 0) w.hopV = 0
      }
      w.flapTimer -= dt
      if (w.flapTimer <= 0 && Math.random() < dt * 0.15) w.flapTimer = 0.5
      w.flap = w.hopY > 0 || w.flapTimer > 0 ? Math.abs(Math.sin(now / 55)) : 0
      w.graze += ((w.state === 'graze' ? 1 : 0) - w.graze) * Math.min(1, dt * 6)
      w.yaw += angle(w.targetYaw - w.yaw) * Math.min(1, dt * 7)
    }

    w.root.position.set(w.x, w.hopY, w.z)
    w.root.rotation.y = w.yaw
    // About what the game swings at a walk; its full 1.4 is a sprint.
    const swing = Math.cos(w.walkPos * 0.6662 * 2) * 0.55 * w.walkSpeed * (w.mob.gait === 'hop' ? 0 : 1)
    const pose = (j: Joint, dx: number, dy = 0, dz = 0) => setRotation(j.group, [j.rest[0] + dx, j.rest[1] + dy, j.rest[2] + dz])
    const quadruped = w.find((n) => n.includes('leg') && (n.includes('front') || n.includes('hind'))).length > 0
    for (const j of w.find((n) => n.includes('leg'))) {
      const left = j.name.includes('left')
      const hind = j.name.includes('hind') || j.name.includes('back')
      const phase = quadruped ? (left !== hind ? 1 : -1) : left ? 1 : -1
      pose(j, swing * phase)
    }
    for (const j of w.find((n) => n.includes('arm') && !n.includes('armor'))) {
      const left = j.name.includes('left')
      if (w.zombieArms) pose(j, -Math.PI / 2 + Math.sin(now / 700 + (left ? 0 : 1)) * 0.05)
      else pose(j, -swing * (left ? 1 : -1) * 0.8)
    }
    for (const j of w.find((n) => n === 'head')) pose(j, w.graze * 0.7 + Math.sin(now / 900 + w.x) * 0.04, Math.sin(now / 1300 + w.z) * 0.15 * (1 - w.walkSpeed))
    for (const j of w.find((n) => n.includes('tail'))) pose(j, 0, Math.sin(now / 160) * 0.25 * (0.4 + w.walkSpeed))
    for (const j of w.find((n) => n.includes('wing'))) pose(j, 0, 0, (j.name.includes('left') ? -1 : 1) * w.flap * 1.1)
  }

  let last = performance.now()
  let frame = 0
  const step = (now: number) => {
    const dt = Math.min(0.05, (now - last) / 1000)
    last = now
    for (const w of walkers) animate(w, dt, now)
    renderer.render(scene, camera)
    frame = requestAnimationFrame(step)
  }
  frame = requestAnimationFrame(step)

  return {
    /** A tap at a point in the canvas (CSS pixels from its top-left): the mob there hops. */
    tap(px: number, py: number) {
      const groundY = canvas.clientHeight - GROUND * PX
      for (const w of walkers) {
        const cx = width / 2 + w.x * PX
        const top = groundY - (w.height + w.hopY + 2) * PX
        if (Math.abs(px - cx) < (w.width / 2 + 2) * PX && py > top && py < groundY + 2 * PX) {
          if (w.hopY === 0) {
            w.hopV = 15
            w.hopY = 0.01
          }
          return true
        }
      }
      return false
    },
    dispose() {
      cancelAnimationFrame(frame)
      observer.disconnect()
      renderer.dispose()
      for (const t of textures) t.dispose()
    },
  }
}
