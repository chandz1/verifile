use iced::{
    Element, Length, Task,
};
use iced::widget::{
    Column, Row, Container, Text, Button, PickList, TextInput, Scrollable, Space,
};

use crate::models::*;
use crate::storage;
use crate::utils;
use std::path::PathBuf;
use uuid::Uuid;
use anyhow::Result;
use rfd::FileDialog;
use tokio::task;
use chrono::Utc;

#[derive(Debug, Clone)]
pub enum Message {
    ChooseFile,
    FileChosen(Option<PathBuf>),
    AlgorithmSelected(Algorithm),
    PasteHashChanged(String),
    LoadHashFile,
    HashFileLoaded(Option<String>),
    StartVerify,
    VerifyComplete(Result<VerificationRecord, String>),
}

pub struct VeriFileApp {
    // UI state
    chosen_file: Option<PathBuf>,
    algorithm: Algorithm,
    paste_hash: String,
    status_message: String,
    is_verifying: bool,

    // past verifications
    past: Vec<VerificationRecord>,
}

impl VeriFileApp {
    pub fn new() -> (Self, Task<Message>) {
        let past = storage::load_all();
        (
            VeriFileApp {
                chosen_file: None,
                algorithm: Algorithm::Blake3,
                paste_hash: String::new(),
                status_message: String::new(),
                is_verifying: false,
                past,
            },
            Task::none(),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ChooseFile => {
                return Task::perform(async {
                    FileDialog::new().set_directory(".").pick_file()
                }, |res| Message::FileChosen(res));
            }
            Message::FileChosen(Some(path)) => {
                self.chosen_file = Some(path);
            }
            Message::FileChosen(None) => { /* cancelled */ }
            Message::AlgorithmSelected(a) => {
                self.algorithm = a;
            }
            Message::PasteHashChanged(s) => {
                self.paste_hash = s;
            }
            Message::LoadHashFile => {
                return Task::perform(async {
                    FileDialog::new().set_directory(".").add_filter("text", &["txt", "hash", "md"]).pick_file()
                }, |res| {
                    if let Some(p) = res {
                        let txt = std::fs::read_to_string(p).ok();
                        Message::HashFileLoaded(txt)
                    } else {
                        Message::HashFileLoaded(None)
                    }
                });
            }
            Message::HashFileLoaded(opt) => {
                if let Some(txt) = opt {
                    if let Some(h) = utils::parse_first_hash_from_text(&txt) {
                        self.paste_hash = h;
                    }
                }
            }
            Message::StartVerify => {
                if let Some(path) = self.chosen_file.clone() {
                    println!("Starting verification for: {:?}", path);
                    self.status_message = "Computing hash...".to_string();
                    self.is_verifying = true;
                    let algo = self.algorithm.clone();
                    let ref_hash = if self.paste_hash.trim().is_empty() { None } else { Some(self.paste_hash.clone()) };
                    return Task::perform(async move {
                        let computed: Result<(String, PathBuf, Algorithm), anyhow::Error> = task::spawn_blocking(move || {
                            let hex = crate::file_ops::compute_file_hash(&path, &algo)?;
                            Ok((hex, path, algo))
                        }).await.unwrap();
                        match computed {
                            Ok((hex, path, algo)) => {
                                let status = if let Some(rh) = &ref_hash {
                                    if rh.trim().eq_ignore_ascii_case(&hex) { VerificationStatus::Success } else { VerificationStatus::Failed }
                                } else {
                                    VerificationStatus::Success
                                };
                                let rec = VerificationRecord {
                                    id: Uuid::new_v4().to_string(),
                                    file_name: path.file_name().and_then(|s| s.to_str()).unwrap_or("file").to_string(),
                                    file_path: path,
                                    algorithm: algo,
                                    computed_hash: hex,
                                    reference_hash: ref_hash,
                                    status,
                                    timestamp: Utc::now(),
                                };
                                Ok(rec)
                            },
                            Err(e) => Err(format!("Hash compute error: {:?}", e)),
                        }
                    }, |res| Message::VerifyComplete(res));
                }
            }
            Message::VerifyComplete(result) => {
                self.is_verifying = false;
                match result {
                    Ok(rec) => {
                        println!("Verification complete: {:?}", rec.status);
                        let status_msg = match rec.status {
                            VerificationStatus::Success => "✓ Verification successful!",
                            VerificationStatus::Failed => "✗ Verification failed - hash mismatch!",
                            VerificationStatus::InProgress => "In progress...",
                        };
                        self.status_message = status_msg.to_string();
                        self.past.insert(0, rec.clone());
                        let _ = storage::save_all(&self.past);
                        self.paste_hash.clear();
                        self.chosen_file = None;
                    }
                    Err(e) => {
                        println!("Verification error: {:?}", e);
                        self.status_message = format!("Error: {}", e);
                    }
                }
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let header = Text::new("New Verification").size(32);

        let file_row = Row::new()
            .spacing(10)
            .push(Button::new(Text::new("Browse Files")).on_press(Message::ChooseFile))
            .push(Text::new(self.chosen_file.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "No file chosen".into())).size(16));

        let algo_list = PickList::new(
            Algorithm::all(),
            Some(self.algorithm.clone()),
            Message::AlgorithmSelected,
        );

        let input = TextInput::new(
            "Paste Hash Reference (optional)",
            &self.paste_hash,
        )
        .on_input(Message::PasteHashChanged)
        .padding(10);

        let verify_btn = if self.chosen_file.is_some() && !self.is_verifying {
            Button::new(Text::new("Verify File")).on_press(Message::StartVerify)
        } else {
            Button::new(Text::new(if self.is_verifying { "Verifying..." } else { "Verify File" }))
        };

        let mut left = Column::new()
            .padding(20)
            .spacing(16)
            .push(header)
            .push(file_row)
            .push(Space::with_height(8))
            .push(Text::new("Algorithm:"))
            .push(algo_list)
            .push(Space::with_height(8))
            .push(input)
            .push(Button::new(Text::new("Upload Hash File")).on_press(Message::LoadHashFile))
            .push(Space::with_height(12))
            .push(verify_btn);

        if !self.status_message.is_empty() {
            left = left.push(Space::with_height(8))
                       .push(Text::new(&self.status_message).size(18));
        }

        let mut past_list = Column::new().padding(10).spacing(8);
        for r in &self.past {
            let status_text = match r.status {
                VerificationStatus::Success => Text::new("Success"),
                VerificationStatus::Failed => Text::new("Fail"),
                VerificationStatus::InProgress => Text::new("In progress"),
            };
            let row = Row::new()
                .spacing(12)
                .push(Text::new(&r.file_name))
                .push(Text::new(r.timestamp.format("%Y-%m-%d %H:%M:%S").to_string()))
                .push(status_text);
            past_list = past_list.push(row);
        }

        let past_scrollable = Scrollable::new(past_list);

        let layout = Row::new()
            .spacing(20)
            .push(Container::new(left).width(Length::FillPortion(2)).padding(16))
            .push(Container::new(past_scrollable).width(Length::FillPortion(3)).padding(16));

        Container::new(layout)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}
