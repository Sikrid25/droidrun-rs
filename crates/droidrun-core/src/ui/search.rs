/// Composable element search filters for accessibility trees.
///
/// Works with raw a11y tree data (serde_json::Value) from Portal.
use regex::Regex;
use serde_json::Value;

/// A filter function that takes a list of tree nodes and returns matching ones.
pub type ElementFilter = Box<dyn Fn(&[Value]) -> Vec<Value> + Send + Sync>;

/// Flatten a tree node into a list of all descendant nodes.
pub fn flatten_tree(root: &Value) -> Vec<Value> {
    let mut results = vec![root.clone()];
    if let Some(children) = root.get("children").and_then(|c| c.as_array()) {
        for child in children {
            results.extend(flatten_tree(child));
        }
    }
    results
}

/// Get center coordinates from boundsInScreen.
pub fn get_element_center(node: &Value) -> (i32, i32) {
    let bounds = node.get("boundsInScreen").cloned().unwrap_or_default();
    let left = bounds.get("left").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let top = bounds.get("top").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let right = bounds.get("right").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let bottom = bounds.get("bottom").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    ((left + right) / 2, (top + bottom) / 2)
}

// ── Filter constructors ─────────────────────────────────────────

/// Match elements by text content (text, contentDescription, or hint).
pub fn text_matches(pattern: &str) -> ElementFilter {
    let regex = Regex::new(&regex::escape(pattern)).unwrap();
    let pattern_owned = pattern.to_string();

    Box::new(move |nodes: &[Value]| {
        let all: Vec<Value> = nodes.iter().flat_map(flatten_tree).collect();

        all.into_iter()
            .filter(|node| {
                for field in &["text", "contentDescription", "hint"] {
                    if let Some(val) = node.get(field).and_then(|v| v.as_str()) {
                        if val == pattern_owned || regex.is_match(val) {
                            return true;
                        }
                        let normalized = val.replace('\n', " ");
                        if normalized == pattern_owned || regex.is_match(&normalized) {
                            return true;
                        }
                    }
                }
                false
            })
            .collect()
    })
}

/// Match elements by resource ID.
pub fn id_matches(pattern: &str) -> ElementFilter {
    let regex = Regex::new(&regex::escape(pattern)).unwrap();
    let pattern_owned = pattern.to_string();

    Box::new(move |nodes: &[Value]| {
        let all: Vec<Value> = nodes.iter().flat_map(flatten_tree).collect();

        all.into_iter()
            .filter(|node| {
                if let Some(id) = node.get("resourceId").and_then(|v| v.as_str()) {
                    if id == pattern_owned || regex.is_match(id) {
                        return true;
                    }
                    // Short ID (after /)
                    if let Some(short) = id.rsplit('/').next() {
                        if short == pattern_owned || regex.is_match(short) {
                            return true;
                        }
                    }
                }
                false
            })
            .collect()
    })
}

/// Match clickable elements.
pub fn clickable() -> ElementFilter {
    Box::new(|nodes: &[Value]| {
        let all: Vec<Value> = nodes.iter().flat_map(flatten_tree).collect();
        all.into_iter()
            .filter(|n| n.get("isClickable").and_then(|v| v.as_bool()).unwrap_or(false))
            .collect()
    })
}

/// Match elements that have non-empty text.
pub fn has_text() -> ElementFilter {
    Box::new(|nodes: &[Value]| {
        let all: Vec<Value> = nodes.iter().flat_map(flatten_tree).collect();
        all.into_iter()
            .filter(|n| {
                n.get("text")
                    .and_then(|v| v.as_str())
                    .map(|s| !s.is_empty())
                    .unwrap_or(false)
                    || n.get("contentDescription")
                        .and_then(|v| v.as_str())
                        .map(|s| !s.is_empty())
                        .unwrap_or(false)
            })
            .collect()
    })
}

/// Find elements positioned below an anchor.
pub fn below(anchor_filter: ElementFilter) -> ElementFilter {
    Box::new(move |nodes: &[Value]| {
        let anchor_results = anchor_filter(nodes);
        let Some(anchor) = anchor_results.first() else {
            return vec![];
        };

        let (ax, ay) = get_element_center(anchor);
        let anchor_bottom = anchor
            .get("boundsInScreen")
            .and_then(|b| b.get("bottom"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        let all: Vec<Value> = nodes.iter().flat_map(flatten_tree).collect();
        let mut candidates: Vec<(f64, Value)> = all
            .into_iter()
            .filter(|n| n != anchor)
            .filter_map(|n| {
                let top = n
                    .get("boundsInScreen")
                    .and_then(|b| b.get("top"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                if top > anchor_bottom {
                    let (nx, ny) = get_element_center(&n);
                    let dist = (((nx - ax).pow(2) + (ny - ay).pow(2)) as f64).sqrt();
                    Some((dist, n))
                } else {
                    None
                }
            })
            .collect();

        candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        candidates.into_iter().map(|(_, n)| n).collect()
    })
}

/// Compose filters sequentially (pipeline).
pub fn compose(filters: Vec<ElementFilter>) -> ElementFilter {
    Box::new(move |nodes: &[Value]| {
        let mut result: Vec<Value> = nodes.to_vec();
        for f in &filters {
            result = f(&result);
            if result.is_empty() {
                break;
            }
        }
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_tree() -> Vec<Value> {
        vec![json!({
            "text": "Hello World",
            "className": "android.widget.TextView",
            "resourceId": "com.example:id/title",
            "isClickable": false,
            "boundsInScreen": {"left": 0, "top": 0, "right": 500, "bottom": 100},
            "children": [
                {
                    "text": "OK",
                    "className": "android.widget.Button",
                    "resourceId": "com.example:id/btn_ok",
                    "isClickable": true,
                    "boundsInScreen": {"left": 100, "top": 200, "right": 300, "bottom": 300},
                    "children": []
                },
                {
                    "text": "Cancel",
                    "className": "android.widget.Button",
                    "resourceId": "com.example:id/btn_cancel",
                    "isClickable": true,
                    "boundsInScreen": {"left": 400, "top": 200, "right": 600, "bottom": 300},
                    "children": []
                }
            ]
        })]
    }

    #[test]
    fn test_text_matches() {
        let filter = text_matches("OK");
        let results = filter(&sample_tree());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].get("text").unwrap().as_str().unwrap(), "OK");
    }

    #[test]
    fn test_id_matches_short() {
        let filter = id_matches("btn_ok");
        let results = filter(&sample_tree());
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_clickable() {
        let results = clickable()(&sample_tree());
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_has_text() {
        let results = has_text()(&sample_tree());
        assert_eq!(results.len(), 3); // Hello World, OK, Cancel
    }

    #[test]
    fn test_flatten_tree() {
        let flat = flatten_tree(&sample_tree()[0]);
        assert_eq!(flat.len(), 3);
    }
}
