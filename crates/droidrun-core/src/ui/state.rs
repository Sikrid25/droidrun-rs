/// UIState — parsed UI elements with element resolution and coordinate conversion.
use serde::{Deserialize, Serialize};

use crate::error::{DroidrunError, Result};
use crate::ui::coord;
use crate::ui::geometry::{find_clear_point, Bounds};

/// A UI element from the accessibility tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    pub index: usize,
    #[serde(default)]
    pub class_name: String,
    #[serde(default)]
    pub resource_id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub bounds: String,
    #[serde(default)]
    pub checked_state: String,
    #[serde(default)]
    pub children: Vec<Element>,
}

/// Phone state information.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PhoneState {
    #[serde(default, rename = "currentApp")]
    pub current_app: String,
    #[serde(default, rename = "packageName")]
    pub package_name: String,
    #[serde(default, rename = "isEditable")]
    pub is_editable: bool,
    #[serde(default, rename = "focusedElement")]
    pub focused_element: Option<serde_json::Value>,
}

/// Screen dimensions.
#[derive(Debug, Clone, Copy)]
pub struct ScreenDimensions {
    pub width: i32,
    pub height: i32,
}

/// A snapshot of the device UI state.
#[derive(Debug, Clone)]
pub struct UIState {
    pub elements: Vec<Element>,
    pub formatted_text: String,
    pub focused_text: String,
    pub phone_state: PhoneState,
    pub screen: ScreenDimensions,
    pub use_normalized: bool,
}

impl UIState {
    pub fn new(
        elements: Vec<Element>,
        formatted_text: String,
        focused_text: String,
        phone_state: PhoneState,
        screen: ScreenDimensions,
        use_normalized: bool,
    ) -> Self {
        Self {
            elements,
            formatted_text,
            focused_text,
            phone_state,
            screen,
            use_normalized,
        }
    }

    /// Find an element by index (recursive tree search).
    pub fn get_element(&self, index: usize) -> Option<&Element> {
        find_by_index(&self.elements, index)
    }

    /// Get the center (x, y) of an element by index.
    pub fn get_element_coords(&self, index: usize) -> Result<(i32, i32)> {
        let el = self
            .get_element(index)
            .ok_or(DroidrunError::ElementNotFound(index))?;

        if el.bounds.is_empty() {
            return Err(DroidrunError::ElementNoBounds(index));
        }

        let bounds = Bounds::from_str(&el.bounds)
            .ok_or_else(|| DroidrunError::InvalidBounds(el.bounds.clone()))?;

        Ok(bounds.center())
    }

    /// Get element info for display.
    pub fn get_element_info(&self, index: usize) -> Option<ElementInfo> {
        let el = self.get_element(index)?;
        Some(ElementInfo {
            text: el.text.clone(),
            class_name: el.class_name.clone(),
            bounds: el.bounds.clone(),
        })
    }

    /// Find a tap point that avoids overlapping elements.
    pub fn get_clear_point(&self, index: usize) -> Result<(i32, i32)> {
        let el = self
            .get_element(index)
            .ok_or(DroidrunError::ElementNotFound(index))?;

        if el.bounds.is_empty() {
            return Err(DroidrunError::ElementNoBounds(index));
        }

        let target = Bounds::from_str(&el.bounds)
            .ok_or_else(|| DroidrunError::InvalidBounds(el.bounds.clone()))?;

        let all_elements = collect_all(&self.elements);
        let blockers: Vec<Bounds> = all_elements
            .iter()
            .filter(|e| e.index > index && !e.bounds.is_empty())
            .filter_map(|e| Bounds::from_str(&e.bounds))
            .filter(|b| target.overlaps(b))
            .collect();

        find_clear_point(&target, &blockers).ok_or(DroidrunError::ElementObscured(index))
    }

    /// Convert point to absolute pixels if normalized mode is active.
    pub fn convert_point(&self, x: i32, y: i32) -> Result<(i32, i32)> {
        if self.use_normalized {
            coord::to_absolute(x, y, self.screen.width, self.screen.height)
        } else {
            Ok((x, y))
        }
    }

    /// Get all element indices (flattened).
    pub fn all_indices(&self) -> Vec<usize> {
        collect_indices(&self.elements)
    }
}

/// Basic element info for display.
#[derive(Debug, Clone)]
pub struct ElementInfo {
    pub text: String,
    pub class_name: String,
    pub bounds: String,
}

// ── Internal helpers ─────────────────────────────────────────────

fn find_by_index(elements: &[Element], target: usize) -> Option<&Element> {
    for el in elements {
        if el.index == target {
            return Some(el);
        }
        if let Some(found) = find_by_index(&el.children, target) {
            return Some(found);
        }
    }
    None
}

fn collect_indices(elements: &[Element]) -> Vec<usize> {
    let mut indices = Vec::new();
    for el in elements {
        indices.push(el.index);
        indices.extend(collect_indices(&el.children));
    }
    indices.sort();
    indices
}

fn collect_all(elements: &[Element]) -> Vec<&Element> {
    let mut result = Vec::new();
    for el in elements {
        result.push(el);
        result.extend(collect_all(&el.children));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_elements() -> Vec<Element> {
        vec![
            Element {
                index: 1,
                class_name: "Button".into(),
                resource_id: "btn_ok".into(),
                text: "OK".into(),
                bounds: "100,200,300,400".into(),
                checked_state: String::new(),
                children: vec![],
            },
            Element {
                index: 2,
                class_name: "TextView".into(),
                resource_id: "".into(),
                text: "Hello".into(),
                bounds: "0,0,1080,100".into(),
                checked_state: String::new(),
                children: vec![Element {
                    index: 3,
                    class_name: "ImageView".into(),
                    resource_id: "icon".into(),
                    text: "".into(),
                    bounds: "10,10,50,50".into(),
                    checked_state: String::new(),
                    children: vec![],
                }],
            },
        ]
    }

    fn sample_state() -> UIState {
        UIState::new(
            sample_elements(),
            "formatted".into(),
            "focused".into(),
            PhoneState::default(),
            ScreenDimensions {
                width: 1080,
                height: 2400,
            },
            false,
        )
    }

    #[test]
    fn test_get_element() {
        let state = sample_state();
        assert!(state.get_element(1).is_some());
        assert_eq!(state.get_element(1).unwrap().text, "OK");
    }

    #[test]
    fn test_get_element_nested() {
        let state = sample_state();
        let el = state.get_element(3).unwrap();
        assert_eq!(el.class_name, "ImageView");
    }

    #[test]
    fn test_get_element_not_found() {
        let state = sample_state();
        assert!(state.get_element(999).is_none());
    }

    #[test]
    fn test_get_element_coords() {
        let state = sample_state();
        let (x, y) = state.get_element_coords(1).unwrap();
        assert_eq!((x, y), (200, 300)); // center of 100,200,300,400
    }

    #[test]
    fn test_all_indices() {
        let state = sample_state();
        assert_eq!(state.all_indices(), vec![1, 2, 3]);
    }

    #[test]
    fn test_convert_point_absolute() {
        let state = sample_state();
        let (x, y) = state.convert_point(540, 1200).unwrap();
        assert_eq!((x, y), (540, 1200));
    }

    #[test]
    fn test_convert_point_normalized() {
        let mut state = sample_state();
        state.use_normalized = true;
        let (x, y) = state.convert_point(500, 500).unwrap();
        assert_eq!((x, y), (540, 1200));
    }
}
