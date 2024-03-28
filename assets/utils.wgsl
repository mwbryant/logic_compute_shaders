const offsets_2d: array<vec2<i32>, 9> = array<vec2<i32>, 9>(
    vec2<i32>(-1, 1),
    vec2<i32>(0, 1),
    vec2<i32>(1, 1),
    vec2<i32>(-1, 0),
    vec2<i32>(0, 0),
    vec2<i32>(1, 0),
    vec2<i32>(-1, -1),
    vec2<i32>(0, -1),
    vec2<i32>(1, -1),
);

const offsets_3d: array<vec3<i32>, 27> = array<vec3<i32>, 27>(
    vec3<i32>(-1, -1, -1),
    vec3<i32>(-1, -1, 0),
    vec3<i32>(-1, -1, 1),
    vec3<i32>(-1, 0, -1),
    vec3<i32>(-1, 0, 0),
    vec3<i32>(-1, 0, 1),
    vec3<i32>(-1, 1, -1),
    vec3<i32>(-1, 1, 0),
    vec3<i32>(-1, 1, 1),
    vec3<i32>(0, -1, -1),
    vec3<i32>(0, -1, 0),
    vec3<i32>(0, -1, 1),
    vec3<i32>(0, 0, -1),
    vec3<i32>(0, 0, 0),
    vec3<i32>(0, 0, 1),
    vec3<i32>(0, 1, -1),
    vec3<i32>(0, 1, 0),
    vec3<i32>(0, 1, 1),
    vec3<i32>(1, -1, -1),
    vec3<i32>(1, -1, 0),
    vec3<i32>(1, -1, 1),
    vec3<i32>(1, 0, -1),
    vec3<i32>(1, 0, 0),
    vec3<i32>(1, 0, 1),
    vec3<i32>(1, 1, -1),
    vec3<i32>(1, 1, 0),
    vec3<i32>(1, 1, 1)
);

const hash_k1: u32 = 15823;
const hash_k2: u32 = 9737333;
const hash_k3: u32 = 440817757;

fn getCell2D(position: vec2<f32>, radius: f32) -> vec2<i32> {
    return vec2<i32>(floor(position / radius));
}

fn getCell3D(position: vec3<f32>, radius: f32) -> vec3<i32> {
    return vec3<i32>(floor(position / radius));
}

fn hashCell2D(cell: vec2<i32>) -> u32 {
    let cell_u = vec2<u32>(u32(cell.x), u32(cell.y));
    let a: u32 = cell_u.x * hash_k1;
    let b: u32 = cell_u.y * hash_k2;
    return (a + b);
}

fn hashCell3D(cell: vec3<i32>) -> u32 {
    let cell_u: vec3<u32> = vec3<u32>(u32(cell.x), u32(cell.y), u32(cell.z));
    return (cell_u.x * hash_k1) + (cell_u.y * hash_k2) + (cell_u.z * hash_k3);
}

fn keyFromHash(hash: u32, table_size: u32) -> u32 {
    return hash % table_size;
}

fn hslToRgb(h: f32, s: f32, l: f32) -> vec3<f32> {
    let c = (1.0 - abs(2.0 * l - 1.0)) * s;
    let x = c * (1.0 - abs(((h / 60.0) % 2.0) - 1.0));
    let m = l - c * 0.5;
    var rgb = vec3<f32>(0.0, 0.0, 0.0);

    if h < 60.0 {
        rgb = vec3<f32>(c, x, 0.0);
    } else if h < 120.0 {
        rgb = vec3<f32>(x, c, 0.0);
    } else if h < 180.0 {
        rgb = vec3<f32>(0.0, c, x);
    } else if h < 240.0 {
        rgb = vec3<f32>(0.0, x, c);
    } else if h < 300.0 {
        rgb = vec3<f32>(x, 0.0, c);
    } else if h < 360.0 {
        rgb = vec3<f32>(c, 0.0, x);
    }

    return rgb + vec3<f32>(m, m, m);
}

fn force(r: f32, a: f32) -> f32 {
    let beta: f32 = 0.3;
    if (r < beta) {
        return r / beta - 1.0;
    } else if (beta < r && r < 1.0) {
        return a * (1.0 - abs(2.0 * r - 1.0 - beta) / (1.0 - beta));
    } else {
        return 0.0;
    }
}
