//! The terrain shader's Legion lane (`benilla-assets`' `terrain.wgsl`, `LEGION_TERRAIN`) compiles
//! and validates as wgpu's compiler takes it: its block, between its markers, over stand-ins for
//! the bindings it shares with 1.12.1's lane, called from a fragment stage as the shader calls it.
//! No GPU needed.

#[test]
fn the_legion_ground_lane_compiles() {
    let src = include_str!("../../benilla-assets/src/shaders/terrain.wgsl");
    let start = src
        .find("// legion-ground-begin")
        .expect("the Legion lane's begin marker");
    let end = src
        .find("// legion-ground-end")
        .expect("the Legion lane's end marker");
    let block = src[start..end].replace("#{MATERIAL_BIND_GROUP}", "2");
    let shader = format!(
        "@group(2) @binding(100) var layer_array: texture_2d_array<f32>;\n\
         @group(2) @binding(105) var splat_samp: sampler;\n\
         {block}\n\
         @fragment\n\
         fn main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {{\n\
             var color = vec4<f32>(0.0);\n\
             if (legion.flags.x > 0.5) {{\n\
                 color = legion_ground(uv, 8.0, vec4<i32>(1, 2, 3, 1), vec4<f32>(0.2, 0.3, 0.1, 0.0), 4, 0.0);\n\
             }}\n\
             return color;\n\
         }}\n"
    );
    let module = naga::front::wgsl::parse_str(&shader).unwrap_or_else(|e| {
        panic!("{}", e.emit_to_string(&shader));
    });
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&shader)));
}
