/// Geometry utilities for UI element bounds and tap point calculation.

/// A rectangle defined by (left, top, right, bottom).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Bounds {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// Parse from "left,top,right,bottom" string.
    pub fn from_str(s: &str) -> Option<Self> {
        let parts: Vec<i32> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        if parts.len() == 4 {
            Some(Self::new(parts[0], parts[1], parts[2], parts[3]))
        } else {
            None
        }
    }

    /// Center point of the bounds.
    pub fn center(&self) -> (i32, i32) {
        ((self.left + self.right) / 2, (self.top + self.bottom) / 2)
    }

    /// Width of the bounds.
    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    /// Height of the bounds.
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }

    /// Area of the bounds.
    pub fn area(&self) -> i32 {
        self.width() * self.height()
    }

    /// Check if two rectangles overlap.
    pub fn overlaps(&self, other: &Bounds) -> bool {
        !(self.right <= other.left
            || other.right <= self.left
            || self.bottom <= other.top
            || other.bottom <= self.top)
    }

    /// Check if a point is inside the bounds.
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    /// Convert to "left,top,right,bottom" string.
    pub fn to_string(&self) -> String {
        format!("{},{},{},{}", self.left, self.top, self.right, self.bottom)
    }
}

/// Find a clear tap point within bounds, avoiding blocker rectangles.
///
/// Uses quadrant subdivision (up to depth 4) to find an unblocked center point.
pub fn find_clear_point(bounds: &Bounds, blockers: &[Bounds]) -> Option<(i32, i32)> {
    find_clear_point_recursive(bounds, blockers, 0)
}

fn find_clear_point_recursive(bounds: &Bounds, blockers: &[Bounds], depth: u32) -> Option<(i32, i32)> {
    let (cx, cy) = bounds.center();

    // Check if center is blocked
    let blocked = blockers.iter().any(|b| b.contains_point(cx, cy));

    if !blocked {
        return Some((cx, cy));
    }

    // Max recursion depth or too-small area
    if depth > 4 || bounds.area() < 100 {
        return None;
    }

    // Try quadrants
    let quadrants = [
        Bounds::new(bounds.left, bounds.top, cx, cy),
        Bounds::new(cx, bounds.top, bounds.right, cy),
        Bounds::new(bounds.left, cy, cx, bounds.bottom),
        Bounds::new(cx, cy, bounds.right, bounds.bottom),
    ];

    let mut best_point = None;
    let mut best_area = 0;

    for q in &quadrants {
        let area = q.area();
        if area <= 0 {
            continue;
        }
        if let Some(point) = find_clear_point_recursive(q, blockers, depth + 1) {
            if area > best_area {
                best_point = Some(point);
                best_area = area;
            }
        }
    }

    best_point
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounds_center() {
        let b = Bounds::new(0, 0, 100, 200);
        assert_eq!(b.center(), (50, 100));
    }

    #[test]
    fn test_bounds_from_str() {
        let b = Bounds::from_str("10,20,30,40").unwrap();
        assert_eq!(b, Bounds::new(10, 20, 30, 40));
    }

    #[test]
    fn test_bounds_from_str_invalid() {
        assert!(Bounds::from_str("10,20").is_none());
        assert!(Bounds::from_str("abc").is_none());
    }

    #[test]
    fn test_overlaps() {
        let a = Bounds::new(0, 0, 100, 100);
        let b = Bounds::new(50, 50, 150, 150);
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
    }

    #[test]
    fn test_no_overlap() {
        let a = Bounds::new(0, 0, 100, 100);
        let b = Bounds::new(100, 100, 200, 200);
        assert!(!a.overlaps(&b));
    }

    #[test]
    fn test_contains_point() {
        let b = Bounds::new(0, 0, 100, 100);
        assert!(b.contains_point(50, 50));
        assert!(!b.contains_point(100, 100));
        assert!(!b.contains_point(-1, 50));
    }

    #[test]
    fn test_find_clear_point_no_blockers() {
        let bounds = Bounds::new(0, 0, 200, 200);
        let point = find_clear_point(&bounds, &[]).unwrap();
        assert_eq!(point, (100, 100));
    }

    #[test]
    fn test_find_clear_point_blocked_center() {
        let bounds = Bounds::new(0, 0, 200, 200);
        let blocker = Bounds::new(90, 90, 110, 110); // blocks center
        let point = find_clear_point(&bounds, &[blocker]).unwrap();
        // Should find a point in one of the quadrants
        assert!(!blocker.contains_point(point.0, point.1));
    }

    #[test]
    fn test_find_clear_point_fully_blocked() {
        let bounds = Bounds::new(0, 0, 10, 10);
        let blocker = Bounds::new(0, 0, 10, 10); // covers entire area
        let point = find_clear_point(&bounds, &[blocker]);
        assert!(point.is_none());
    }

    #[test]
    fn test_bounds_dimensions() {
        let b = Bounds::new(10, 20, 110, 220);
        assert_eq!(b.width(), 100);
        assert_eq!(b.height(), 200);
        assert_eq!(b.area(), 20000);
    }
}
