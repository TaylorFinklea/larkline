//! Helpers for the output pane: visibility, selection, search filter, and
//! form initialization. All operate on [`AppState`] directly.

use crate::app::{AppState, FormFieldState, FormState, OutputMode};
use crate::plugin::traits::{FieldType, FormSpec, OutputItem, PluginOutput};

/// Number of visible output items (filtered count when searching, total otherwise).
pub fn visible_output_count(state: &AppState) -> usize {
    if !state.output_filtered_indices.is_empty() || !state.output_query.is_empty() {
        state.output_filtered_indices.len()
    } else {
        state
            .plugin_output
            .as_ref()
            .map_or(0, |o| o.items.len())
    }
}

/// Returns the output item at the current `output_selected` position,
/// mapped through `output_filtered_indices` when a search is active.
pub fn selected_output_item(state: &AppState) -> Option<&OutputItem> {
    let items = &state.plugin_output.as_ref()?.items;
    if state.output_filtered_indices.is_empty() && state.output_query.is_empty() {
        items.get(state.output_selected)
    } else {
        let real_index = *state
            .output_filtered_indices
            .get(state.output_selected)?;
        items.get(real_index)
    }
}

/// Rebuild `output_filtered_indices` based on `output_query`.
///
/// Empty query → all item indices. Non-empty → case-insensitive substring match
/// on label+detail.
pub fn rebuild_output_filter(state: &mut AppState) {
    let items = if let Some(ref o) = state.plugin_output {
        &o.items
    } else {
        state.output_filtered_indices.clear();
        return;
    };

    if state.output_query.is_empty() {
        state.output_filtered_indices = (0..items.len()).collect();
    } else {
        let query_lower = state.output_query.to_lowercase();
        state.output_filtered_indices = items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                let haystack = match item.detail {
                    Some(ref d) => format!("{} {d}", item.label),
                    None => item.label.clone(),
                };
                haystack.to_lowercase().contains(&query_lower)
            })
            .map(|(i, _)| i)
            .collect();
    }

    // Clamp selection to filtered range.
    let max = state.output_filtered_indices.len().saturating_sub(1);
    if state.output_selected > max {
        state.output_selected = max;
    }
}

/// Reset output search state (called when entering `ViewOutput` or going Back).
pub fn reset_output_search(state: &mut AppState) {
    state.output_query.clear();
    state.output_searching = false;
    state.output_filtered_indices.clear();
}

/// Determine the best output mode for the given output.
pub fn output_mode_for(output: &PluginOutput) -> OutputMode {
    if output.output_format.as_deref() == Some("markdown") && output.raw_text.is_some() {
        OutputMode::Markdown
    } else if !output.columns.is_empty() {
        OutputMode::Table
    } else if output.raw_text.is_some() && output.items.is_empty() {
        OutputMode::RawText
    } else {
        OutputMode::List
    }
}

/// Check if the current plugin output has a form and initialize form state.
pub fn check_form_init(state: &mut AppState, plugin_index: usize) {
    let form = state
        .plugin_output
        .as_ref()
        .and_then(|o| o.form.clone());
    if let Some(form_spec) = form {
        initialize_form(state, plugin_index, &form_spec);
    }
}

/// Initialize form state from a `FormSpec` returned by a plugin.
pub fn initialize_form(state: &mut AppState, plugin_index: usize, form_spec: &FormSpec) {
    let fields: Vec<FormFieldState> = form_spec
        .fields
        .iter()
        .map(|field| {
            let default = field.default_value.clone().unwrap_or_default();
            let selected_option = if let FieldType::Select { ref options } = field.field_type {
                options.iter().position(|o| o == &default).unwrap_or(0)
            } else {
                0
            };
            let toggled = default == "true";
            let cursor = default.len();
            FormFieldState {
                spec: field.clone(),
                value: default,
                cursor,
                selected_option,
                toggled,
            }
        })
        .collect();

    state.form_state = Some(FormState {
        fields,
        focused: 0,
        plugin_index,
        submit_label: form_spec
            .submit_label
            .clone()
            .unwrap_or_else(|| "Submit".to_string()),
        is_settings: false,
    });
}
