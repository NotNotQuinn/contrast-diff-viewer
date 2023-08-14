use egui::{Align, Color32, Layout, RichText, ScrollArea, Ui, Window};
use std::path::PathBuf;

use git::{Diff, DiffParsingError, Line, Stats};

use eframe::egui;

mod git;

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(320.0, 240.0)),
        ..Default::default()
    };

    eframe::run_native("Contrast", options, Box::new(|_cc| Box::<MyApp>::default()))
}

#[derive(Default)]
struct MyApp {
    app_data: Option<AppData>,
    show_err_dialog: bool,
    error_information: String,
}

struct AppData {
    project_path: String,
    diffs: Vec<Diff>,
    stats: Stats,
    selected_diff_index: usize,
}

enum AppDataCreationError {
    Parsing,
}

impl AppData {
    fn new(path: PathBuf) -> Result<AppData, AppDataCreationError> {
        let project_path = path
            .to_str()
            .ok_or(AppDataCreationError::Parsing)?
            .to_owned();
        let (diffs, stats) =
            git::get_diffs(project_path.clone()).map_err(|_| AppDataCreationError::Parsing)?;

        Ok(AppData {
            project_path,
            diffs,
            stats,
            selected_diff_index: 0,
        })
    }

    fn refresh(&mut self) -> Result<(), DiffParsingError> {
        let (diffs, stats) = git::get_diffs(self.project_path.clone())?;
        self.diffs = diffs;
        self.stats = stats;
        self.selected_diff_index = 0;

        Ok(())
    }

    fn get_selected_diff(&self) -> Option<&Diff> {
        self.diffs.get(self.selected_diff_index)
    }

    fn get_stats_richtext(&self) -> RichText {
        let file_changed_count = self.stats.files_changed;
        let insertion_count = self.stats.insertions;
        let deletion_count = self.stats.deletions;

        let content = format!(
            "{} file(s) changed, {} insertions(+), {} deletions(-)\n",
            file_changed_count, insertion_count, deletion_count
        );

        RichText::new(content).color(Color32::WHITE)
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.selection_area(ctx, ui);
            self.project_area(ui);

            if let Some(app_data) = &self.app_data {
                if app_data.diffs.is_empty() {
                    return;
                }
            }

            ui.with_layout(Layout::left_to_right(Align::LEFT), |ui| {
                self.files_area(ui);
                self.diff_area(ui);
            });
        });
    }
}

impl MyApp {
    fn selection_area(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Diff Viewer").color(Color32::WHITE));
            ui.separator();

            if ui
                .button(RichText::new("Open").color(Color32::WHITE))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    match AppData::new(path) {
                        Ok(app_data) => self.app_data = Some(app_data),
                        Err(err) => match err {
                            AppDataCreationError::Parsing => {
                                self.show_error("Parsing failed!".to_owned())
                            }
                        },
                    }
                }
            }

            if self.show_err_dialog {
                self.error_dialog(ctx);
            }

            if self.app_data.is_some()
                && ui
                    .button(RichText::new("Refresh").color(Color32::WHITE))
                    .clicked()
            {
                if let Some(app_data) = &mut self.app_data {
                    if app_data.refresh().is_err() {
                        self.show_error("Refresh failed!".to_owned());
                    };
                }
            }
        });

        ui.separator();
    }

    fn project_area(&mut self, ui: &mut Ui) {
        if let Some(app_data) = &mut self.app_data {
            ui.heading(RichText::new(app_data.project_path.clone()).color(Color32::WHITE));
            ui.label(app_data.get_stats_richtext());
            ui.separator();
        }
    }

    fn files_area(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            if let Some(app_data) = &mut self.app_data {
                ScrollArea::vertical()
                    .id_source("file scroll area")
                    .show(ui, |ui| {
                        for (i, diff) in app_data.diffs.iter().enumerate() {
                            if app_data.selected_diff_index == i {
                                ui.button(diff.file_name()).highlight();
                            } else if ui.button(diff.file_name()).clicked() {
                                app_data.selected_diff_index = i;
                            }
                        }
                    });
            }
        });
    }

    fn diff_area(&self, ui: &mut Ui) {
        if let Some(app_data) = &self.app_data {
            let Some(diff) = app_data.get_selected_diff() else {
                return;
            };

            if diff.lines.is_empty() {
                ui.label(RichText::new("No content").color(Color32::GRAY));
                return;
            }

            let longest_line = self.get_longest_line(diff.clone());

            ui.vertical(|ui| {
                ScrollArea::both()
                    .id_source("diff area")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for line in &diff.lines {
                            for header in &diff.headers {
                                if header.line == line.new_lineno.unwrap_or(0)
                                    && line.origin != '+'
                                    && line.origin != '-'
                                {
                                    let (green_label, white_label) = header.to_labels();
                                    ui.horizontal(|ui| {
                                        ui.add(green_label);
                                        ui.add(white_label);
                                    });
                                }
                            }

                            let line_no_richtext = self.get_line_no_richtext(line, longest_line);

                            ui.horizontal(|ui| {
                                ui.label(line_no_richtext);
                                ui.label(line.to_richtext());
                            });
                        }
                    });
            });
        }
    }

    fn get_line_no_richtext(&self, line: &Line, longest_line: u32) -> RichText {
        let mut line_no = match line.origin {
            '+' => line.new_lineno.unwrap_or(0).to_string(),
            '-' => line.old_lineno.unwrap_or(0).to_string(),
            _ => line.new_lineno.unwrap_or(0).to_string(),
        };

        while line_no.len() != longest_line.to_string().len() {
            line_no = format!(" {}", line_no);
        }

        RichText::new(line_no).color(Color32::GRAY).monospace()
    }

    fn get_longest_line(&self, diff: Diff) -> u32 {
        let mut longest_line = 0;
        for line in &diff.lines {
            let line_no = match line.origin {
                '+' => line.new_lineno.unwrap_or(0),
                '-' => line.old_lineno.unwrap_or(0),
                _ => line.new_lineno.unwrap_or(0),
            };

            if line_no > longest_line {
                longest_line = line_no;
            }
        }

        longest_line
    }

    fn show_error(&mut self, information: String) {
        self.error_information = information;
        self.show_err_dialog = true;
    }

    fn error_dialog(&mut self, ctx: &egui::Context) {
        Window::new("Error")
            .collapsible(false)
            .resizable(true)
            .show(ctx, |ui| {
                ui.label(RichText::new(self.error_information.clone()).strong());
                if ui.button("Close").clicked() {
                    self.error_information = "".to_owned();
                    self.show_err_dialog = false;
                }
            });
    }
}
