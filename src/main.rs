#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::ops::Deref;

use eframe::egui;
use heed::types::ByteSlice;
use heed::{Database, Env, EnvOpenOptions};
use once_cell::sync::OnceCell;
use rfd::FileDialog;

static ENV: OnceCell<Env> = OnceCell::new();

fn main() -> anyhow::Result<()> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(320.0, 240.0)),
        ..Default::default()
    };

    if let Some(env_path) = FileDialog::new().pick_folder() {
        let env = EnvOpenOptions::new().open(env_path)?;
        let _ = ENV.set(env.clone());

        eframe::run_native(
            "LMDB Editor",
            options,
            Box::new(|ctx| Box::new(LmdbEditor::new(env, ctx))),
        )
        .unwrap();
    }

    Ok(())
}

struct LmdbEditor {
    env: Env,
    database: (Option<String>, Database<ByteSlice, ByteSlice>),
    entry_to_insert: EscapedEntry,
    wtxn: Option<heed::RwTxn<'static>>,
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
        LmdbEditor {
            env,
            database: (None, main_db),
            entry_to_insert: EscapedEntry::default(),
            wtxn: None,
        }
    }
}

impl eframe::App for LmdbEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                let label = ui.label("Insert value: ");
                let EscapedEntry { key, data } = &mut self.entry_to_insert;
                ui.add(egui::TextEdit::singleline(key).hint_text("escaped key"))
                    .labelled_by(label.id);
                ui.add(egui::TextEdit::singleline(data).hint_text("escaped data"));
            });

            ui.horizontal(|ui| {
                if ui.button("insert").clicked() {
                    if let Some(wtxn) = self.wtxn.as_mut() {
                        let key = self.entry_to_insert.decode_key().unwrap();
                        let data = self.entry_to_insert.decode_data().unwrap();
                        self.database.1.put(wtxn, &key, &data).unwrap();
                        self.entry_to_insert.clear();
                    }
                }

                ui.separator();

                if ui.button("start writing").clicked() {
                    let env = ENV.wait();
                    let wtxn = env.write_txn().unwrap();
                    self.wtxn = Some(wtxn);
                }

                if ui.button("commit changes").clicked() {
                    if let Some(wtxn) = self.wtxn.take() {
                        wtxn.commit().unwrap();
                    }
                }

                if ui.button("abort changes").clicked() {
                    if let Some(wtxn) = self.wtxn.take() {
                        wtxn.abort();
                    }
                }
            });

            ui.separator();

            // If there is a write txn opened, use it, else use a new read txn.
            let long_rtxn;
            let rtxn;
            match self.wtxn.as_ref() {
                Some(wtxn) => rtxn = wtxn.deref(),
                None => {
                    long_rtxn = self.env.read_txn().unwrap();
                    rtxn = &long_rtxn;
                }
            };

            let text_style = egui::TextStyle::Body;
            let row_height = ui.text_style_height(&text_style);
            // let row_height = ui.spacing().interact_size.y; // if you are adding buttons instead of labels.
            let total_rows = self.database.1.len(&rtxn).unwrap().try_into().unwrap();
            egui::ScrollArea::vertical().show_rows(ui, row_height, total_rows, |ui, row_range| {
                let iter = self.database.1.iter(&rtxn).unwrap();
                for result in iter.skip(row_range.start).take(row_range.len()) {
                    let (key, data) = result.unwrap();
                    ui.horizontal(|ui| {
                        ui.label(stfu8::encode_u8(key));
                        ui.label(stfu8::encode_u8(data));
                    });
                }
            });
        });
    }
}

#[derive(Debug, Default)]
struct EscapedEntry {
    key: String,
    data: String,
}

impl EscapedEntry {
    pub fn clear(&mut self) {
        self.key.clear();
        self.data.clear();
    }

    pub fn decode_key(&self) -> Result<Vec<u8>, stfu8::DecodeError> {
        stfu8::decode_u8(&self.key)
    }

    pub fn decode_data(&self) -> Result<Vec<u8>, stfu8::DecodeError> {
        stfu8::decode_u8(&self.data)
    }
}
