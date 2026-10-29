//! sRGB colour with alpha, serialised as `#RRGGBBAA`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// An sRGB colour with straight (not premultiplied) alpha.
///
/// Written as `"#RRGGBBAA"` (upper-case hex); `"#RRGGBB"` is accepted on read and means
/// fully opaque.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

/// Error for a string that is not `#RRGGBB` or `#RRGGBBAA`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseColorError(String);

impl fmt::Display for ParseColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid colour {:?}: expected #RRGGBB or #RRGGBBAA",
            self.0
        )
    }
}

impl std::error::Error for ParseColorError {}

impl FromStr for Color {
    type Err = ParseColorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ParseColorError(s.to_owned());
        let hex = s.strip_prefix('#').ok_or_else(err)?;
        if !(hex.len() == 6 || hex.len() == 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(err());
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| err());
        let a = if hex.len() == 8 { byte(6)? } else { 255 };
        Ok(Self::rgba(byte(0)?, byte(2)?, byte(4)?, a))
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "#{:02X}{:02X}{:02X}{:02X}",
            self.r, self.g, self.b, self.a
        )
    }
}

impl TryFrom<String> for Color {
    type Error = ParseColorError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<Color> for String {
    fn from(c: Color) -> Self {
        c.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_forms() {
        assert_eq!("#FF8017".parse(), Ok(Color::rgb(0xFF, 0x80, 0x17)));
        assert_eq!("#1977ff66".parse(), Ok(Color::rgba(0x19, 0x77, 0xFF, 0x66)));
    }

    #[test]
    fn writes_upper_case_with_alpha() {
        assert_eq!(Color::rgb(0xFC, 0x0C, 0x59).to_string(), "#FC0C59FF");
    }

    #[test]
    fn rejects_malformed() {
        for s in [
            "",
            "FF0000",
            "#FF00",
            "#FF00000",
            "#GG0000",
            "#FF0000FF00",
            "#+F0000",
        ] {
            assert!(s.parse::<Color>().is_err(), "{s:?} should be rejected");
        }
    }
}
