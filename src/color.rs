use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const BLACK: Self = Self::new(0, 0, 0);
    pub const WHITE: Self = Self::new(255, 255, 255);
    pub const RED: Self = Self::new(255, 0, 0);
    pub const GREEN: Self = Self::new(0, 255, 0);
    pub const BLUE: Self = Self::new(0, 0, 255);
    pub const LEGION_CYAN: Self = Self::new(0, 210, 255);
    pub const ICE_BLUE: Self = Self::new(0, 180, 255);
    pub const MAGENTA: Self = Self::new(255, 0, 255);
    pub const YELLOW: Self = Self::new(255, 255, 0);
    pub const ORANGE: Self = Self::new(255, 128, 0);
    pub const PURPLE: Self = Self::new(147, 0, 211);
    pub const PINK: Self = Self::new(255, 105, 180);
    pub const MINT: Self = Self::new(64, 255, 170);
    pub const GOLD: Self = Self::new(255, 191, 0);
    pub const ACID_GREEN: Self = Self::new(176, 255, 0);
    pub const CRIMSON: Self = Self::new(220, 20, 60);

    pub fn to_hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let r = (self.r as f32 + (other.r as f32 - self.r as f32) * t).round() as u8;
        let g = (self.g as f32 + (other.g as f32 - self.g as f32) * t).round() as u8;
        let b = (self.b as f32 + (other.b as f32 - self.b as f32) * t).round() as u8;
        Self { r, g, b }
    }

    pub fn scale(self, factor: f32) -> Self {
        let factor = factor.clamp(0.0, 1.0);
        Self {
            r: (self.r as f32 * factor).round() as u8,
            g: (self.g as f32 * factor).round() as u8,
            b: (self.b as f32 * factor).round() as u8,
        }
    }

    /// Convert HSV (h in 0..360, s in 0..1, v in 0..1) to RGB Color
    pub fn from_hsv(h: f32, s: f32, v: f32) -> Self {
        let h = ((h % 360.0) + 360.0) % 360.0;
        let s = s.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);

        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;

        let (r1, g1, b1) = match (h / 60.0) as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };

        Self {
            r: ((r1 + m) * 255.0).round() as u8,
            g: ((g1 + m) * 255.0).round() as u8,
            b: ((b1 + m) * 255.0).round() as u8,
        }
    }

    /// Convert RGB to HSV (h in 0..360, s in 0..1, v in 0..1)
    pub fn to_hsv(&self) -> (f32, f32, f32) {
        let r = self.r as f32 / 255.0;
        let g = self.g as f32 / 255.0;
        let b = self.b as f32 / 255.0;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;

        let h = if delta == 0.0 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / delta) % 6.0)
        } else if max == g {
            60.0 * (((b - r) / delta) + 2.0)
        } else {
            60.0 * (((r - g) / delta) + 4.0)
        };
        let h = if h < 0.0 { h + 360.0 } else { h };

        let s = if max == 0.0 { 0.0 } else { delta / max };
        let v = max;

        (h, s, v)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl FromStr for Color {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim().to_lowercase();
        match trimmed.as_str() {
            "off" | "black" | "none" => return Ok(Self::BLACK),
            "white" => return Ok(Self::WHITE),
            "warm-white" | "warmwhite" => return Ok(Self::new(255, 230, 180)),
            "red" => return Ok(Self::RED),
            "green" => return Ok(Self::GREEN),
            "blue" => return Ok(Self::BLUE),
            "cyan" | "legion-cyan" | "legion" | "vantage" => return Ok(Self::LEGION_CYAN),
            "ice-blue" | "iceblue" | "ice" => return Ok(Self::ICE_BLUE),
            "magenta" | "fuchsia" => return Ok(Self::MAGENTA),
            "yellow" => return Ok(Self::YELLOW),
            "orange" => return Ok(Self::ORANGE),
            "purple" | "violet" => return Ok(Self::PURPLE),
            "pink" => return Ok(Self::PINK),
            "mint" => return Ok(Self::MINT),
            "gold" | "amber" => return Ok(Self::GOLD),
            "acid-green" | "lime" => return Ok(Self::ACID_GREEN),
            "crimson" => return Ok(Self::CRIMSON),
            _ => {}
        }

        // Comma-separated: "r,g,b"
        if trimmed.contains(',') {
            let parts: Vec<&str> = trimmed.split(',').collect();
            if parts.len() == 3 {
                let r = parts[0].trim().parse::<u8>().map_err(|_| "Invalid red value (0..255)")?;
                let g = parts[1].trim().parse::<u8>().map_err(|_| "Invalid green value (0..255)")?;
                let b = parts[2].trim().parse::<u8>().map_err(|_| "Invalid blue value (0..255)")?;
                return Ok(Self::new(r, g, b));
            }
        }

        // Hex string: "#RRGGBB", "RRGGBB", "#RGB", "RGB"
        let hex = trimmed.strip_prefix('#').unwrap_or(&trimmed);
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid hex red")?;
            let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid hex green")?;
            let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid hex blue")?;
            return Ok(Self::new(r, g, b));
        } else if hex.len() == 3 {
            let r_ch = &hex[0..1];
            let g_ch = &hex[1..2];
            let b_ch = &hex[2..3];
            let r = u8::from_str_radix(&format!("{}{}", r_ch, r_ch), 16).map_err(|_| "Invalid hex")?;
            let g = u8::from_str_radix(&format!("{}{}", g_ch, g_ch), 16).map_err(|_| "Invalid hex")?;
            let b = u8::from_str_radix(&format!("{}{}", b_ch, b_ch), 16).map_err(|_| "Invalid hex")?;
            return Ok(Self::new(r, g, b));
        }

        Err(format!(
            "Could not parse color '{}'. Use '#RRGGBB', 'R,G,B', or a name (red, blue, cyan, etc.)",
            s
        ))
    }
}

impl Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Color::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_parsing() {
        assert_eq!(Color::from_str("#ff0000").unwrap(), Color::RED);
        assert_eq!(Color::from_str("00ff00").unwrap(), Color::GREEN);
        assert_eq!(Color::from_str("#00f").unwrap(), Color::BLUE);
        assert_eq!(Color::from_str("cyan").unwrap(), Color::LEGION_CYAN);
        assert_eq!(Color::from_str("255, 128, 0").unwrap(), Color::ORANGE);
    }

    #[test]
    fn test_hsv_roundtrip() {
        let original = Color::new(255, 0, 128);
        let (h, s, v) = original.to_hsv();
        let reconstructed = Color::from_hsv(h, s, v);
        assert!((original.r as i16 - reconstructed.r as i16).abs() <= 1);
        assert!((original.g as i16 - reconstructed.g as i16).abs() <= 1);
        assert!((original.b as i16 - reconstructed.b as i16).abs() <= 1);
    }
}
