//! Native border stop colors, separate from the strict six-digit palette parser.
use crate::color::Color;

#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub color: Color,
    pub alpha: f64,
}
impl Sample {
    /// Interpolate premultiplied encoded-sRGB gradient stops.
    pub fn mix(self, other: Self, t: f64) -> Self {
        let alpha = self.alpha * (1. - t) + other.alpha * t;
        let color = Color(std::array::from_fn(|i| {
            if alpha == 0. {
                0.
            } else {
                (self.color.0[i] * self.alpha * (1. - t) + other.color.0[i] * other.alpha * t)
                    / alpha
            }
        }));
        Self { color, alpha }
    }
}

/// Match BorderGeometry.canonicalColor's supported spellings and alpha-byte rounding.
/// RGB values are encoded sRGB; eight-digit #hex is RRGGBBAA, not Qt's AARRGGBB.
pub fn parse(token: &str, opacity: f64) -> Option<Sample> {
    let token = token.trim().to_ascii_lowercase();
    if !token.is_ascii() || !opacity.is_finite() || !(0. ..=1.).contains(&opacity) {
        return None;
    }
    if token == "transparent" {
        return Some(Sample {
            color: Color([0.; 3]),
            alpha: 0.,
        });
    }
    let hex = |rgb: &str, a: &str| -> Option<(Color, f64)> {
        Some((
            Color::parse(&format!("#{rgb}")).ok()?,
            u8::from_str_radix(a, 16).ok()? as f64 / 255.,
        ))
    };
    let (color, alpha) = if let Some(h) = token.strip_prefix('#') {
        match h.len() {
            3 => {
                let expanded: String = h.chars().flat_map(|c| [c, c]).collect();
                hex(&expanded, "ff")?
            }
            6 => hex(h, "ff")?,
            8 => hex(&h[..6], &h[6..])?,
            _ => return None,
        }
    } else if let Some(h) = token.strip_prefix("0x") {
        if h.len() != 8 {
            return None;
        }
        hex(&h[2..], &h[..2])?
    } else {
        let (body, has_alpha) = if let Some(s) = token
            .strip_prefix("rgba(")
            .and_then(|s| s.strip_suffix(')'))
        {
            (s, true)
        } else {
            let s = token
                .strip_prefix("rgb(")
                .and_then(|s| s.strip_suffix(')'))?;
            (s, false)
        };
        if body.contains(',') {
            let parts: Vec<_> = body.split(',').collect();
            if parts.len() != if has_alpha { 4 } else { 3 } {
                return None;
            }
            let mut rgb = [0.; 3];
            for (i, value) in rgb.iter_mut().enumerate() {
                if parts[i].is_empty() || !parts[i].bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                let n = parts[i].parse::<f64>().ok()?;
                if !n.is_finite() {
                    return None;
                }
                *value = n.clamp(0., 255.) / 255.;
            }
            let alpha = if has_alpha {
                if parts[3].is_empty() || !parts[3].bytes().all(|b| b.is_ascii_digit() || b == b'.')
                {
                    return None;
                }
                let a = parts[3].parse::<f64>().ok()?;
                if !a.is_finite() {
                    return None;
                }
                a.clamp(0., 1.)
            } else {
                1.
            };
            (Color(rgb), alpha)
        } else {
            if body.len() != if has_alpha { 8 } else { 6 } {
                return None;
            }
            hex(&body[..6], if has_alpha { &body[6..] } else { "ff" })?
        }
    };
    Some(Sample {
        color,
        alpha: (alpha * opacity * 255.).round() / 255.,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_formats_and_alpha_rounding() {
        for token in ["rgba(D83A66ee)", "#d83a66ee", "0xeed83a66"] {
            let s = parse(token, 0.82).unwrap();
            assert_eq!(s.color.hex(), "#d83a66");
            assert!((s.alpha - 195. / 255.).abs() < 1e-12);
        }
        for token in ["#abc", "rgb(aabbcc)", "rgb(170,187,204)"] {
            assert_eq!(parse(token, 1.).unwrap().color.hex(), "#aabbcc");
        }
        assert_eq!(
            parse("rgba(255,0,128,0.5)", 0.82).unwrap().alpha,
            105. / 255.
        );
        for bad in [
            "rgba(zzzzzzff)",
            "rgba(123456)",
            "rgb(123456ff)",
            "rgba(1,2,3,NaN)",
            "#12345é",
            "red",
        ] {
            assert!(parse(bad, 1.).is_none(), "{bad}");
        }
    }
    #[test]
    fn transparent_gradient_interpolates_premultiplied_colors() {
        let red = parse("rgba(ff0000ff)", 1.).unwrap();
        let transparent_blue = parse("rgba(0000ff00)", 1.).unwrap();
        let mid = red.mix(transparent_blue, 0.5);
        assert_eq!(mid.color.hex(), "#ff0000");
        assert_eq!(mid.alpha, 0.5);
        assert_eq!(mid.color.over(mid.alpha, Color([0.; 3])).hex(), "#800000");
    }
}
