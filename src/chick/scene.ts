import * as THREE from 'three'

/**
 * The baby chicken, in 3D, walking along the bottom of the window.
 *
 * The shape is Minecraft's baby chicken model (26.x `BabyChickenModel`): a
 * 4x4x4 body that is also the head, a beak, flat legs with feet, flat wings.
 * The texture is read from the player's own installed game at runtime
 * (crates/kryoto-mc/src/mobs.rs); the launcher ships none of Mojang's files.
 *
 * Units are the model's pixels. Boxes use Minecraft's box UV layout and its
 * per-face shading (top lit, sides dimmer, bottom darkest), so it looks the
 * way it does in game.
 */

type Box = { tex: [number, number]; from: [number, number, number]; size: [number, number, number] }
type PartSpec = { pivot: [number, number, number]; boxes: Box[] }

const TEX = 16

// Minecraft model space: y points down, the feet stand at y = 24.
const PARTS: Record<'body' | 'leftLeg' | 'rightLeg' | 'leftWing' | 'rightWing', PartSpec> = {
  body: {
    pivot: [0, 20.25, -1.25],
    boxes: [
      { tex: [0, 0], from: [-2, -2.25, -0.75], size: [4, 4, 4] },
      { tex: [10, 8], from: [-1, -0.25, -1.75], size: [2, 1, 1] },
    ],
  },
  leftLeg: {
    pivot: [1, 22, 0.5],
    boxes: [
      { tex: [2, 2], from: [-0.5, 0, 0], size: [1, 2, 0] },
      { tex: [0, 1], from: [-0.5, 2, -1], size: [1, 0, 1] },
    ],
  },
  rightLeg: {
    pivot: [-1, 22, 0.5],
    boxes: [
      { tex: [0, 2], from: [-0.5, 0, 0], size: [1, 2, 0] },
      { tex: [0, 0], from: [-0.5, 2, -1], size: [1, 0, 1] },
    ],
  },
  rightWing: { pivot: [2, 20, 0], boxes: [{ tex: [6, 8], from: [0, 0, -1], size: [1, 0, 2] }] },
  leftWing: { pivot: [-2, 20, 0], boxes: [{ tex: [4, 8], from: [-1, 0, -1], size: [1, 0, 2] }] },
}

type V3 = [number, number, number]

/** One textured, shaded quad: corners seen from outside (TL, TR, BR, BL), a texture rectangle. */
function quad(pos: number[], uv: number[], col: number[], c: V3[], r: [number, number, number, number], shade: number) {
  const [u, v, w, h] = r
  const uvs = [
    [u, v],
    [u + w, v],
    [u + w, v + h],
    [u, v + h],
  ].map(([a, b]) => [a! / TEX, 1 - b! / TEX])
  for (const i of [0, 1, 2, 0, 2, 3]) {
    pos.push(...c[i]!)
    uv.push(...uvs[i]!)
    col.push(shade, shade, shade)
  }
}

/** A box in world space (x mirrored and y flipped from model space, as the game draws it). */
function boxGeometry(b: Box): THREE.BufferGeometry {
  const [x, y, z] = b.from
  const [w, h, d] = b.size
  const [u, v] = b.tex
  const X0 = -(x + w),
    X1 = -x,
    Y0 = -(y + h),
    Y1 = -y,
    Z0 = z,
    Z1 = z + d
  const pos: number[] = []
  const uv: number[] = []
  const col: number[] = []
  // A flat box (a leg, a wing) is one double-sided quad.
  if (d === 0) quad(pos, uv, col, [[X1, Y1, Z0], [X0, Y1, Z0], [X0, Y0, Z0], [X1, Y0, Z0]], [u + d, v + d, w, h], 0.8)
  else if (h === 0) quad(pos, uv, col, [[X1, Y1, Z1], [X0, Y1, Z1], [X0, Y1, Z0], [X1, Y1, Z0]], [u + d, v, w, d], 1)
  else {
    quad(pos, uv, col, [[X1, Y1, Z1], [X0, Y1, Z1], [X0, Y1, Z0], [X1, Y1, Z0]], [u + d, v, w, d], 1) // top
    quad(pos, uv, col, [[X1, Y0, Z0], [X0, Y0, Z0], [X0, Y0, Z1], [X1, Y0, Z1]], [u + d + w, v, w, d], 0.5) // bottom
    quad(pos, uv, col, [[X1, Y1, Z0], [X0, Y1, Z0], [X0, Y0, Z0], [X1, Y0, Z0]], [u + d, v + d, w, h], 0.8) // front
    quad(pos, uv, col, [[X0, Y1, Z1], [X1, Y1, Z1], [X1, Y0, Z1], [X0, Y0, Z1]], [u + 2 * d + w, v + d, w, h], 0.8) // back
    quad(pos, uv, col, [[X1, Y1, Z1], [X1, Y1, Z0], [X1, Y0, Z0], [X1, Y0, Z1]], [u, v + d, d, h], 0.6) // one side
    quad(pos, uv, col, [[X0, Y1, Z0], [X0, Y1, Z1], [X0, Y0, Z1], [X0, Y0, Z0]], [u + d + w, v + d, d, h], 0.6) // the other
  }
  const g = new THREE.BufferGeometry()
  g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3))
  g.setAttribute('uv', new THREE.Float32BufferAttribute(uv, 2))
  g.setAttribute('color', new THREE.Float32BufferAttribute(col, 3))
  return g
}

type Rig = { root: THREE.Group; body: THREE.Group; leftLeg: THREE.Group; rightLeg: THREE.Group; leftWing: THREE.Group; rightWing: THREE.Group }

function buildChick(texture: THREE.Texture): Rig {
  const material = new THREE.MeshBasicMaterial({ map: texture, vertexColors: true, alphaTest: 0.5, side: THREE.DoubleSide })
  const root = new THREE.Group()
  const make = (spec: PartSpec) => {
    const g = new THREE.Group()
    g.position.set(-spec.pivot[0], 24 - spec.pivot[1], spec.pivot[2])
    for (const b of spec.boxes) g.add(new THREE.Mesh(boxGeometry(b), material))
    root.add(g)
    return g
  }
  const rig = {
    root,
    body: make(PARTS.body),
    leftLeg: make(PARTS.leftLeg),
    rightLeg: make(PARTS.rightLeg),
    leftWing: make(PARTS.leftWing),
    rightWing: make(PARTS.rightWing),
  }
  // A soft shadow to stand on.
  const shadow = new THREE.Mesh(new THREE.CircleGeometry(2.6, 20), new THREE.MeshBasicMaterial({ color: 0x000000, transparent: true, opacity: 0.28, depthWrite: false }))
  shadow.rotation.x = -Math.PI / 2
  shadow.position.y = 0.02
  shadow.scale.set(1, 0.75, 1)
  root.add(shadow)
  return rig
}

const PX = 6 // screen pixels per model pixel
const GROUND = 1.6 // model pixels between the canvas bottom and the ground

type State = 'idle' | 'walk' | 'peck' | 'look'

export type ChickScene = { dispose: () => void; hop: () => void; hit: (x: number, y: number) => boolean }

export async function startChick(canvas: HTMLCanvasElement, textureUrl: string, still: boolean): Promise<ChickScene> {
  const texture = await new THREE.TextureLoader().loadAsync(textureUrl)
  texture.magFilter = THREE.NearestFilter
  texture.minFilter = THREE.NearestFilter
  texture.colorSpace = THREE.SRGBColorSpace
  texture.generateMipmaps = false

  const renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: false, powerPreference: 'low-power' })
  renderer.setPixelRatio(Math.min(2, window.devicePixelRatio || 1))
  renderer.outputColorSpace = THREE.SRGBColorSpace
  const scene = new THREE.Scene()
  const camera = new THREE.OrthographicCamera()
  // A little from above, the way you look down at a chick.
  const tilt = 0.32
  camera.position.set(0, Math.sin(tilt) * 60, Math.cos(tilt) * 60)
  camera.lookAt(0, 0, 0)

  const rig = buildChick(texture)
  scene.add(rig.root)

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
    camera.updateProjectionMatrix()
    halfSpan = Math.max(4, width / 2 / PX - 6)
  }
  resize()
  const observer = new ResizeObserver(resize)
  observer.observe(canvas)

  // Where it is and what it is doing.
  let x = (Math.random() * 2 - 1) * halfSpan * 0.6
  let targetX = x
  let yaw = Math.PI
  let targetYaw = Math.PI
  let state: State = 'idle'
  let timer = 1 + Math.random() * 2
  let walkPos = 0
  let walkSpeed = 0
  let hopY = 0
  let hopV = 0
  let flap = 0
  let flapTimer = 0
  let peckPhase = 0

  const faceCamera = Math.PI
  const facing = (dir: number) => (dir > 0 ? -Math.PI / 2 - 0.5 : Math.PI / 2 + 0.5)
  const choose = () => {
    const r = Math.random()
    if (r < 0.55) {
      const span = 8 + Math.random() * 36
      targetX = Math.max(-halfSpan, Math.min(halfSpan, x + (Math.random() < 0.5 ? -span : span)))
      if (Math.abs(targetX - x) < 3) targetX = Math.max(-halfSpan, Math.min(halfSpan, x - Math.sign(targetX - x || 1) * span))
      state = 'walk'
    } else if (r < 0.78) {
      state = 'peck'
      timer = 0.9 + Math.random() * 0.9
      peckPhase = 0
    } else {
      state = 'look'
      targetYaw = faceCamera + (Math.random() - 0.5) * 0.6
      timer = 1.2 + Math.random() * 1.5
    }
  }

  const angle = (a: number) => Math.atan2(Math.sin(a), Math.cos(a))
  let last = performance.now()
  let frame = 0
  const step = (now: number) => {
    const dt = Math.min(0.05, (now - last) / 1000)
    last = now
    if (!still && !document.hidden) {
      if (state === 'walk') {
        const dir = Math.sign(targetX - x)
        targetYaw = facing(dir)
        const speed = 7
        x += dir * speed * dt
        walkSpeed = Math.min(1, walkSpeed + dt * 4)
        if (Math.abs(targetX - x) < 0.3 || x <= -halfSpan || x >= halfSpan) {
          x = Math.max(-halfSpan, Math.min(halfSpan, x))
          state = 'idle'
          timer = 0.8 + Math.random() * 2.5
        }
      } else {
        walkSpeed = Math.max(0, walkSpeed - dt * 5)
        timer -= dt
        if (state === 'peck') peckPhase += dt
        if (timer <= 0) {
          if (state === 'idle') choose()
          else {
            state = 'idle'
            timer = 0.8 + Math.random() * 2.5
          }
        }
      }
      walkPos += dt * 9 * walkSpeed
      // Hops (a click), and now and then a flap for nothing.
      if (hopY > 0 || hopV > 0) {
        hopV -= 70 * dt
        hopY = Math.max(0, hopY + hopV * dt)
        if (hopY === 0) hopV = 0
      }
      flapTimer -= dt
      if (flapTimer <= 0 && Math.random() < dt * 0.15) flapTimer = 0.5
      flap = hopY > 0 || flapTimer > 0 ? Math.abs(Math.sin(now / 55)) : 0
      yaw += angle(targetYaw - yaw) * Math.min(1, dt * 8)
    }

    rig.root.position.set(x, hopY, 0)
    rig.root.rotation.y = yaw
    const swing = Math.cos(walkPos * 0.6662 * 2) * 1.2 * walkSpeed
    rig.rightLeg.rotation.x = swing
    rig.leftLeg.rotation.x = -swing
    rig.rightWing.rotation.z = flap * 1.1
    rig.leftWing.rotation.z = -flap * 1.1
    const peck = state === 'peck' ? Math.max(0, Math.sin(peckPhase * 11)) : 0
    rig.body.rotation.x = -peck * 0.55
    rig.body.position.y = 24 - PARTS.body.pivot[1] + Math.abs(Math.sin(walkPos * 0.6662 * 2)) * 0.25 * walkSpeed
    renderer.render(scene, camera)
    frame = requestAnimationFrame(step)
  }
  frame = requestAnimationFrame(step)

  return {
    hop() {
      if (hopY === 0) {
        hopV = 16
        hopY = 0.01
      }
    },
    /** Whether a point in the canvas (CSS pixels from its top-left) is on the chick. */
    hit(px: number, py: number) {
      const cx = width / 2 + x * PX
      const groundY = canvas.clientHeight - GROUND * PX - hopY * PX
      return Math.abs(px - cx) < 5 * PX && py > groundY - 9 * PX && py < groundY + 2 * PX
    },
    dispose() {
      cancelAnimationFrame(frame)
      observer.disconnect()
      renderer.dispose()
      texture.dispose()
    },
  }
}
