const FLT_MAX: f32 = 3.40282346638528859812e+38;

struct Uniforms {
  width: u32,
  height: u32,
  frame_count: u32,
}
@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct Rng {
  state: u32,
};
var<private> rng: Rng;

fn init_rng(pixel: vec2u) {
  // Seed the PRNG using the scalar index of the pixel and the current frame count.
  let seed = (pixel.x + pixel.y * uniforms.width) ^ jenkins_hash(uniforms.frame_count);
  rng.state = jenkins_hash(seed);
}

// A slightly modified version of the "One-at-a-Time Hash" function by Bob Jenkins.
// See https://www.burtleburtle.net/bob/hash/doobs.html
fn jenkins_hash(i: u32) -> u32 {
  var x = i;
  x += x << 10u;
  x ^= x >> 6u;
  x += x << 3u;
  x ^= x >> 11u;
  x += x << 15u;
  return x;
}

// The 32-bit "xor" function from Marsaglia G., "Xorshift RNGs", Section 3.
fn xorshift32() -> u32 {
  var x = rng.state;
  x ^= x << 13;
  x ^= x >> 17;
  x ^= x << 5;
  rng.state = x;
  return x;
}

// Returns a random float in the range [0...1]. This sets the floating point exponent to zero and
// sets the most significant 23 bits of a random 32-bit unsigned integer as the mantissa. That
// generates a number in the range [1, 1.9999999], which is then mapped to [0, 0.9999999] by
// subtraction. See Ray Tracing Gems II, Section 14.3.4.
fn rand_f32() -> f32 {
  return bitcast<f32>(0x3f800000u | (xorshift32() >> 9u)) - 1.;
}

struct Intersection {
  normal: vec3f,
  t: f32,
}

fn no_intersection() -> Intersection {
  return Intersection(vec3(0.), -1.);
}

struct Sphere {
  center: vec3f,
  radius: f32,
}

fn intersect_sphere(ray: Ray, sphere: Sphere) -> Intersection {
  let v = ray.origin - sphere.center;
  let a = dot(ray.direction, ray.direction);
  let b = dot(v, ray.direction);
  let c = dot(v, v) - sphere.radius * sphere.radius;

  let d = b * b - a * c;
  if d < 0. {
    return no_intersection();
  }

  let sqrt_d = sqrt(d);
  let recip_a = 1. / a;
  let mb = -b;
  let t1 = (mb - sqrt_d) * recip_a;
  let t2 = (mb + sqrt_d) * recip_a;
  let t = select(t2, t1, t1 > 0.);
  if t <= 0. {
    return no_intersection();
  }

  let p = point_on_ray(ray, t);
  let N = (p - sphere.center) / sphere.radius;
  return Intersection(N, t);
}

struct Ray {
  origin: vec3f,
  direction: vec3f,
}

fn point_on_ray(ray: Ray, t: f32) -> vec3<f32> {
  return ray.origin + t * ray.direction;
}

fn sky_color(ray: Ray) -> vec3f {
  let t = 0.5 * (normalize(ray.direction).y + 1.);
  return (1. - t) * vec3(1.) + t * vec3(0.3, 0.5, 1.);
}

const OBJECT_COUNT: u32 = 2;
alias Spheres = array<Sphere, OBJECT_COUNT>;
var<private> spheres: Spheres = Spheres(
  Sphere(/*center*/ vec3(0., 0., -1.), /*radius*/ 0.5),
  Sphere(/*center*/ vec3(0., -100.5, -1.), /*radius*/ 100.),
);

@group(0) @binding(1) var radiance_samples_old: texture_2d<f32>;
@group(0) @binding(2) var radiance_samples_new: texture_storage_2d<rgba32float, write>;

alias TriangleVertices = array<vec2f, 6>;
var<private> vertices: TriangleVertices = TriangleVertices(
  vec2f(-1.0,  1.0),
  vec2f(-1.0, -1.0),
  vec2f( 1.0,  1.0),
  vec2f( 1.0,  1.0),
  vec2f(-1.0, -1.0),
  vec2f( 1.0, -1.0),
);

@vertex fn path_tracer_vs(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4f {
  return vec4f(vertices[vid], 0.0, 1.0);
}


@fragment fn path_tracer_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  init_rng(vec2u(pos.xy));
  
  let origin = vec3(0.);
  let focus_distance = 1.;
  let aspect_ratio = f32(uniforms.width) / f32(uniforms.height);

  // Offset and normalize the viewport coordinates of the ray.
  let offset = vec2(rand_f32() - 0.5, rand_f32() - 0.5);
  var uv = (pos.xy + offset) / vec2f(f32(uniforms.width - 1u), f32(uniforms.height - 1u));

  // Map `uv` from y-down (normalized) viewport coordinates to camera coordinates.
  uv = (2. * uv - vec2(1.)) * vec2(aspect_ratio, -1.);

  let direction = vec3(uv, -focus_distance);
  let ray = Ray(origin, direction);
  var closest_hit = Intersection(vec3(0.), FLT_MAX);
  for (var i = 0u; i < OBJECT_COUNT; i += 1u) {
    let sphere = spheres[i];
    // var sphere = spheres[i];
    // sphere.radius += sin(f32(uniforms.frame_count) * 0.02) * 0.2;
    let hit = intersect_sphere(ray, sphere);
    if hit.t > 0. && hit.t < closest_hit.t {
      closest_hit = hit;
    }
  }
  var radiance_sample: vec3f;
  if closest_hit.t < FLT_MAX {
    radiance_sample = vec3(0.5 * closest_hit.normal + vec3(0.5));
  } else {
    radiance_sample = sky_color(ray);
  }

  // Fetch the old sum of samples.
  var old_sum: vec3f;
  if uniforms.frame_count > 1 {
    old_sum = textureLoad(radiance_samples_old, vec2u(pos.xy), 0).xyz;
  } else {
    old_sum = vec3(0.);
  }

  // Compute and store the new sum.
  let new_sum = radiance_sample + old_sum;
  textureStore(radiance_samples_new, vec2u(pos.xy), vec4(new_sum, 0.));

  // Display the average.
  return vec4(new_sum / f32(uniforms.frame_count), 1.);
}