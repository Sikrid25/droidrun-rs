/// Coordinate conversion utilities for normalized [0-1000] coordinates.
use crate::error::{DroidrunError, Result};

/// Maximum value for normalized coordinates.
pub const NORMALIZED_MAX: i32 = 1000;

/// Convert [0-1000] normalized to absolute pixels.
pub fn to_absolute(x: i32, y: i32, width: i32, height: i32) -> Result<(i32, i32)> {
    if width <= 0 || height <= 0 {
        return Err(DroidrunError::NoDimensions);
    }
    Ok((x * width / NORMALIZED_MAX, y * height / NORMALIZED_MAX))
}

/// Convert absolute pixels to [0-1000] normalized.
pub fn to_normalized(x: i32, y: i32, width: i32, height: i32) -> Result<(i32, i32)> {
    if width <= 0 || height <= 0 {
        return Err(DroidrunError::NoDimensions);
    }
    Ok((x * NORMALIZED_MAX / width, y * NORMALIZED_MAX / height))
}

/// Convert "left,top,right,bottom" bounds string to normalized.
pub fn bounds_to_normalized(bounds: &str, width: i32, height: i32) -> Result<String> {
    let parts: Vec<i32> = bounds
        .split(',')
        .map(|s| {
            s.trim()
                .parse()
                .map_err(|_| DroidrunError::InvalidBounds(bounds.into()))
        })
        .collect::<Result<Vec<_>>>()?;

    if parts.len() != 4 {
        return Err(DroidrunError::InvalidBounds(bounds.into()));
    }

    let (nl, nt) = to_normalized(parts[0], parts[1], width, height)?;
    let (nr, nb) = to_normalized(parts[2], parts[3], width, height)?;

    Ok(format!("{nl},{nt},{nr},{nb}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_absolute() {
        let (x, y) = to_absolute(500, 500, 1080, 2400).unwrap();
        assert_eq!(x, 540);
        assert_eq!(y, 1200);
    }

    #[test]
    fn test_to_absolute_zero() {
        let (x, y) = to_absolute(0, 0, 1080, 2400).unwrap();
        assert_eq!(x, 0);
        assert_eq!(y, 0);
    }

    #[test]
    fn test_to_absolute_max() {
        let (x, y) = to_absolute(1000, 1000, 1080, 2400).unwrap();
        assert_eq!(x, 1080);
        assert_eq!(y, 2400);
    }

    #[test]
    fn test_to_normalized() {
        let (x, y) = to_normalized(540, 1200, 1080, 2400).unwrap();
        assert_eq!(x, 500);
        assert_eq!(y, 500);
    }

    #[test]
    fn test_bounds_to_normalized() {
        let result = bounds_to_normalized("0,0,1080,2400", 1080, 2400).unwrap();
        assert_eq!(result, "0,0,1000,1000");
    }

    #[test]
    fn test_invalid_dimensions() {
        assert!(to_absolute(500, 500, 0, 0).is_err());
    }

    #[test]
    fn test_invalid_bounds_format() {
        assert!(bounds_to_normalized("invalid", 1080, 2400).is_err());
    }
}
