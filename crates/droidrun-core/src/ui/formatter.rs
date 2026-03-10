/// Tree formatting — converts filtered a11y tree to indexed elements + text.
use serde_json::Value;

use crate::ui::coord::bounds_to_normalized;
use crate::ui::state::{Element, PhoneState};

/// Trait for formatting filtered trees.
pub trait TreeFormatter: Send + Sync {
    /// Format filtered tree to standard output format.
    ///
    /// Returns (formatted_text, focused_text, elements, phone_state).
    fn format(
        &self,
        filtered_tree: Option<&Value>,
        phone_state: &Value,
        screen_width: i32,
        screen_height: i32,
        use_normalized: bool,
    ) -> (String, String, Vec<Element>, PhoneState);
}

/// Standard DroidRun indexed formatter.
pub struct IndexedFormatter;

impl TreeFormatter for IndexedFormatter {
    fn format(
        &self,
        filtered_tree: Option<&Value>,
        phone_state: &Value,
        screen_width: i32,
        screen_height: i32,
        use_normalized: bool,
    ) -> (String, String, Vec<Element>, PhoneState) {
        let focused_text = get_focused_text(phone_state);
        let parsed_phone_state = parse_phone_state(phone_state);

        let elements = match filtered_tree {
            Some(tree) => {
                let mut counter = 1usize;
                flatten_with_index(tree, &mut counter, screen_width, screen_height, use_normalized)
            }
            None => vec![],
        };

        let phone_state_text = format_phone_state(&parsed_phone_state);
        let ui_elements_text =
            format_ui_elements_text(&elements, use_normalized);

        let formatted = format!("{phone_state_text}\n\n{ui_elements_text}");
        (formatted, focused_text, elements, parsed_phone_state)
    }
}

// ── Internal functions ──────────────────────────────────────────

fn get_focused_text(phone_state: &Value) -> String {
    phone_state
        .get("focusedElement")
        .and_then(|fe| fe.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string()
}

fn parse_phone_state(raw: &Value) -> PhoneState {
    serde_json::from_value(raw.clone()).unwrap_or_default()
}

fn format_phone_state(ps: &PhoneState) -> String {
    let focused_desc = ps
        .focused_element
        .as_ref()
        .and_then(|fe| fe.get("text"))
        .and_then(|t| t.as_str())
        .map(|t| format!("'{t}'"))
        .unwrap_or_else(|| "''".into());

    let keyboard = if ps.is_editable {
        "Visible"
    } else {
        "Hidden"
    };

    format!(
        "**Current Phone State:**\n\
         • **App:** {} ({})\n\
         • **Keyboard:** {}\n\
         • **Focused Element:** {}",
        ps.current_app, ps.package_name, keyboard, focused_desc
    )
}

fn format_ui_elements_text(elements: &[Element], use_normalized: bool) -> String {
    let coord_note = if use_normalized {
        " (normalized [0-1000])"
    } else {
        ""
    };
    let schema = "'index. className: resourceId; checkedState, text - bounds(x1,y1,x2,y2)'";

    if elements.is_empty() {
        return format!(
            "Current Clickable UI elements{coord_note}:\n{schema}:\nNo UI elements found"
        );
    }

    let formatted = format_elements(elements, 0);
    format!("Current Clickable UI elements{coord_note}:\n{schema}:\n{formatted}")
}

fn format_elements(elements: &[Element], level: usize) -> String {
    let indent = "  ".repeat(level);
    let mut lines = Vec::new();

    for el in elements {
        let mut parts = Vec::new();

        parts.push(format!("{}.", el.index));

        if !el.class_name.is_empty() {
            parts.push(format!("{}:", el.class_name));
        }

        let mut details = Vec::new();
        if !el.resource_id.is_empty() {
            details.push(format!("\"{}\"", el.resource_id));
        }
        if !el.text.is_empty() {
            details.push(format!("\"{}\"", el.text));
        }
        if !details.is_empty() {
            parts.push(details.join(", "));
        }

        if !el.checked_state.is_empty() {
            parts.push(format!("; {}", el.checked_state));
        }

        if !el.bounds.is_empty() {
            parts.push(format!("- ({})", el.bounds));
        }

        lines.push(format!("{indent}{}", parts.join(" ")));

        if !el.children.is_empty() {
            lines.push(format_elements(&el.children, level + 1));
        }
    }

    lines.join("\n")
}

fn flatten_with_index(
    node: &Value,
    counter: &mut usize,
    screen_width: i32,
    screen_height: i32,
    use_normalized: bool,
) -> Vec<Element> {
    let mut results = Vec::new();

    let element = format_node(node, *counter, screen_width, screen_height, use_normalized);
    *counter += 1;
    results.push(element);

    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for child in children {
            results.extend(flatten_with_index(
                child,
                counter,
                screen_width,
                screen_height,
                use_normalized,
            ));
        }
    }

    results
}

fn format_node(
    node: &Value,
    index: usize,
    screen_width: i32,
    screen_height: i32,
    use_normalized: bool,
) -> Element {
    let bounds = node.get("boundsInScreen").cloned().unwrap_or_default();
    let left = bounds.get("left").and_then(|v| v.as_i64()).unwrap_or(0);
    let top = bounds.get("top").and_then(|v| v.as_i64()).unwrap_or(0);
    let right = bounds.get("right").and_then(|v| v.as_i64()).unwrap_or(0);
    let bottom = bounds.get("bottom").and_then(|v| v.as_i64()).unwrap_or(0);

    let bounds_str = format!("{left},{top},{right},{bottom}");
    let bounds_str = if use_normalized && screen_width > 0 && screen_height > 0 {
        bounds_to_normalized(&bounds_str, screen_width, screen_height).unwrap_or(bounds_str)
    } else {
        bounds_str
    };

    let text = node
        .get("text")
        .or_else(|| node.get("contentDescription"))
        .or_else(|| node.get("resourceId"))
        .or_else(|| node.get("className"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let class_name = node
        .get("className")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let short_class = class_name.rsplit('.').next().unwrap_or(class_name);

    let checked_state = if node
        .get("isCheckable")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        if node
            .get("isChecked")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            "isChecked=True".to_string()
        } else {
            "isChecked=False".to_string()
        }
    } else {
        String::new()
    };

    Element {
        index,
        resource_id: node
            .get("resourceId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        class_name: short_class.to_string(),
        checked_state,
        text,
        bounds: bounds_str,
        children: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_format_phone_state() {
        let ps = PhoneState {
            current_app: "Settings".into(),
            package_name: "com.android.settings".into(),
            is_editable: false,
            focused_element: None,
        };
        let text = format_phone_state(&ps);
        assert!(text.contains("Settings"));
        assert!(text.contains("Hidden"));
    }

    #[test]
    fn test_format_phone_state_with_keyboard() {
        let ps = PhoneState {
            current_app: "Chrome".into(),
            package_name: "com.android.chrome".into(),
            is_editable: true,
            focused_element: Some(json!({"text": "Search"})),
        };
        let text = format_phone_state(&ps);
        assert!(text.contains("Visible"));
        assert!(text.contains("'Search'"));
    }

    #[test]
    fn test_flatten_with_index() {
        let tree = json!({
            "className": "android.widget.FrameLayout",
            "boundsInScreen": {"left": 0, "top": 0, "right": 1080, "bottom": 2400},
            "children": [
                {
                    "className": "android.widget.Button",
                    "text": "OK",
                    "boundsInScreen": {"left": 100, "top": 200, "right": 300, "bottom": 400},
                    "children": []
                }
            ]
        });

        let mut counter = 1;
        let elements = flatten_with_index(&tree, &mut counter, 1080, 2400, false);
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].index, 1);
        assert_eq!(elements[0].class_name, "FrameLayout");
        assert_eq!(elements[1].index, 2);
        assert_eq!(elements[1].text, "OK");
    }

    #[test]
    fn test_format_node_normalized() {
        let node = json!({
            "className": "android.widget.Button",
            "text": "Submit",
            "boundsInScreen": {"left": 0, "top": 0, "right": 1080, "bottom": 2400}
        });
        let el = format_node(&node, 1, 1080, 2400, true);
        assert_eq!(el.bounds, "0,0,1000,1000");
    }
}
