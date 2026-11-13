use super::*;
use eframe::egui;

#[derive(Default)]
pub struct Renderer {
    cache: BTreeMap<(String, String), (u64, Result<Value, String>)>,
}

impl Renderer {
    /// Drops the cached metadata of windows that are gone.
    pub fn retain(&mut self, windows: &[Window]) {
        self.cache
            .retain(|(id, _), _| windows.iter().any(|w| &w.id == id));
    }

    /// The widgets of one window, drawn into `ui`; wherever the window is placed.
    pub fn contents(
        &mut self,
        ui: &mut egui::Ui,
        window: &Window,
        parameters: &mut impl Parameters,
        mut observe: impl FnMut(&Widget, &egui::Response),
    ) {
        for widget in &window.widgets {
            ui.push_id(&widget.id, |ui| {
                if widget.kind == "label" {
                    ui.label(&widget.label);
                    return;
                }
                let result = self.read(window, widget, parameters);
                match result {
                    Err(error) => {
                        ui.add_enabled(false, egui::Label::new(&widget.label));
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                    Ok(parameter) => {
                        let mut value = parameter.value;
                        let response = if widget.kind == "checkbox" {
                            let mut checked = value == parameter.max;
                            let response = ui.checkbox(&mut checked, &widget.label);
                            value = if checked {
                                parameter.max
                            } else {
                                parameter.min
                            };
                            response
                        } else {
                            let slider =
                                egui::Slider::new(&mut value, parameter.min..=parameter.max)
                                    .text(&widget.label);
                            let slider = if parameter.discrete {
                                slider.integer()
                            } else {
                                slider
                            };
                            ui.add(slider)
                        };
                        observe(widget, &response);
                        if response.changed() {
                            if let Err(error) =
                                self.write(window, widget, parameters, &parameter, value)
                            {
                                ui.colored_label(ui.visuals().error_fg_color, error);
                            }
                        }
                    }
                }
            });
        }
    }

    fn read(
        &mut self,
        window: &Window,
        widget: &Widget,
        parameters: &mut impl Parameters,
    ) -> Result<Parameter, String> {
        let parameter = parameters.parameter(widget.binding.as_ref().unwrap())?;
        let entry = self
            .cache
            .entry((window.id.clone(), widget.id.clone()))
            .or_insert_with(|| (window.revision, Ok(parameter.fingerprint.clone())));
        if entry.0 != window.revision {
            *entry = (window.revision, Ok(parameter.fingerprint.clone()));
        }
        if entry
            .1
            .as_ref()
            .is_ok_and(|fingerprint| fingerprint != &parameter.fingerprint)
        {
            entry.1 = Err(t("ui_changed").into());
        }
        if let Err(error) = &entry.1 {
            return Err(error.clone());
        }
        if parameter.readonly {
            return Err(t("ui_readonly").into());
        }
        if widget.kind == "checkbox"
            && (!parameter.discrete || parameter.max - parameter.min != 1.0)
        {
            return Err(t("ui_binary").into());
        }
        Ok(parameter)
    }

    fn write(
        &mut self,
        window: &Window,
        widget: &Widget,
        parameters: &mut impl Parameters,
        before: &Parameter,
        value: f64,
    ) -> Result<(), String> {
        let current = self.read(window, widget, parameters)?;
        if before.fingerprint != current.fingerprint
            || !value.is_finite()
            || value < current.min
            || value > current.max
        {
            return Err(t("ui_changed").into());
        }
        parameters.set(widget.binding.as_ref().unwrap(), value)
    }

    pub(super) fn test_parameter(
        &mut self,
        window: &Window,
        widget: &Widget,
        parameters: &mut impl Parameters,
    ) -> Result<Parameter, String> {
        self.read(window, widget, parameters)
    }

    #[cfg(test)]
    pub(super) fn test_read(
        &mut self,
        window: &Window,
        widget: &Widget,
        parameters: &mut impl Parameters,
    ) -> Result<Parameter, String> {
        self.read(window, widget, parameters)
    }
}
