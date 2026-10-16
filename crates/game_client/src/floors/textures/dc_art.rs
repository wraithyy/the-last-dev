//! Data-center art: rack fronts and backs, the all-red NOC dashboards and the zone's signs.
//! Split out of `textures.rs` so the data-center pass touches one place; it draws with the
//! parent's private canvas and sign spec.

use super::{Canvas, Rgb, SignSpec, hash};
use crate::floors::kit::{Sign, Tex};

const GREEN: Rgb = [70, 255, 130];
const AMBER: Rgb = [255, 190, 60];
const BLUE: Rgb = [90, 160, 255];
const RED_LED: Rgb = [255, 70, 55];
const CABLES: [Rgb; 6] = [
    [18, 18, 22],
    [150, 128, 30],
    [36, 72, 170],
    [150, 44, 40],
    [56, 58, 68],
    [44, 130, 66],
];

pub(super) fn draw(tex: Tex) -> Canvas {
    match tex {
        Tex::RackBlades => blades(false),
        Tex::RackScorched => blades(true),
        Tex::RackPatch => patch(),
        Tex::RackDrives => drives(),
        Tex::RackBack => back(false),
        Tex::RackBackHot => back(true),
        Tex::NocGraph => noc_graph(),
        Tex::NocAlerts => noc_alerts(),
        _ => noc_regions(),
    }
}

fn rack_frame(c: &mut Canvas) {
    let (w, h) = (c.w as i32, c.h as i32);
    c.rect(0, 0, w, h, [16, 17, 22]);
    c.rect(0, 0, 2, h, [38, 40, 48]);
    c.rect(w - 2, 0, 2, h, [38, 40, 48]);
    c.rect(0, 0, w, 2, [38, 40, 48]);
    c.rect(0, h - 2, w, 2, [38, 40, 48]);
}

fn led(c: &mut Canvas, x: i32, y: i32, k: f32, hot: bool) {
    let col = if hot {
        [RED_LED, AMBER, RED_LED, [90, 20, 16]][(k * 4.0) as usize % 4]
    } else {
        [GREEN, GREEN, BLUE, AMBER][(k * 4.0) as usize % 4]
    };
    c.rect(x, y, 2, 1, col);
}

fn blades(scorched: bool) -> Canvas {
    let mut c = Canvas::new(44, 100, [16, 17, 22]);
    rack_frame(&mut c);
    for u in 0..24u32 {
        let y = 2 + u as i32 * 4;
        let r = hash(u, 7 + scorched as u32);
        // Scorched racks have units pulled half out and whole gaps where hardware was salvaged.
        let pulled = scorched && r > 0.8;
        let x0 = if pulled { 6 } else { 3 };
        if scorched && r < 0.18 {
            continue;
        }
        if r < 0.12 {
            c.rect(3, y, 38, 3, [24, 25, 31]);
        } else if r > 0.72 {
            c.rect(x0, y, 38, 3, [30, 38, 58]);
            for p in 0..8 {
                c.rect(x0 + 3 + p * 3, y + 1, 2, 1, [10, 12, 18]);
            }
            led(&mut c, x0 + 33, y + 1, hash(u, 3), scorched);
        } else {
            c.rect(x0, y, 38, 3, [46, 48, 58]);
            for p in 0..9 {
                c.rect(x0 + 2 + p * 2, y + 1, 1, 1, [18, 19, 24]);
            }
            for b in 0..3 {
                c.rect(x0 + 22 + b * 4, y, 3, 3, [30, 32, 40]);
            }
            led(&mut c, x0 + 34, y + 1, hash(u, 5), scorched);
        }
        c.rect(3, y + 3, 38, 1, [10, 10, 14]);
        if pulled {
            c.rect(3, y, 3, 3, [8, 8, 10]);
        }
    }
    if scorched {
        soot(&mut c);
    }
    c
}

fn patch() -> Canvas {
    let mut c = Canvas::new(44, 100, [16, 17, 22]);
    rack_frame(&mut c);
    for panel in 0..4 {
        let y = 3 + panel * 9;
        c.rect(3, y, 38, 8, [34, 36, 46]);
        for row in 0..2 {
            for port in 0..12 {
                let lit = hash(port as u32 + panel as u32 * 12, row as u32) > 0.7;
                c.rect(
                    5 + port * 3,
                    y + 1 + row * 3,
                    2,
                    2,
                    if lit { GREEN } else { [8, 9, 12] },
                );
            }
        }
    }
    // Every port has a cable and none of them go where the label says.
    for i in 0..22u32 {
        let x0 = 5.0 + i as f32 * 1.6;
        let sway = (hash(i, 9) - 0.5) * 16.0;
        let col = CABLES[(hash(i, 4) * 6.0) as usize % 6];
        c.line_a((x0, 40.0), (x0 + sway, 70.0), col, 255, 1);
        c.line_a((x0 + sway, 70.0), (x0 + sway * 0.4, 97.0), col, 255, 1);
    }
    c
}

fn drives() -> Canvas {
    let mut c = Canvas::new(44, 100, [16, 17, 22]);
    rack_frame(&mut c);
    for shelf in 0..6 {
        let y = 3 + shelf * 15;
        c.rect(3, y, 38, 13, [30, 32, 40]);
        for row in 0..2 {
            for col in 0..6 {
                let k = hash(col as u32 + shelf as u32 * 6, row as u32 + 2);
                let x = 4 + col * 6;
                let yy = y + 1 + row * 6;
                // A pulled drive leaves a black slot; an amber bay is rebuilding.
                let face = if k < 0.1 { [6, 6, 8] } else { [52, 54, 64] };
                c.rect(x, yy, 5, 5, face);
                if k > 0.1 {
                    c.rect(x + 1, yy + 4, 1, 1, if k > 0.85 { AMBER } else { GREEN });
                }
            }
        }
    }
    c
}

fn back(hot: bool) -> Canvas {
    let mut c = Canvas::new(44, 100, [12, 12, 16]);
    rack_frame(&mut c);
    for u in 0..6 {
        let y = 3 + u * 15;
        c.rect(3, y, 38, 11, [26, 27, 34]);
        for psu in 0..2 {
            let x = 5 + psu * 18;
            c.rect(x, y + 1, 15, 9, [44, 46, 56]);
            c.disc(x as f32 + 5.0, y as f32 + 5.0, 3.5, [14, 14, 18]);
            c.disc(x as f32 + 5.0, y as f32 + 5.0, 1.2, [60, 62, 72]);
            c.rect(x + 10, y + 3, 4, 4, [20, 20, 26]);
            led(&mut c, x + 11, y + 2, hash(u as u32, psu as u32), hot);
        }
    }
    for i in 0..16u32 {
        let x0 = 4.0 + hash(i, 1) * 36.0;
        let col = CABLES[(hash(i, 6) * 6.0) as usize % 6];
        let (x1, x2) = (hash(i, 2) * 40.0 + 2.0, hash(i, 8) * 40.0 + 2.0);
        c.line_a((x0, 8.0 + hash(i, 3) * 40.0), (x1, 60.0), col, 255, 1);
        c.line_a((x1, 60.0), (x2, 98.0), col, 255, 1);
    }
    if hot {
        soot(&mut c);
    }
    c
}

/// Heat damage: a darker top, blotchy char and a warm tint in the lower half.
fn soot(c: &mut Canvas) {
    let h = c.h as f32;
    for y in 0..c.h {
        for x in 0..c.w {
            let k = (1.0 - 0.6 * (1.0 - y as f32 / h).powi(2)) * (0.72 + 0.28 * hash(x / 3, y / 3));
            let px = &mut c.px[(y * c.w + x) as usize];
            px[0] = (px[0] as f32 * k * 1.12).min(255.0) as u8;
            px[1] = (px[1] as f32 * k * 0.92) as u8;
            px[2] = (px[2] as f32 * k * 0.8) as u8;
        }
    }
}

const NOC_BG: Rgb = [34, 5, 7];
const NOC_LINE: Rgb = [255, 72, 60];
const NOC_TEXT: Rgb = [255, 150, 130];
const NOC_DIM: Rgb = [92, 14, 16];

fn noc_base(title: &str) -> Canvas {
    let mut c = Canvas::new(128, 72, NOC_BG);
    c.frame(0, 0, 128, 72, 2, [150, 24, 24]);
    c.rect(2, 2, 124, 10, [86, 12, 14]);
    c.text(5, 3, 1, title, NOC_TEXT);
    c
}

fn noc_graph() -> Canvas {
    let mut c = noc_base("P99 LATENCY: LOL");
    for i in 1..5 {
        c.rect(4, 12 + i * 11, 120, 1, NOC_DIM);
    }
    let mut prev = (6.0, 62.0);
    for i in 1..14u32 {
        let x = 6.0 + i as f32 * 9.0;
        let y = (62.0 - i as f32 * 3.6 - hash(i, 11) * 9.0).max(16.0);
        c.line(prev, (x, y), NOC_LINE);
        prev = (x, y);
    }
    c.text(70, 58, 1, "9999 MS", [255, 210, 200]);
    c
}

fn noc_alerts() -> Canvas {
    let mut c = noc_base("ACTIVE ALERTS: 4096");
    let lines = [
        "CRIT: EVERYTHING",
        "CRIT: DISK 100%",
        "CRIT: CPU 100%",
        "CRIT: VIBES LOW",
        "CRIT: SEE ABOVE",
        "PAGE: ON-CALL (YOU)",
    ];
    for (i, l) in lines.iter().enumerate() {
        let y = 14 + i as i32 * 9;
        if i % 2 == 0 {
            c.rect(2, y - 1, 124, 9, [52, 8, 10]);
        }
        c.rect(4, y + 1, 4, 4, NOC_LINE);
        c.text(11, y, 1, l, NOC_TEXT);
    }
    c
}

fn noc_regions() -> Canvas {
    let mut c = noc_base("ALL REGIONS: DOWN");
    let codes = [
        "USE", "USW", "EU", "APA", "LAT", "MEA", "CA", "UK", "DE", "FR", "IN", "JP", "BR", "AU",
        "ZA",
    ];
    for (i, code) in codes.iter().enumerate() {
        let (cx, cy) = (i as i32 % 5, i as i32 / 5);
        let (x, y) = (4 + cx * 24, 15 + cy * 19);
        let k = hash(i as u32, 21);
        c.rect(
            x,
            y,
            22,
            17,
            if k > 0.5 {
                [150, 20, 22]
            } else {
                [100, 14, 16]
            },
        );
        c.text(x + 2, y + 5, 1, code, [255, 214, 204]);
    }
    c
}

pub(super) fn sign_spec(s: Sign) -> SignSpec {
    let sp = |lines, bg, fg, hi| SignSpec { lines, bg, fg, hi };
    match s {
        Sign::AiTraining => sp(
            &["AI TRAINING", "IN PROGRESS", "PLEASE DO NOT BREATHE"],
            [10, 10, 14],
            [255, 90, 70],
            [255, 210, 90],
        ),
        Sign::Kubernetes => sp(
            &["KUBERNETES", "(WE THINK)"],
            [34, 76, 170],
            [235, 242, 255],
            [255, 214, 90],
        ),
        Sign::Mainframe => sp(
            &["LEGACY MAINFRAME", "DO NOT TOUCH", "(NOBODY KNOWS HOW)"],
            [214, 204, 176],
            [60, 52, 40],
            [180, 40, 30],
        ),
        Sign::PowerStrip => sp(
            &["DO NOT UNPLUG", "NOBODY KNOWS", "WHAT THIS RUNS"],
            [250, 230, 60],
            [20, 20, 20],
            [190, 30, 30],
        ),
        Sign::Rack42 => sp(
            &["RACK 42", "PURPOSE: UNKNOWN", "OWNER: LEFT IN 2019"],
            [30, 34, 42],
            [210, 218, 232],
            [255, 190, 70],
        ),
        Sign::NoFood => sp(
            &["NO FOOD OR DRINK", "EXCEPT ENERGY DRINKS"],
            [236, 232, 220],
            [50, 50, 60],
            [200, 50, 40],
        ),
        Sign::RackFire => sp(
            &["RACK 17 IS ON FIRE", "KNOWN ISSUE", "TICKET OPEN 3 YEARS"],
            [40, 8, 6],
            [255, 190, 110],
            [255, 70, 50],
        ),
        Sign::Escort => sp(
            &[
                "VISITORS MUST BE",
                "ESCORTED BY SOMEONE",
                "WHO KNOWS THE WAY",
            ],
            [226, 228, 232],
            [36, 46, 66],
            [190, 60, 60],
        ),
        _ => sp(&["?"], [0, 0, 0], [255, 255, 255], [255, 255, 255]),
    }
}
