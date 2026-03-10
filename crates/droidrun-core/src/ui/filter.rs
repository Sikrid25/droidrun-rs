/// Accessibility tree filtering.
use serde_json::Value;

/// Trait for filtering accessibility trees.
pub trait TreeFilter: Send + Sync {
    /// Filter tree and return filtered tree with hierarchy preserved.
    fn filter(&self, a11y_tree: &Value, device_context: &Value) -> Option<Value>;

    /// Filter name for identification.
    fn name(&self) -> &str;
}

/// Concise tree filtering — removes off-screen and tiny elements.
pub struct ConciseFilter;

impl TreeFilter for ConciseFilter {
    fn filter(&self, a11y_tree: &Value, device_context: &Value) -> Option<Value> {
        let screen_bounds = device_context
            .get("screen_bounds")
            .cloned()
            .unwrap_or_default();
        let filtering_params = device_context
            .get("filtering_params")
            .cloned()
            .unwrap_or_default();

        filter_node(a11y_tree, &screen_bounds, &filtering_params)
    }

    fn name(&self) -> &str {
        "concise"
    }
}

fn filter_node(node: &Value, screen_bounds: &Value, filtering_params: &Value) -> Option<Value> {
    let min_size = filtering_params
        .get("min_element_size")
        .and_then(|v| v.as_i64())
        .unwrap_or(5) as i32;
    let screen_width = screen_bounds
        .get("width")
        .and_then(|v| v.as_i64())
        .unwrap_or(1080) as i32;
    let screen_height = screen_bounds
        .get("height")
        .and_then(|v| v.as_i64())
        .unwrap_or(2400) as i32;

    if !intersects_screen(node, screen_width, screen_height) {
        return None;
    }
    if !meets_min_size(node, min_size) {
        return None;
    }

    let filtered_children: Vec<Value> = node
        .get("children")
        .and_then(|c| c.as_array())
        .map(|children| {
            children
                .iter()
                .filter_map(|child| filter_node(child, screen_bounds, filtering_params))
                .collect()
        })
        .unwrap_or_default();

    let mut result = node.clone();
    if let Some(obj) = result.as_object_mut() {
        obj.insert("children".into(), Value::Array(filtered_children));
    }

    Some(result)
}

fn intersects_screen(node: &Value, screen_width: i32, screen_height: i32) -> bool {
    let bounds = match node.get("boundsInScreen") {
        Some(b) => b,
        None => return true, // no bounds = include
    };

    let left = bounds.get("left").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let top = bounds.get("top").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let right = bounds.get("right").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let bottom = bounds.get("bottom").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

    !(right <= 0 || bottom <= 0 || left >= screen_width || top >= screen_height)
}

fn meets_min_size(node: &Value, min_size: i32) -> bool {
    let bounds = match node.get("boundsInScreen") {
        Some(b) => b,
        None => return true,
    };

    let left = bounds.get("left").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let top = bounds.get("top").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let right = bounds.get("right").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let bottom = bounds.get("bottom").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

    let w = right - left;
    let h = bottom - top;

    w > min_size && h > min_size
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_concise_filter_keeps_visible() {
        let tree = json!({
            "boundsInScreen": {"left": 0, "top": 0, "right": 100, "bottom": 100},
            "children": []
        });
        let ctx = json!({"screen_bounds": {"width": 1080, "height": 2400}});
        let filter = ConciseFilter;
        assert!(filter.filter(&tree, &ctx).is_some());
    }

    #[test]
    fn test_concise_filter_removes_offscreen() {
        let tree = json!({
            "boundsInScreen": {"left": -200, "top": -200, "right": -100, "bottom": -100},
            "children": []
        });
        let ctx = json!({"screen_bounds": {"width": 1080, "height": 2400}});
        let filter = ConciseFilter;
        assert!(filter.filter(&tree, &ctx).is_none());
    }

    #[test]
    fn test_concise_filter_removes_tiny() {
        let tree = json!({
            "boundsInScreen": {"left": 0, "top": 0, "right": 3, "bottom": 3},
            "children": []
        });
        let ctx = json!({"screen_bounds": {"width": 1080, "height": 2400}});
        let filter = ConciseFilter;
        assert!(filter.filter(&tree, &ctx).is_none());
    }

    #[test]
    fn test_concise_filter_preserves_hierarchy() {
        let tree = json!({
            "boundsInScreen": {"left": 0, "top": 0, "right": 500, "bottom": 500},
            "children": [
                {
                    "boundsInScreen": {"left": 10, "top": 10, "right": 200, "bottom": 200},
                    "children": []
                },
                {
                    "boundsInScreen": {"left": -100, "top": -100, "right": -50, "bottom": -50},
                    "children": []
                }
            ]
        });
        let ctx = json!({"screen_bounds": {"width": 1080, "height": 2400}});
        let filter = ConciseFilter;
        let result = filter.filter(&tree, &ctx).unwrap();
        let children = result.get("children").unwrap().as_array().unwrap();
        assert_eq!(children.len(), 1); // Only visible child kept
    }
}
