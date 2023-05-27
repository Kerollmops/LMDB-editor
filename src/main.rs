#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::mem;
use std::ops::Deref;

use eframe::egui;
use egui::Color32;
use egui_extras::{Column, TableBuilder};
use heed::types::ByteSlice;
use heed::{Database, Env, EnvOpenOptions};
use once_cell::sync::OnceCell;
use rfd::FileDialog;

static ENV: OnceCell<Env> = OnceCell::new();

fn main() -> anyhow::Result<()> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(720.0, 480.0)),
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
    database_to_open: String,
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
            database_to_open: String::new(),
            entry_to_insert: EscapedEntry::default(),
            wtxn: None,
        }
    }
}

impl eframe::App for LmdbEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::Window::new("Edit or Insert an Entry").default_pos([720.0, 480.0]).show(ctx, |ui| {
            ui.style_mut().spacing.interact_size.y = 0.0; // hack to make `horizontal_wrapped` work better with text.

            ui.label("We use STFU-8 as a hacky text encoding/decoding protocol for data that might be not quite UTF-8 but is still mostly UTF-8. \
            It is based on the syntax of the repr created when you write (or print) binary text in python, C or other common programming languages.");

            ui.add_space(8.0);

            ui.label("Basically STFU-8 is the text format you already write when use escape codes in C, python, rust, etc. \
            It permits binary data in UTF-8 by escaping them with \\, for instance \\n and \\x0F.");

            ui.add_space(8.0);

            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label("More about how we interpret encoding/decoding ");
                ui.hyperlink_to("on the stfu8 documentation", "https://docs.rs/stfu8");
                ui.label(".");
            });

            ui.separator();

            let EscapedEntry { key, data } = &mut self.entry_to_insert;
            ui.add(egui::TextEdit::singleline(key).hint_text("escaped key"));
            ui.add(egui::TextEdit::multiline(data).hint_text("escaped data"));

            if ui.button("insert").clicked() {
                if let Some(wtxn) = self.wtxn.as_mut() {
                    let key = self.entry_to_insert.decoded_key().unwrap();
                    let data = self.entry_to_insert.decoded_data().unwrap();
                    self.database.1.put(wtxn, &key, &data).unwrap();
                    self.entry_to_insert.clear();
                }
            }

            if ui.button("delete").clicked() {
                if let Some(wtxn) = self.wtxn.as_mut() {
                    let key = self.entry_to_insert.decoded_key().unwrap();
                    self.database.1.delete(wtxn, &key).unwrap();
                    self.entry_to_insert.clear();
                }
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.database_to_open)
                            .hint_text("database name"),
                    );
                    if ui.button("open").clicked() {
                        let env = ENV.wait();
                        let database_name = if self.database_to_open.is_empty() {
                            None
                        } else {
                            Some(mem::take(&mut self.database_to_open))
                        };

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

                        let db = env
                            .open_database(&rtxn, database_name.as_ref().map(AsRef::as_ref))
                            .unwrap()
                            .unwrap();
                        self.database = (database_name, db);
                    }
                });

                ui.separator();

                let button = if self.wtxn.is_some() {
                    egui::Button::new("start writing").fill(Color32::DARK_GREEN)
                } else {
                    egui::Button::new("start writing").fill(Color32::DARK_RED)
                };

                if ui.add(button).clicked() && self.wtxn.is_none() {
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

            let num_rows = self.database.1.len(&rtxn).unwrap().try_into().unwrap();
            let mut prev_row_index = None;
            let mut iter = self.database.1.iter(&rtxn).unwrap();

            TableBuilder::new(ui)
                .column(Column::auto().resizable(true))
                .column(Column::auto().resizable(true))
                .column(Column::remainder())
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.label("Keys");
                    });
                    header.col(|ui| {
                        ui.label("Values");
                    });
                    header.col(|ui| {
                        ui.label("Operations");
                    });
                })
                .body(|body| {
                    body.rows(30.0, num_rows, |row_index, mut row| {
                        assert!(prev_row_index.map_or(true, |p| p + 1 == row_index));
                        if prev_row_index.is_none() {
                            let _ = iter.by_ref().skip(row_index).next();
                            // iter.advance_by(row_index).unwrap();
                        }
                        prev_row_index = Some(row_index);

                        if let Some(result) = iter.next() {
                            let (key, data) = result.unwrap();
                            let encoded_key = stfu8::encode_u8_pretty(key);
                            let encoded_data = stfu8::encode_u8_pretty(data);

                            row.col(|ui| {
                                ui.label(&encoded_key);
                            });
                            row.col(|ui| {
                                ui.label(&encoded_data);
                            });
                            row.col(|ui| {
                                // TODO Replace me by a ✏️
                                if ui.button("edit").clicked() {
                                    self.entry_to_insert.key = encoded_key;
                                    self.entry_to_insert.data = encoded_data;
                                }
                                // // Replace me by a red 🗑️
                                // if ui.button("delete").clicked() {
                                //     if let Some(wtxn) = self.wtxn.as_mut() {
                                //     }
                                // }
                            });
                        }
                    });
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

    pub fn decoded_key(&self) -> Result<Vec<u8>, stfu8::DecodeError> {
        stfu8::decode_u8(&self.key)
    }

    pub fn decoded_data(&self) -> Result<Vec<u8>, stfu8::DecodeError> {
        stfu8::decode_u8(&self.data)
    }
}
