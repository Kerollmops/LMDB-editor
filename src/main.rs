#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;
use heed::types::ByteSlice;
use heed::{Database, Env, EnvOpenOptions};

fn main() -> anyhow::Result<()> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(320.0, 240.0)),
        ..Default::default()
    };

    let env_path = "edit-me.mdb";
    std::fs::create_dir_all(env_path)?;
    let env = EnvOpenOptions::new().open(env_path)?;

    eframe::run_native("LMDB Editor", options, Box::new(|ctx| Box::new(LmdbEditor::new(env, ctx))))
        .unwrap();

    Ok(())
}

struct LmdbEditor {
    env: Env,
    database: (Option<String>, Database<ByteSlice, ByteSlice>),
    entry_to_insert: String,
}

impl LmdbEditor {
    fn new(env: Env, _cc: &eframe::CreationContext<'_>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.

        // TODO do not try to create the database here.
        let mut wtxn = env.write_txn().unwrap();
        let main_db = env.create_database(&mut wtxn, None).unwrap();
        wtxn.commit().unwrap();
        LmdbEditor { env, database: (None, main_db), entry_to_insert: String::new() }
    }
}

impl eframe::App for LmdbEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                let label = ui.label("Insert value: ");
                ui.text_edit_singleline(&mut self.entry_to_insert).labelled_by(label.id);
            });

            if ui.button("insert").clicked() {
                let mut wtxn = self.env.write_txn().unwrap();
                self.database.1.put(&mut wtxn, self.entry_to_insert.as_bytes(), &[]).unwrap();
                wtxn.commit().unwrap();
                self.entry_to_insert.clear();
            }

            ui.separator();

            let rtxn = self.env.read_txn().unwrap();
            let text_style = egui::TextStyle::Body;
            let row_height = ui.text_style_height(&text_style);
            // let row_height = ui.spacing().interact_size.y; // if you are adding buttons instead of labels.
            let total_rows = self.database.1.len(&rtxn).unwrap().try_into().unwrap();
            egui::ScrollArea::vertical().show_rows(ui, row_height, total_rows, |ui, row_range| {
                let iter = self.database.1.iter(&rtxn).unwrap();
                for result in iter.skip(row_range.start).take(row_range.len()) {
                    use bstr::ByteSlice;
                    let (key, value) = result.unwrap();
                    let text = format!("{:?} - {:?}", key.as_bstr(), value.as_bstr());
                    ui.label(text);
                }
            });
        });
    }
}
