//! Action handlers for the form overlay (shown over `ViewOutput` pane).
//!
//! All functions take `&mut App` and mutate `state.form_state` in place.

use crate::action::Action;
use crate::app::App;
use crate::plugin::traits::FieldType;

pub fn next_field(app: &mut App) {
    if let Some(ref mut form) = app.state.form_state {
        if form.fields.is_empty() {
            return;
        }
        form.focused = (form.focused + 1) % form.fields.len();
    }
}

pub fn prev_field(app: &mut App) {
    if let Some(ref mut form) = app.state.form_state {
        form.focused = if form.focused == 0 {
            form.fields.len().saturating_sub(1)
        } else {
            form.focused - 1
        };
    }
}

pub fn input(app: &mut App, c: char) {
    if let Some(ref mut form) = app.state.form_state {
        if let Some(field) = form.fields.get_mut(form.focused) {
            if matches!(field.spec.field_type, FieldType::Text) {
                field.value.insert(field.cursor, c);
                field.cursor += c.len_utf8();
            }
        }
    }
}

pub fn backspace(app: &mut App) {
    if let Some(ref mut form) = app.state.form_state {
        if let Some(field) = form.fields.get_mut(form.focused) {
            if matches!(field.spec.field_type, FieldType::Text) && field.cursor > 0 {
                let prev = field.value[..field.cursor]
                    .char_indices()
                    .next_back()
                    .map_or(0, |(i, _)| i);
                field.value.remove(prev);
                field.cursor = prev;
            }
        }
    }
}

pub fn cursor_left(app: &mut App) {
    if let Some(ref mut form) = app.state.form_state {
        if let Some(field) = form.fields.get_mut(form.focused) {
            if field.cursor > 0 {
                field.cursor = field.value[..field.cursor]
                    .char_indices()
                    .next_back()
                    .map_or(0, |(i, _)| i);
            }
        }
    }
}

pub fn cursor_right(app: &mut App) {
    if let Some(ref mut form) = app.state.form_state {
        if let Some(field) = form.fields.get_mut(form.focused) {
            if field.cursor < field.value.len() {
                field.cursor = field.value[field.cursor..]
                    .char_indices()
                    .nth(1)
                    .map_or(field.value.len(), |(i, _)| field.cursor + i);
            }
        }
    }
}

pub fn select_next(app: &mut App) {
    if let Some(ref mut form) = app.state.form_state {
        if let Some(field) = form.fields.get_mut(form.focused) {
            if let FieldType::Select { ref options } = field.spec.field_type {
                if !options.is_empty() {
                    field.selected_option = (field.selected_option + 1) % options.len();
                    field.value = options[field.selected_option].clone();
                }
            }
        }
    }
}

pub fn select_prev(app: &mut App) {
    if let Some(ref mut form) = app.state.form_state {
        if let Some(field) = form.fields.get_mut(form.focused) {
            if let FieldType::Select { ref options } = field.spec.field_type {
                if !options.is_empty() {
                    field.selected_option = if field.selected_option == 0 {
                        options.len() - 1
                    } else {
                        field.selected_option - 1
                    };
                    field.value = options[field.selected_option].clone();
                }
            }
        }
    }
}

pub fn toggle(app: &mut App) {
    let kind = app
        .state
        .form_state
        .as_ref()
        .and_then(|f| f.fields.get(f.focused))
        .map(|f| f.spec.field_type.clone());
    match kind {
        Some(FieldType::Toggle) => {
            if let Some(ref mut form) = app.state.form_state {
                if let Some(field) = form.fields.get_mut(form.focused) {
                    field.toggled = !field.toggled;
                    field.value = if field.toggled { "true" } else { "false" }.to_string();
                }
            }
        }
        Some(FieldType::Select { .. }) => app.handle_action(Action::FormSelectNext),
        Some(FieldType::Text) => app.handle_action(Action::FormInput(' ')),
        None => {}
    }
}

pub fn submit(app: &mut App) {
    let Some(form) = app.state.form_state.take() else {
        return;
    };

    let all_valid = form
        .fields
        .iter()
        .all(|f| !f.spec.required || !f.value.trim().is_empty());

    if !all_valid {
        app.state.status_message = Some((
            "Required fields cannot be empty".to_string(),
            std::time::Instant::now(),
        ));
        app.state.form_state = Some(form);
        return;
    }

    if form.is_settings {
        // Settings form: persist values to plugin store, then rerun.
        let plugin_index = form.plugin_index;
        if let Some(meta) = app.state.plugins.get(plugin_index) {
            let store_path =
                crate::plugin::store::store_path_for(&meta.name, meta.plugin_group.as_deref());
            let mut store = crate::plugin::store::PluginStore::load(store_path);
            for field in &form.fields {
                let value = match field.spec.field_type {
                    FieldType::Toggle => if field.toggled { "true" } else { "false" }.to_string(),
                    FieldType::Select { ref options } => options
                        .get(field.selected_option)
                        .cloned()
                        .unwrap_or_default(),
                    FieldType::Text => field.value.clone(),
                };
                let _ = store.set(field.spec.id.clone(), serde_json::Value::String(value));
            }
            if let Err(e) = store.save() {
                tracing::warn!(error = %e, "failed to save plugin settings");
            }
        }
        app.state.result_cache.remove(&plugin_index);
        app.state.plugin_output = None;
        app.state.plugin_error = None;
        app.state.is_loading = true;
        app.state.loading_started = Some(std::time::Instant::now());
        app.state.scroll_offset = 0;
        app.dispatch_plugin(plugin_index);
    } else {
        let mut values = std::collections::HashMap::new();
        for field in &form.fields {
            values.insert(field.spec.id.clone(), field.value.clone());
        }
        let plugin_index = form.plugin_index;
        app.state.is_loading = true;
        app.state.plugin_output = None;
        app.state.loading_started = Some(std::time::Instant::now());
        app.dispatch_plugin_with_form(plugin_index, values);
    }
}

pub fn cancel(app: &mut App) {
    app.state.form_state = None;
    app.handle_action(Action::Back);
}
