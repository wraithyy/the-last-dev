// Radial clock-wipe for action-button cooldowns. params.x = fraction of the
// cooldown still remaining (1 = just started, 0 = ready). The remaining part
// stays dark and the cleared wedge grows clockwise from 12 o'clock, like WoW.
// A vec4 is used because WebGL2 needs 16-byte aligned uniforms.
#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> params: vec4<f32>;

const TAU: f32 = 6.28318530718;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let p = in.uv - vec2<f32>(0.5, 0.5);
    var t = atan2(p.x, -p.y) / TAU;
    if (t < 0.0) {
        t = t + 1.0;
    }
    let elapsed = 1.0 - params.x;
    if (t < elapsed) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    // Bright seam on the sweeping edge so the motion reads at a glance.
    let edge = 1.0 - smoothstep(0.0, 0.012, t - elapsed);
    let shade = vec4<f32>(0.0, 0.0, 0.0, 0.68);
    return mix(shade, vec4<f32>(1.0, 0.95, 0.7, 0.85), edge);
}
