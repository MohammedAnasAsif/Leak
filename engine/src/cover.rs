//! Original cover art for every story, generated from the story itself.
//!
//! Each region covering the story contributes one flat geometric shape in that
//! region's colour, sized by how many outlets there cover it, on a tint of the
//! section colour. The layout comes from the story id, so a cover is stable,
//! and it gains shapes as the story spreads to more of the world.

use crate::views::RegionCount;
use std::fmt::Write;

const W: f32 = 1200.0;
const H: f32 = 750.0;
const U: f32 = 150.0; // grid unit: the canvas is 8 × 5 units
const INK: &str = "#14161F";

pub fn region_hex(slug: &str) -> &'static str {
    match slug {
        "africa" => "#1F9D6B",
        "americas" => "#2F5BEA",
        "asia" => "#E23C6E",
        "europe" => "#7A4FE0",
        "middle-east" => "#F08A12",
        "oceania" => "#12A8C0",
        _ => "#8A93A6",
    }
}

fn parse_hex(hex: &str) -> (f32, f32, f32) {
    let h = hex.trim_start_matches('#');
    let v = u32::from_str_radix(h, 16).unwrap_or(0x888888);
    (
        ((v >> 16) & 0xff) as f32,
        ((v >> 8) & 0xff) as f32,
        (v & 0xff) as f32,
    )
}

/// Mix a colour toward white (t > 0) or black (t < 0).
pub fn mix(hex: &str, t: f32) -> String {
    let (r, g, b) = parse_hex(hex);
    let target = if t >= 0.0 { 255.0 } else { 0.0 };
    let t = t.abs();
    let m = |c: f32| (c + (target - c) * t).round() as u8;
    format!("#{:02X}{:02X}{:02X}", m(r), m(g), m(b))
}

struct Rng {
    bytes: [u8; 32],
    i: usize,
    round: u32,
    seed: String,
}

impl Rng {
    fn new(seed: &str) -> Self {
        Rng {
            bytes: *blake3::hash(seed.as_bytes()).as_bytes(),
            i: 0,
            round: 0,
            seed: seed.to_string(),
        }
    }
    fn next(&mut self) -> u32 {
        if self.i >= 32 {
            self.round += 1;
            self.bytes =
                *blake3::hash(format!("{}#{}", self.seed, self.round).as_bytes()).as_bytes();
            self.i = 0;
        }
        let v = self.bytes[self.i] as u32;
        self.i += 1;
        v
    }
    fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            0
        } else {
            self.next() % n
        }
    }
}

pub fn svg(story_id: &str, section_color: &str, regions: &[RegionCount]) -> String {
    let mut rng = Rng::new(story_id);
    let bg = mix(section_color, 0.84);
    let deep = mix(section_color, -0.25);

    // Colours: the section's deep tone anchors, then each region, biggest first.
    let mut colors: Vec<(&str, f32)> = vec![(deep.as_str(), 1.0)];
    let total: usize = regions.iter().map(|r| r.count).sum::<usize>().max(1);
    for r in regions {
        colors.push((region_hex(&r.slug), r.count as f32 / total as f32));
    }
    let n = (regions.len() + 2).clamp(3, 7);

    let mut out = String::with_capacity(2048);
    let _ = write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}"><rect width="{W}" height="{H}" fill="{bg}"/>"#
    );

    for i in 0..n {
        let (color, share) = colors[i % colors.len()];
        // Bigger shapes for the anchor and for regions with more coverage.
        let size_u: u32 = match i {
            0 => 4,
            _ if share >= 0.34 => 4,
            _ if share >= 0.18 || i < 3 => 3,
            _ => 2,
        };
        let s = size_u as f32 * U;
        let x = rng.below(8 - size_u + 1) as f32 * U;
        let y = rng.below(5u32.saturating_sub(size_u) + 1) as f32 * U;
        let (cx, cy) = (x + s / 2.0, y + s / 2.0);
        let rot = rng.below(4) * 90;
        let kind = if i == 0 { rng.below(3) } else { rng.below(5) };
        let t = format!(r#" transform="rotate({rot} {cx} {cy})""#);
        let _ = match kind {
            0 => write!(
                out,
                r#"<path d="M{x} {y} L{} {y} A{s} {s} 0 0 1 {x} {} Z" fill="{color}"{t}/>"#,
                x + s,
                y + s
            ),
            1 => write!(
                out,
                r#"<path d="M{x} {cy} A{} {} 0 0 1 {} {cy} Z" fill="{color}"{t}/>"#,
                s / 2.0,
                s / 2.0,
                x + s
            ),
            2 => write!(
                out,
                r#"<circle cx="{cx}" cy="{cy}" r="{}" fill="{color}"/>"#,
                s / 2.0
            ),
            3 => write!(
                out,
                r#"<rect x="{x}" y="{}" width="{s}" height="{}" fill="{color}"{t}/>"#,
                y + s / 4.0,
                s / 2.0
            ),
            _ => write!(
                out,
                r#"<circle cx="{cx}" cy="{cy}" r="{}" fill="none" stroke="{color}" stroke-width="{}"/>"#,
                s / 2.0 - s / 10.0,
                s / 5.0
            ),
        };
    }

    // A small dot grid in ink: the "wire" detail every cover shares.
    let gx = rng.below(7) as f32 * U + 30.0;
    let gy = rng.below(4) as f32 * U + 30.0;
    for r in 0..4 {
        for c in 0..4 {
            let _ = write!(
                out,
                r#"<circle cx="{}" cy="{}" r="6" fill="{INK}"/>"#,
                gx + c as f32 * 30.0,
                gy + r as f32 * 30.0
            );
        }
    }
    out.push_str("</svg>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_valid() {
        let regions = vec![
            RegionCount {
                name: "Europe".into(),
                slug: "europe".into(),
                count: 3,
            },
            RegionCount {
                name: "Asia".into(),
                slug: "asia".into(),
                count: 1,
            },
        ];
        let a = svg("abc123", "#2457D6", &regions);
        assert_eq!(a, svg("abc123", "#2457D6", &regions));
        assert_ne!(a, svg("def456", "#2457D6", &regions));
        assert!(a.starts_with("<svg") && a.ends_with("</svg>"));
        assert!(a.contains("#7A4FE0"));
    }

    #[test]
    fn mixes() {
        assert_eq!(mix("#000000", 1.0), "#FFFFFF");
        assert_eq!(mix("#FFFFFF", -1.0), "#000000");
    }
}
