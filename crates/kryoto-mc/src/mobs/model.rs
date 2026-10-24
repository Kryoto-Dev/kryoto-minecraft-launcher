//! Reading an entity model out of the game itself.
//!
//! Minecraft builds each model in code: a static method (`createBodyLayer`)
//! calls `CubeListBuilder.create().texOffs(u, v).addBox(...)`,
//! `PartPose.offset(...)` and `PartDefinition.addOrReplaceChild(...)`, and
//! returns `LayerDefinition.create(mesh, width, height)`. This module reads
//! that method's bytecode from the player's installed client jar and runs it
//! on a tiny stack machine that only knows those builder calls (and the
//! handful of opcodes such methods use), giving the parts, their boxes and
//! their poses. Nothing of the game's is copied into the launcher.
//!
//! Anything it does not understand stops it with an error, and that mob is
//! simply not shown.

use std::collections::HashMap;
use std::io::Read;

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cube {
    pub from: [f32; 3],
    pub size: [f32; 3],
    pub uv: [f32; 2],
    pub inflate: [f32; 3],
    pub mirror: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    pub name: String,
    pub pivot: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: f32,
    pub cubes: Vec<Cube>,
    pub children: Vec<Part>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub texture_width: u32,
    pub texture_height: u32,
    pub root: Part,
}

/* ── Class files ────────────────────────────────────────── */

#[derive(Clone, Debug)]
enum Const {
    Utf8(String),
    Int(i32),
    Float(f32),
    Long(i64),
    Double(f64),
    Class(u16),
    Str(u16),
    Ref(u16, u16),
    NameType(u16, u16),
    Other,
}

struct Method {
    name: String,
    desc: String,
    code: Vec<u8>,
}

struct Class {
    pool: Vec<Const>,
    methods: Vec<Method>,
    super_name: String,
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn bytes(&mut self, n: usize) -> Result<&[u8], String> {
        let out = self.data.get(self.pos..self.pos + n).ok_or("The class file ends early.")?;
        self.pos += n;
        Ok(out)
    }
    fn u1(&mut self) -> Result<u8, String> {
        Ok(self.bytes(1)?[0])
    }
    fn u2(&mut self) -> Result<u16, String> {
        let b = self.bytes(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    fn u4(&mut self) -> Result<u32, String> {
        let b = self.bytes(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
}

fn parse_class(data: &[u8]) -> Result<Class, String> {
    let mut r = Reader { data, pos: 0 };
    if r.u4()? != 0xCAFE_BABE {
        return Err("Not a class file.".into());
    }
    r.u2()?;
    r.u2()?;
    let count = r.u2()? as usize;
    let mut pool = vec![Const::Other; count];
    let mut i = 1;
    while i < count {
        let tag = r.u1()?;
        pool[i] = match tag {
            1 => {
                let len = r.u2()? as usize;
                Const::Utf8(String::from_utf8_lossy(r.bytes(len)?).into_owned())
            }
            3 => Const::Int(r.u4()? as i32),
            4 => Const::Float(f32::from_bits(r.u4()?)),
            5 | 6 => {
                let hi = r.u4()? as u64;
                let lo = r.u4()? as u64;
                let bits = (hi << 32) | lo;
                let c = if tag == 5 { Const::Long(bits as i64) } else { Const::Double(f64::from_bits(bits)) };
                pool[i] = c;
                i += 2;
                continue;
            }
            7 => Const::Class(r.u2()?),
            8 => Const::Str(r.u2()?),
            9..=11 => Const::Ref(r.u2()?, r.u2()?),
            12 => Const::NameType(r.u2()?, r.u2()?),
            15 => {
                r.u1()?;
                r.u2()?;
                Const::Other
            }
            16 | 19 | 20 => {
                r.u2()?;
                Const::Other
            }
            17 | 18 => {
                r.u2()?;
                r.u2()?;
                Const::Other
            }
            _ => return Err(format!("Unknown constant tag {tag}.")),
        };
        i += 1;
    }
    r.u2()?;
    r.u2()?;
    let super_index = r.u2()?;
    let interfaces = r.u2()? as usize;
    r.bytes(interfaces * 2)?;
    for _ in 0..r.u2()? {
        r.bytes(6)?;
        for _ in 0..r.u2()? {
            r.u2()?;
            let len = r.u4()? as usize;
            r.bytes(len)?;
        }
    }
    let utf = |pool: &[Const], k: u16| match pool.get(k as usize) {
        Some(Const::Utf8(s)) => s.clone(),
        _ => String::new(),
    };
    let mut methods = Vec::new();
    for _ in 0..r.u2()? {
        r.u2()?;
        let name = utf(&pool, r.u2()?);
        let desc = utf(&pool, r.u2()?);
        let mut code = Vec::new();
        for _ in 0..r.u2()? {
            let attr = utf(&pool, r.u2()?);
            let len = r.u4()? as usize;
            let body = r.bytes(len)?;
            if attr == "Code" && body.len() >= 8 {
                let code_len = u32::from_be_bytes([body[4], body[5], body[6], body[7]]) as usize;
                code = body.get(8..8 + code_len).ok_or("Bad Code attribute.")?.to_vec();
            }
        }
        methods.push(Method { name, desc, code });
    }
    let super_name = match pool.get(super_index as usize) {
        Some(Const::Class(n)) => utf(&pool, *n),
        _ => String::new(),
    };
    Ok(Class { pool, methods, super_name })
}

impl Class {
    fn utf(&self, k: u16) -> &str {
        match self.pool.get(k as usize) {
            Some(Const::Utf8(s)) => s,
            _ => "",
        }
    }
    /// (owner class, name, descriptor) of a field or method reference.
    fn member(&self, k: u16) -> (String, String, String) {
        let Some(Const::Ref(class, nt)) = self.pool.get(k as usize) else { return Default::default() };
        let owner = match self.pool.get(*class as usize) {
            Some(Const::Class(n)) => self.utf(*n).to_string(),
            _ => String::new(),
        };
        let Some(Const::NameType(n, d)) = self.pool.get(*nt as usize) else { return (owner, String::new(), String::new()) };
        (owner, self.utf(*n).to_string(), self.utf(*d).to_string())
    }
}

/// Argument kinds of a method descriptor, and whether it returns something.
fn signature(desc: &str) -> (Vec<char>, bool) {
    let mut args = Vec::new();
    let mut chars = desc.chars().skip(1).peekable();
    while let Some(c) = chars.next() {
        match c {
            ')' => break,
            'L' => {
                for c in chars.by_ref() {
                    if c == ';' {
                        break;
                    }
                }
                args.push('L');
            }
            '[' => {
                while chars.peek() == Some(&'[') {
                    chars.next();
                }
                if chars.next() == Some('L') {
                    for c in chars.by_ref() {
                        if c == ';' {
                            break;
                        }
                    }
                }
                args.push('[');
            }
            c => args.push(c),
        }
    }
    (args, !desc.ends_with(")V"))
}

/* ── The stack machine ──────────────────────────────────── */

#[derive(Clone, Debug)]
enum V {
    Int(i64),
    Float(f64),
    Str(String),
    Null,
    Deform([f32; 3]),
    Pose { offset: [f32; 3], rotation: [f32; 3], scale: f32 },
    Builder(usize),
    Part(usize),
    Mesh(usize),
    Layer { root: usize, width: u32, height: u32 },
    Scale(f32),
    Array(usize),
    New(usize),
    Other,
}

#[derive(Default, Clone)]
struct BuilderState {
    cubes: Vec<Cube>,
    u: f32,
    v: f32,
    mirror: bool,
}

#[derive(Default, Clone)]
struct PartState {
    name: String,
    pose: ([f32; 3], [f32; 3], f32),
    cubes: Vec<Cube>,
    children: Vec<usize>,
}

struct Machine<'a> {
    jar: &'a mut zip::ZipArchive<std::fs::File>,
    classes: HashMap<String, std::rc::Rc<Class>>,
    builders: Vec<BuilderState>,
    parts: Vec<PartState>,
    arrays: Vec<Vec<V>>,
    news: Vec<Option<V>>,
    steps: usize,
}

const MODEL_PREFIX: &str = "net/minecraft/client/model/";

impl Machine<'_> {
    fn class(&mut self, name: &str) -> Result<std::rc::Rc<Class>, String> {
        if let Some(c) = self.classes.get(name) {
            return Ok(c.clone());
        }
        let mut entry = self.jar.by_name(&format!("{name}.class")).map_err(|_| format!("{name} isn't in this game version."))?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        let class = std::rc::Rc::new(parse_class(&bytes)?);
        self.classes.insert(name.to_string(), class.clone());
        Ok(class)
    }

    fn resolve(&self, v: V) -> V {
        match v {
            V::New(i) => self.news.get(i).cloned().flatten().unwrap_or(V::Other),
            v => v,
        }
    }

    fn new_part(&mut self, name: &str, pose: ([f32; 3], [f32; 3], f32)) -> usize {
        self.parts.push(PartState { name: name.into(), pose, ..Default::default() });
        self.parts.len() - 1
    }

    fn run(&mut self, class_name: &str, method: &str, desc: &str, args: Vec<V>, depth: usize) -> Result<V, String> {
        if depth > 6 {
            return Err("The model calls too deep.".into());
        }
        let class = self.class(class_name)?;
        let m = class
            .methods
            .iter()
            .find(|m| m.name == method && m.desc == desc)
            .ok_or_else(|| format!("{class_name}.{method} isn't there."))?;
        let code = &m.code;
        let mut locals: Vec<V> = vec![V::Other; 64];
        let mut slot = 0;
        let (kinds, _) = signature(desc);
        for (arg, kind) in args.into_iter().zip(kinds) {
            locals[slot] = arg;
            slot += if kind == 'J' || kind == 'D' { 2 } else { 1 };
        }
        let mut stack: Vec<V> = Vec::new();
        let mut pc = 0usize;
        macro_rules! pop {
            () => {
                stack.pop().ok_or("The model's bytecode underflowed the stack.")?
            };
        }
        let u16_at = |pc: usize| -> u16 { u16::from_be_bytes([code[pc], code[pc + 1]]) };
        let i16_at = |pc: usize| -> i16 { i16::from_be_bytes([code[pc], code[pc + 1]]) };
        let num = |v: &V| -> f64 {
            match v {
                V::Int(i) => *i as f64,
                V::Float(f) => *f,
                _ => 0.0,
            }
        };
        while pc < code.len() {
            self.steps += 1;
            if self.steps > 200_000 {
                return Err("The model took too long to read.".into());
            }
            let op = code[pc];
            pc += 1;
            match op {
                0x00 => {}
                0x01 => stack.push(V::Null),
                0x02..=0x08 => stack.push(V::Int(op as i64 - 3)),
                0x09 | 0x0a => stack.push(V::Int(op as i64 - 9)),
                0x0b..=0x0d => stack.push(V::Float((op - 0x0b) as f64)),
                0x0e | 0x0f => stack.push(V::Float((op - 0x0e) as f64)),
                0x10 => {
                    stack.push(V::Int(code[pc] as i8 as i64));
                    pc += 1;
                }
                0x11 => {
                    stack.push(V::Int(i16_at(pc) as i64));
                    pc += 2;
                }
                0x12..=0x14 => {
                    let k = if op == 0x12 { code[pc] as u16 } else { u16_at(pc) };
                    pc += if op == 0x12 { 1 } else { 2 };
                    stack.push(match class.pool.get(k as usize) {
                        Some(Const::Int(i)) => V::Int(*i as i64),
                        Some(Const::Float(f)) => V::Float(*f as f64),
                        Some(Const::Long(l)) => V::Int(*l),
                        Some(Const::Double(d)) => V::Float(*d),
                        Some(Const::Str(s)) => V::Str(class.utf(*s).to_string()),
                        _ => V::Other,
                    });
                }
                0x15..=0x19 => {
                    stack.push(locals[code[pc] as usize].clone());
                    pc += 1;
                }
                0x1a..=0x2d => stack.push(locals[((op - 0x1a) % 4) as usize].clone()),
                0x2e..=0x35 => {
                    let i = num(&pop!()) as usize;
                    let a = pop!();
                    stack.push(match a {
                        V::Array(id) => self.arrays[id].get(i).cloned().unwrap_or(V::Other),
                        _ => V::Other,
                    });
                }
                0x36..=0x3a => {
                    let v = pop!();
                    locals[code[pc] as usize] = v;
                    pc += 1;
                }
                0x3b..=0x4e => {
                    let v = pop!();
                    locals[((op - 0x3b) % 4) as usize] = v;
                }
                0x4f..=0x56 => {
                    let v = pop!();
                    let i = num(&pop!()) as usize;
                    if let V::Array(id) = pop!() {
                        if let Some(slot) = self.arrays[id].get_mut(i) {
                            *slot = v;
                        }
                    }
                }
                0x57 => {
                    pop!();
                }
                0x58 => {
                    pop!();
                    pop!();
                }
                0x59 => {
                    let v = stack.last().cloned().ok_or("Stack underflow.")?;
                    stack.push(v);
                }
                0x5a => {
                    let a = pop!();
                    let b = pop!();
                    stack.extend([a.clone(), b, a]);
                }
                0x5f => {
                    let a = pop!();
                    let b = pop!();
                    stack.extend([a, b]);
                }
                0x60..=0x73 => {
                    let b = num(&pop!());
                    let a = num(&pop!());
                    let r = match (op - 0x60) / 4 {
                        0 => a + b,
                        1 => a - b,
                        2 => a * b,
                        3 => a / b,
                        _ => a % b,
                    };
                    stack.push(if (op - 0x60) % 4 < 2 { V::Int(r as i64) } else { V::Float(r) });
                }
                0x74..=0x77 => {
                    let a = num(&pop!());
                    stack.push(if op < 0x76 { V::Int(-a as i64) } else { V::Float(-a) });
                }
                0x84 => {
                    let i = code[pc] as usize;
                    let by = code[pc + 1] as i8 as i64;
                    locals[i] = V::Int(num(&locals[i]) as i64 + by);
                    pc += 2;
                }
                0x85..=0x93 => {
                    let a = num(&pop!());
                    // To int or long: truncate; to float or double: as is.
                    let to_int = matches!(op, 0x85 | 0x88 | 0x8b | 0x8c | 0x8e | 0x8f | 0x91..=0x93);
                    stack.push(if to_int { V::Int(a as i64) } else { V::Float(a) });
                }
                0x94..=0x98 => {
                    let b = num(&pop!());
                    let a = num(&pop!());
                    stack.push(V::Int(if a > b { 1 } else if a < b { -1 } else { 0 }));
                }
                0x99..=0x9e => {
                    let a = num(&pop!());
                    let target = (pc as i64 - 1 + i16_at(pc) as i64) as usize;
                    let jump = match op {
                        0x99 => a == 0.0,
                        0x9a => a != 0.0,
                        0x9b => a < 0.0,
                        0x9c => a >= 0.0,
                        0x9d => a > 0.0,
                        _ => a <= 0.0,
                    };
                    pc = if jump { target } else { pc + 2 };
                }
                0x9f..=0xa4 => {
                    let b = num(&pop!());
                    let a = num(&pop!());
                    let target = (pc as i64 - 1 + i16_at(pc) as i64) as usize;
                    let jump = match op {
                        0x9f => a == b,
                        0xa0 => a != b,
                        0xa1 => a < b,
                        0xa2 => a >= b,
                        0xa3 => a > b,
                        _ => a <= b,
                    };
                    pc = if jump { target } else { pc + 2 };
                }
                0xa7 => pc = (pc as i64 - 1 + i16_at(pc) as i64) as usize,
                0xac..=0xb0 => return Ok(self.resolve(pop!())),
                0xb1 => return Ok(V::Other),
                0xb2 => {
                    let (owner, name, _) = class.member(u16_at(pc));
                    pc += 2;
                    stack.push(match (owner.rsplit('/').next().unwrap_or(""), name.as_str()) {
                        ("PartPose", "ZERO") => V::Pose { offset: [0.0; 3], rotation: [0.0; 3], scale: 1.0 },
                        ("CubeDeformation", "NONE") => V::Deform([0.0; 3]),
                        _ => V::Other,
                    });
                }
                0xb3..=0xb5 => {
                    pc += 2;
                    pop!();
                    if op == 0xb5 {
                        pop!();
                    }
                }
                0xb6..=0xb9 => {
                    let (owner, name, desc) = class.member(u16_at(pc));
                    pc += if op == 0xb9 { 4 } else { 2 };
                    let (kinds, returns) = signature(&desc);
                    let mut args = Vec::with_capacity(kinds.len());
                    for _ in 0..kinds.len() {
                        let v = pop!();
                        args.push(self.resolve(v));
                    }
                    args.reverse();
                    let this = if op == 0xb8 { None } else { Some(pop!()) };
                    let result = self.call(&owner, &name, &desc, this, args, depth)?;
                    if returns {
                        stack.push(result);
                    }
                }
                // String building (part names made in a loop) is beyond this reader.
                0xba => return Err("The model builds names at run time.".into()),
                0xbb => {
                    pc += 2;
                    self.news.push(None);
                    stack.push(V::New(self.news.len() - 1));
                }
                0xbc | 0xbd => {
                    pc += if op == 0xbc { 1 } else { 2 };
                    let n = num(&pop!()).max(0.0) as usize;
                    self.arrays.push(vec![V::Other; n.min(256)]);
                    stack.push(V::Array(self.arrays.len() - 1));
                }
                0xbe => {
                    let len = match pop!() {
                        V::Array(id) => self.arrays[id].len(),
                        _ => 0,
                    };
                    stack.push(V::Int(len as i64));
                }
                0xc0 => pc += 2,
                0xc6 | 0xc7 => {
                    let v = pop!();
                    let null = matches!(v, V::Null);
                    let target = (pc as i64 - 1 + i16_at(pc) as i64) as usize;
                    pc = if (op == 0xc6) == null { target } else { pc + 2 };
                }
                _ => return Err(format!("Unsupported opcode 0x{op:02x} in {class_name}.{method}.")),
            }
        }
        Ok(V::Other)
    }

    fn floats(args: &[V]) -> Vec<f32> {
        args.iter()
            .filter_map(|a| match a {
                V::Float(f) => Some(*f as f32),
                V::Int(i) => Some(*i as f32),
                _ => None,
            })
            .collect()
    }

    fn call(&mut self, owner: &str, name: &str, desc: &str, raw_this: Option<V>, args: Vec<V>, depth: usize) -> Result<V, String> {
        // A constructor fills in the object `new` made; everything else works on the object itself.
        let new_slot = match raw_this {
            Some(V::New(i)) if name == "<init>" => Some(i),
            _ => None,
        };
        let this = raw_this.map(|v| self.resolve(v));
        let simple = owner.rsplit('/').next().unwrap_or("");
        match (simple, name) {
            ("CubeDeformation", "<init>") => {
                let f = Self::floats(&args);
                let d = match f.as_slice() {
                    [g] => [*g; 3],
                    [x, y, z, ..] => [*x, *y, *z],
                    _ => [0.0; 3],
                };
                if let Some(i) = new_slot {
                    self.news[i] = Some(V::Deform(d));
                }
                Ok(V::Other)
            }
            ("CubeDeformation", "extend") => {
                let f = Self::floats(&args);
                let base = match this {
                    Some(V::Deform(d)) => d,
                    _ => [0.0; 3],
                };
                let add = match f.as_slice() {
                    [g] => [*g; 3],
                    [x, y, z, ..] => [*x, *y, *z],
                    _ => [0.0; 3],
                };
                Ok(V::Deform([base[0] + add[0], base[1] + add[1], base[2] + add[2]]))
            }
            ("MeshDefinition", "<init>") => {
                let root = self.new_part("root", ([0.0; 3], [0.0; 3], 1.0));
                if let Some(i) = new_slot {
                    self.news[i] = Some(V::Mesh(root));
                }
                Ok(V::Other)
            }
            ("MeshDefinition", "getRoot") => match this {
                Some(V::Mesh(root)) => Ok(V::Part(root)),
                _ => Err("getRoot on something that isn't a mesh.".into()),
            },
            ("CubeListBuilder", "create") => {
                self.builders.push(BuilderState::default());
                Ok(V::Builder(self.builders.len() - 1))
            }
            ("CubeListBuilder", "texOffs") => {
                let Some(V::Builder(b)) = this else { return Err("texOffs on something that isn't a builder.".into()) };
                let f = Self::floats(&args);
                self.builders[b].u = *f.first().unwrap_or(&0.0);
                self.builders[b].v = *f.get(1).unwrap_or(&0.0);
                Ok(V::Builder(b))
            }
            ("CubeListBuilder", "mirror") => {
                let Some(V::Builder(b)) = this else { return Err("mirror on something that isn't a builder.".into()) };
                self.builders[b].mirror = match args.first() {
                    Some(V::Int(i)) => *i != 0,
                    _ => true,
                };
                Ok(V::Builder(b))
            }
            ("CubeListBuilder", "addBox") => {
                let Some(V::Builder(b)) = this else { return Err("addBox on something that isn't a builder.".into()) };
                // Overloads: (name?, x, y, z, w, h, d, [deformation | mirror | u, v]...).
                let numbers: Vec<f32> = Self::floats(&args);
                if numbers.len() < 6 {
                    return Err("addBox without a box.".into());
                }
                let deform = args.iter().find_map(|a| if let V::Deform(d) = a { Some(*d) } else { None }).unwrap_or([0.0; 3]);
                let (kinds, _) = signature(desc);
                let boolean_mirror = kinds.contains(&'Z').then(|| args.iter().rev().find_map(|a| if let V::Int(i) = a { Some(*i != 0) } else { None })).flatten();
                // addBox(name, x, y, z, w, h, d, u, v): its own texture offset.
                let (mut u, mut v) = (self.builders[b].u, self.builders[b].v);
                if matches!(args.first(), Some(V::Str(_))) && numbers.len() >= 8 && !kinds.contains(&'L') {
                    u = numbers[6];
                    v = numbers[7];
                }
                let st = &mut self.builders[b];
                st.cubes.push(Cube {
                    from: [numbers[0], numbers[1], numbers[2]],
                    size: [numbers[3], numbers[4], numbers[5]],
                    uv: [u, v],
                    inflate: deform,
                    mirror: boolean_mirror.unwrap_or(st.mirror),
                });
                Ok(V::Builder(b))
            }
            ("PartPose", "offset") => {
                let f = Self::floats(&args);
                Ok(V::Pose { offset: [f[0], f[1], f[2]], rotation: [0.0; 3], scale: 1.0 })
            }
            ("PartPose", "rotation") => {
                let f = Self::floats(&args);
                Ok(V::Pose { offset: [0.0; 3], rotation: [f[0], f[1], f[2]], scale: 1.0 })
            }
            ("PartPose", "offsetAndRotation") => {
                let f = Self::floats(&args);
                Ok(V::Pose { offset: [f[0], f[1], f[2]], rotation: [f[3], f[4], f[5]], scale: 1.0 })
            }
            ("PartPose", "withScale") | ("PartPose", "scaled") => {
                let f = Self::floats(&args);
                match this {
                    Some(V::Pose { offset, rotation, scale }) => Ok(V::Pose { offset, rotation, scale: scale * f.first().copied().unwrap_or(1.0) }),
                    _ => Err("withScale on something that isn't a pose.".into()),
                }
            }
            ("PartPose", "translated") => {
                let f = Self::floats(&args);
                match this {
                    Some(V::Pose { offset, rotation, scale }) => Ok(V::Pose { offset: [offset[0] + f[0], offset[1] + f[1], offset[2] + f[2]], rotation, scale }),
                    _ => Err("translated on something that isn't a pose.".into()),
                }
            }
            ("PartDefinition", "addOrReplaceChild") => {
                let Some(V::Part(parent)) = this else { return Err("addOrReplaceChild on something that isn't a part.".into()) };
                let child_name = match args.first() {
                    Some(V::Str(s)) => s.clone(),
                    _ => return Err("A part without a name.".into()),
                };
                let cubes = match args.get(1) {
                    Some(V::Builder(b)) => self.builders[*b].cubes.clone(),
                    _ => Vec::new(),
                };
                let pose = match args.get(2) {
                    Some(V::Pose { offset, rotation, scale }) => (*offset, *rotation, *scale),
                    _ => ([0.0; 3], [0.0; 3], 1.0),
                };
                // Replacing a child drops the old one.
                let existing: Vec<usize> = self.parts[parent].children.iter().copied().filter(|c| self.parts[*c].name == child_name).collect();
                self.parts[parent].children.retain(|c| !existing.contains(c));
                let child = self.new_part(&child_name, pose);
                self.parts[child].cubes = cubes;
                self.parts[parent].children.push(child);
                Ok(V::Part(child))
            }
            ("PartDefinition", "getChild") => {
                let Some(V::Part(parent)) = this else { return Err("getChild on something that isn't a part.".into()) };
                let want = match args.first() {
                    Some(V::Str(s)) => s.clone(),
                    _ => String::new(),
                };
                let found = self.parts[parent].children.iter().copied().find(|c| self.parts[*c].name == want);
                found.map(V::Part).ok_or_else(|| format!("No part called {want}."))
            }
            ("PartDefinition", "clearChild") | ("PartDefinition", "clearRecursively") | ("PartDefinition", "retainPartsAndChildren") | ("PartDefinition", "retainExactParts") => {
                Ok(this.unwrap_or(V::Other))
            }
            ("LayerDefinition", "create") => {
                let root = match args.first() {
                    Some(V::Mesh(root)) => *root,
                    _ => return Err("LayerDefinition.create without a mesh.".into()),
                };
                let f = Self::floats(&args);
                Ok(V::Layer { root, width: *f.first().unwrap_or(&64.0) as u32, height: *f.get(1).unwrap_or(&32.0) as u32 })
            }
            ("LayerDefinition", "apply") => match (this, args.first()) {
                (Some(V::Layer { root, width, height }), Some(V::Scale(s))) => {
                    self.parts[root].pose.2 *= *s;
                    Ok(V::Layer { root, width, height })
                }
                (Some(layer), _) => Ok(layer),
                _ => Err("apply on something that isn't a layer.".into()),
            },
            ("MeshTransformer", "scaling") => Ok(V::Scale(Self::floats(&args).first().copied().unwrap_or(1.0))),
            ("MeshDefinition", "transformed") => Ok(this.unwrap_or(V::Other)),
            ("Math", "toRadians") => Ok(V::Float(Self::floats(&args).first().copied().unwrap_or(0.0).to_radians() as f64)),
            ("Mth", "sin") => Ok(V::Float((Self::floats(&args).first().copied().unwrap_or(0.0)).sin() as f64)),
            ("Mth", "cos") => Ok(V::Float((Self::floats(&args).first().copied().unwrap_or(0.0)).cos() as f64)),
            _ if owner.starts_with(MODEL_PREFIX) && this.is_none() => {
                // A helper in another model class (a shared body, a head).
                self.run(owner, name, desc, args, depth + 1)
            }
            _ => Ok(V::Other),
        }
    }

    fn tree(&self, id: usize) -> Part {
        let p = &self.parts[id];
        Part {
            name: p.name.clone(),
            pivot: p.pose.0,
            rotation: p.pose.1,
            scale: p.pose.2,
            cubes: p.cubes.clone(),
            children: p.children.iter().map(|c| self.tree(*c)).collect(),
        }
    }
}

/// Read a model class's body layer from the jar. The method that builds it is
/// the class's own or a parent's, and returns either the layer or the mesh
/// (then `texture_width`/`texture_height` are 0: the texture's size applies).
pub fn read(jar: &mut zip::ZipArchive<std::fs::File>, class_name: &str) -> Result<Model, String> {
    let mut m = Machine { jar, classes: HashMap::new(), builders: Vec::new(), parts: Vec::new(), arrays: Vec::new(), news: Vec::new(), steps: 0 };
    const LAYER: &str = ")Lnet/minecraft/client/model/geom/builders/LayerDefinition;";
    const MESH: &str = ")Lnet/minecraft/client/model/geom/builders/MeshDefinition;";
    // (class, method, descriptor, rank): lower rank is a better guess.
    let mut best: Option<(String, String, String, u32)> = None;
    let mut current = class_name.to_string();
    for depth in 0..4u32 {
        if !current.starts_with(MODEL_PREFIX) {
            break;
        }
        let class = m.class(&current)?;
        for x in &class.methods {
            let returns_layer = x.desc.ends_with(LAYER);
            if !returns_layer && !x.desc.ends_with(MESH) {
                continue;
            }
            let mut rank = depth * 100 + if returns_layer { 0 } else { 10 };
            let lower = x.name.to_lowercase();
            if lower.contains("baby") || lower.contains("body") {
                rank += 0;
            } else {
                rank += 5;
            }
            if lower.contains("hat") || lower.contains("armor") || lower.contains("outer") || lower.contains("saddle") {
                rank += 50;
            }
            if best.as_ref().is_none_or(|b| rank < b.3) {
                best = Some((current.clone(), x.name.clone(), x.desc.clone(), rank));
            }
        }
        current = class.super_name.clone();
    }
    let (owner, name, desc, _) = best.ok_or_else(|| format!("{class_name} has no body layer."))?;
    let (kinds, _) = signature(&desc);
    let args = kinds
        .iter()
        .map(|k| match k {
            'L' => V::Deform([0.0; 3]),
            'F' | 'D' => V::Float(0.0),
            _ => V::Int(0),
        })
        .collect();
    match m.run(&owner, &name, &desc, args, 0)? {
        V::Layer { root, width, height } => Ok(Model { texture_width: width, texture_height: height, root: m.tree(root) }),
        V::Mesh(root) => Ok(Model { texture_width: 0, texture_height: 0, root: m.tree(root) }),
        other => Err(format!("{class_name} didn't build a model ({other:?}).")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptors() {
        let (args, returns) = signature("(Ljava/lang/String;FFF[ILnet/x/Y;Z)Lnet/Z;");
        assert_eq!(args, ['L', 'F', 'F', 'F', '[', 'L', 'Z']);
        assert!(returns);
        assert!(!signature("()V").1);
    }
}
