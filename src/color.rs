use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Color(pub [f64; 3]);
impl Color {
    pub fn parse(s: &str) -> Result<Self> {
        let h = s.strip_prefix('#').unwrap_or("");
        if h.len() != 6 || !h.is_ascii() {
            bail!("unsupported solid color: {s}")
        }
        let mut c = [0.; 3];
        for (i, v) in c.iter_mut().enumerate() {
            *v = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16)? as f64 / 255.;
        }
        Ok(Self(c))
    }
    pub fn over(self, alpha: f64, background: Self) -> Self {
        Self(std::array::from_fn(|i| {
            self.0[i] * alpha + background.0[i] * (1. - alpha)
        }))
    }
    pub fn hex(self) -> String {
        format!(
            "#{:02x}{:02x}{:02x}",
            (self.0[0] * 255.).round() as u8,
            (self.0[1] * 255.).round() as u8,
            (self.0[2] * 255.).round() as u8
        )
    }
    pub fn luminance(self) -> f64 {
        let c = self.0.map(linear);
        c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722
    }
    pub fn contrast(self, other: Self) -> f64 {
        let a = self.luminance();
        let b = other.luminance();
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
    pub fn lab(self) -> [f64; 3] {
        let [r, g, b] = self.0.map(linear);
        let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
        let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
        let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
        [
            0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
            1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
            0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
        ]
    }
    pub fn distance(self, other: Self) -> f64 {
        self.lab()
            .iter()
            .zip(other.lab())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
    }
}
fn linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contrast_vectors() {
        let b = Color::parse("#000000").unwrap();
        let w = Color::parse("#ffffff").unwrap();
        assert!((b.contrast(w) - 21.).abs() < 1e-12);
        assert_eq!(b.contrast(b), 1.);
        assert!((w.lab()[0] - 1.).abs() < 1e-7);
        assert!(w.lab()[1].abs() < 1e-7);
    }
    #[test]
    fn composition() {
        let b = Color::parse("#000000").unwrap();
        let w = Color::parse("#ffffff").unwrap();
        assert_eq!(w.over(0.5, b).hex(), "#808080");
        assert_eq!(w.over(0., b).hex(), "#000000");
    }
    #[test]
    fn oklab_red_reference() {
        let actual = Color::parse("#ff0000").unwrap().lab();
        let expected = [0.6279553606145516, 0.22486306106597398, 0.1258462985307351];
        for (a, e) in actual.into_iter().zip(expected) {
            assert!((a - e).abs() < 1e-9);
        }
    }
    #[test]
    fn fractional_composition_is_not_hex_rounded() {
        let white = Color::parse("#ffffff").unwrap();
        let half = white.over(0.5, Color::parse("#000000").unwrap());
        assert!((white.contrast(half) - 3.976653024912438).abs() < 1e-9);
        assert!(
            (white.contrast(Color::parse(&half.hex()).unwrap()) - white.contrast(half)).abs()
                > 0.01
        );
    }
    #[test]
    fn parsing_bounds() {
        for bad in ["red", "#fff", "#12345é", "#gg0000", "#000000ff"] {
            assert!(Color::parse(bad).is_err());
        }
    }
}
