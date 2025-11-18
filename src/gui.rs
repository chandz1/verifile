use iced::{
    Element, Length, Task, Color, Alignment, Border,
};
use iced::widget::{
    Column, Row, Container, Text, Button, PickList, TextInput, Scrollable, Space, rule,
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

#[derive(Debug, Clone, PartialEq)]
pub enum VerificationStep {
    UploadFile,
    UploadHash,
    Verifying,
    Result,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Theme {
    Light,
    Dark,
}

#[derive(Debug, Clone)]
pub enum Message {
    ChooseFile,
    FileChosen(Option<PathBuf>),
    AlgorithmSelected(Algorithm),
    PasteHashChanged(String),
    LoadHashFile,
    HashFileLoaded(Option<String>),
    ProceedToHash,
    StartVerify,
    VerifyComplete(Result<VerificationRecord, String>),
    ResetVerification,
    ToggleHistory,
    ToggleTheme,
}

pub struct VeriFileApp {
    // UI state
    chosen_file: Option<PathBuf>,
    algorithm: Algorithm,
    paste_hash: String,
    status_message: String,
    current_step: VerificationStep,
    is_verifying: bool,
    show_history: bool,
    last_result: Option<VerificationRecord>,
    theme: Theme,

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
                current_step: VerificationStep::UploadFile,
                is_verifying: false,
                show_history: false,
                theme: Theme::Light,
                last_result: None,
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
                self.current_step = VerificationStep::UploadFile;
            }
            Message::FileChosen(None) => { /* cancelled */ }
            Message::AlgorithmSelected(a) => {
                self.algorithm = a;
            }
            Message::ProceedToHash => {
                if self.chosen_file.is_some() {
                    self.current_step = VerificationStep::UploadHash;
                }
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
                    self.current_step = VerificationStep::Verifying;
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
                self.current_step = VerificationStep::Result;
                match result {
                    Ok(rec) => {
                        println!("Verification complete: {:?}", rec.status);
                        let status_msg = match rec.status {
                            VerificationStatus::Success => "✓ Verification successful!",
                            VerificationStatus::Failed => "✗ Verification failed - hash mismatch!",
                            VerificationStatus::InProgress => "In progress...",
                        };
                        self.status_message = status_msg.to_string();
                        self.last_result = Some(rec.clone());
                        self.past.insert(0, rec.clone());
                        let _ = storage::save_all(&self.past);
                    }
                    Err(e) => {
                        println!("Verification error: {:?}", e);
                        self.status_message = format!("Error: {}", e);
                        self.last_result = None;
                    }
                }
            }
            Message::ResetVerification => {
                self.chosen_file = None;
                self.paste_hash.clear();
                self.status_message.clear();
                self.current_step = VerificationStep::UploadFile;
                self.last_result = None;
            }
            Message::ToggleHistory => {
                self.show_history = !self.show_history;
            }
            Message::ToggleTheme => {
                self.theme = match self.theme {
                    Theme::Light => Theme::Dark,
                    Theme::Dark => Theme::Light,
                };
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        // Sidebar
        let sidebar = self.view_sidebar();
        
        // Main content based on current step
        let main_content = match self.current_step {
            VerificationStep::UploadFile => self.view_upload_file(),
            VerificationStep::UploadHash => self.view_upload_hash(),
            VerificationStep::Verifying => self.view_verifying(),
            VerificationStep::Result => self.view_result(),
        };

        // Layout
        let layout = Row::new()
            .push(sidebar)
            .push(rule::Rule::vertical(1))
            .push(main_content);

        let bg_color = self.bg_color();
        Container::new(layout)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_theme| {
                iced::widget::container::Style {
                    background: Some(iced::Background::Color(bg_color)),
                    border: Border::default(),
                    ..Default::default()
                }
            })
            .into()
    }

    fn view_sidebar(&self) -> Element<'_, Message> {
        let accent = self.accent_color();
        let text_color = self.text_color();
        let secondary_text = self.secondary_text_color();

        let title = Text::new("VeriFile")
            .size(28)
            .style(move |_theme| {
                iced::widget::text::Style {
                    color: Some(accent),
                }
            });

        let subtitle = Text::new("File Hash Verifier")
            .size(14)
            .style(move |_theme| {
                iced::widget::text::Style {
                    color: Some(secondary_text),
                }
            });

        let divider = rule::Rule::horizontal(1);

        let algo_label = Text::new("Hash Algorithm")
            .size(16)
            .style(move |_theme| {
                iced::widget::text::Style {
                    color: Some(text_color),
                }
            });
        let algo_picker = PickList::new(
            Algorithm::all(),
            Some(self.algorithm.clone()),
            Message::AlgorithmSelected,
        )
        .padding(10)
        .width(Length::Fill);

        let tertiary_text = self.tertiary_text_color();
        let algo_desc = Text::new(self.get_algorithm_description())
            .size(12)
            .style(move |_theme| {
                iced::widget::text::Style {
                    color: Some(tertiary_text),
                }
            });

        let theme_btn = Button::new(
            Text::new(match self.theme {
                Theme::Light => "🌙 Dark Mode",
                Theme::Dark => "☀️ Light Mode",
            })
                .size(14)
        )
        .on_press(Message::ToggleTheme)
        .padding(10)
        .width(Length::Fill);

        let history_btn = Button::new(
            Text::new(if self.show_history { "Hide History" } else { "Show History" })
                .size(14)
        )
        .on_press(Message::ToggleHistory)
        .padding(10)
        .width(Length::Fill);

        let mut sidebar_content = Column::new()
            .padding(20)
            .spacing(20)
            .width(Length::Fixed(280.0))
            .push(title)
            .push(subtitle)
            .push(Space::with_height(10))
            .push(divider)
            .push(Space::with_height(10))
            .push(algo_label)
            .push(algo_picker)
            .push(algo_desc)
            .push(Space::with_height(20))
            .push(theme_btn)
            .push(history_btn);

        // Show history if toggled
        if self.show_history {
            sidebar_content = sidebar_content
                .push(Space::with_height(10))
                .push(rule::Rule::horizontal(1))
                .push(Space::with_height(10))
                .push(Text::new("Recent Verifications").size(14));

            let mut history_list = Column::new().spacing(8);
            for (i, r) in self.past.iter().take(5).enumerate() {
                let status_icon = match r.status {
                    VerificationStatus::Success => "✓",
                    VerificationStatus::Failed => "✗",
                    VerificationStatus::InProgress => "⋯",
                };
                let status_color = match r.status {
                    VerificationStatus::Success => Color::from_rgb(0.2, 0.8, 0.2),
                    VerificationStatus::Failed => Color::from_rgb(0.9, 0.2, 0.2),
                    VerificationStatus::InProgress => Color::from_rgb(0.7, 0.7, 0.7),
                };
                
                let history_item = Column::new()
                    .spacing(4)
                    .push(
                        Row::new()
                            .spacing(5)
                            .push(
                                Text::new(status_icon)
                                    .style(move |_theme| {
                                        iced::widget::text::Style {
                                            color: Some(status_color),
                                        }
                                    })
                            )
                            .push(Text::new(&r.file_name).size(12))
                    )
                    .push(
                        Text::new(r.timestamp.format("%m/%d %H:%M").to_string())
                            .size(10)
                            .style(|_theme| {
                                iced::widget::text::Style {
                                    color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                                }
                            })
                    );
                
                history_list = history_list.push(history_item);
                if i < 4 && i < self.past.len() - 1 {
                    history_list = history_list.push(rule::Rule::horizontal(1));
                }
            }

            let history_scrollable = Scrollable::new(history_list)
                .height(Length::Fixed(200.0));

            sidebar_content = sidebar_content.push(history_scrollable);
        }

        let sidebar_bg = self.sidebar_bg_color();
        Container::new(sidebar_content)
            .height(Length::Fill)
            .style(move |_theme| {
                iced::widget::container::Style {
                    background: Some(iced::Background::Color(sidebar_bg)),
                    border: Border::default(),
                    ..Default::default()
                }
            })
            .into()
    }

    fn view_upload_file(&self) -> Element<'_, Message> {
        let step_indicator = self.step_indicator(1);

        let text_color = self.text_color();
        let secondary_text = self.secondary_text_color();
        let container_bg = self.container_bg_color();
        let border_color = self.border_color();

        let title = Text::new("Step 1: Select File")
            .size(32)
            .style(move |_theme| {
                iced::widget::text::Style {
                    color: Some(text_color),
                }
            });

        let description = Text::new("Choose the file you want to verify")
            .size(16)
            .style(move |_theme| {
                iced::widget::text::Style {
                    color: Some(secondary_text),
                }
            });

        let file_display = if let Some(path) = &self.chosen_file {
            let file_text_color = self.text_color();
            Column::new()
                .spacing(10)
                .push(
                    Text::new("Selected File:")
                        .size(14)
                        .style(move |_theme| {
                            iced::widget::text::Style {
                                color: Some(secondary_text),
                            }
                        })
                )
                .push(
                    Container::new(
                        Text::new(path.display().to_string())
                            .size(16)
                            .style(move |_theme| {
                                iced::widget::text::Style {
                                    color: Some(file_text_color),
                                }
                            })
                    )
                    .padding(15)
                    .width(Length::Fill)
                    .style(move |_theme| {
                        iced::widget::container::Style {
                            background: Some(iced::Background::Color(container_bg)),
                            border: Border {
                                color: border_color,
                                width: 1.0,
                                radius: 4.0.into(),
                            },
                            ..Default::default()
                        }
                    })
                )
        } else {
            let tertiary_text = self.tertiary_text_color();
            Column::new()
                .spacing(10)
                .push(
                    Text::new("No file selected")
                        .size(16)
                        .style(move |_theme| {
                            iced::widget::text::Style {
                                color: Some(tertiary_text),
                            }
                        })
                )
        };

        let browse_btn = Button::new(
            Text::new("Browse Files")
                .size(18)
        )
        .on_press(Message::ChooseFile)
        .padding(15)
        .width(Length::Fixed(200.0));

        let next_btn = if self.chosen_file.is_some() {
            Button::new(
                Text::new("Next: Upload Hash →")
                    .size(16)
            )
            .on_press(Message::ProceedToHash)
            .padding(15)
            .width(Length::Fixed(200.0))
        } else {
            Button::new(
                Text::new("Next: Upload Hash →")
                    .size(16)
            )
            .padding(15)
            .width(Length::Fixed(200.0))
        };

        let content = Column::new()
            .padding(40)
            .spacing(30)
            .width(Length::Fill)
            .push(step_indicator)
            .push(title)
            .push(description)
            .push(Space::with_height(20))
            .push(file_display)
            .push(browse_btn)
            .push(Space::with_height(40))
            .push(next_btn);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .into()
    }

    fn view_upload_hash(&self) -> Element<'_, Message> {
        let step_indicator = self.step_indicator(2);

        let text_color = self.text_color();
        let secondary_text = self.secondary_text_color();
        let tertiary_text = self.tertiary_text_color();

        let title = Text::new("Step 2: Enter Reference Hash")
            .size(32)
            .style(move |_theme| {
                iced::widget::text::Style {
                    color: Some(text_color),
                }
            });

        let description = Text::new("Paste or upload the expected hash value (optional)")
            .size(16)
            .style(move |_theme| {
                iced::widget::text::Style {
                    color: Some(secondary_text),
                }
            });

        let hash_input = TextInput::new(
            "Paste hash here (e.g., abc123def456...)",
            &self.paste_hash,
        )
        .on_input(Message::PasteHashChanged)
        .padding(15)
        .size(16)
        .width(Length::Fill);

        let load_file_btn = Button::new(
            Text::new("📁 Load Hash from File")
                .size(14)
        )
        .on_press(Message::LoadHashFile)
        .padding(12);

        let note = Text::new("Note: If no reference hash is provided, only the computed hash will be shown")
            .size(12)
            .style(move |_theme| {
                iced::widget::text::Style {
                    color: Some(tertiary_text),
                }
            })
            .style(|_theme| {
                iced::widget::text::Style {
                    color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                }
            });

        let back_btn = Button::new(
            Text::new("← Back")
                .size(16)
        )
        .on_press(Message::ResetVerification)
        .padding(15)
        .width(Length::Fixed(150.0));

        let verify_btn = Button::new(
            Text::new("Verify Now")
                .size(16)
        )
        .on_press(Message::StartVerify)
        .padding(15)
        .width(Length::Fixed(200.0));

        let button_row = Row::new()
            .spacing(20)
            .push(back_btn)
            .push(verify_btn);

        let content = Column::new()
            .padding(40)
            .spacing(30)
            .width(Length::Fill)
            .push(step_indicator)
            .push(title)
            .push(description)
            .push(Space::with_height(20))
            .push(hash_input)
            .push(load_file_btn)
            .push(note)
            .push(Space::with_height(40))
            .push(button_row);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .into()
    }

    fn view_verifying(&self) -> Element<'_, Message> {
        let step_indicator = self.step_indicator(3);

        let text_color = self.text_color();
        let secondary_text = self.secondary_text_color();
        let accent = self.accent_color();

        let title = Text::new("Verifying...")
            .size(32)
            .style(move |_theme| {
                iced::widget::text::Style {
                    color: Some(text_color),
                }
            });

        let description = Text::new(&self.status_message)
            .size(18)
            .style(move |_theme| {
                iced::widget::text::Style {
                    color: Some(secondary_text),
                }
            });

        let spinner = Text::new("⟳")
            .size(64)
            .style(move |_theme| {
                iced::widget::text::Style {
                    color: Some(accent),
                }
            });

        let content = Column::new()
            .padding(40)
            .spacing(30)
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .push(step_indicator)
            .push(title)
            .push(spinner)
            .push(description);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    fn view_result(&self) -> Element<'_, Message> {
        let step_indicator = self.step_indicator(3);

        let (title, title_color, icon) = if let Some(rec) = &self.last_result {
            match rec.status {
                VerificationStatus::Success => (
                    "Verification Successful!",
                    Color::from_rgb(0.2, 0.7, 0.2),
                    "✓"
                ),
                VerificationStatus::Failed => (
                    "Verification Failed",
                    Color::from_rgb(0.9, 0.2, 0.2),
                    "✗"
                ),
                VerificationStatus::InProgress => (
                    "In Progress",
                    Color::from_rgb(0.5, 0.5, 0.5),
                    "⋯"
                ),
            }
        } else {
            ("Error", Color::from_rgb(0.9, 0.2, 0.2), "✗")
        };

        let icon_text = Text::new(icon)
            .size(80)
            .style(move |_theme| {
                iced::widget::text::Style {
                    color: Some(title_color),
                }
            });

        let title_text = Text::new(title)
            .size(36)
            .style(move |_theme| {
                iced::widget::text::Style {
                    color: Some(title_color),
                }
            });

        let text_color = self.text_color();
        let secondary_text = self.secondary_text_color();
        let container_bg = self.container_bg_color();
        let border_color = self.border_color();

        let mut details = Column::new().spacing(15).width(Length::Fill);

        if let Some(rec) = &self.last_result {
            details = details
                .push(
                    Column::new()
                        .spacing(5)
                        .push(Text::new("File:").size(14).style(move |_theme| {
                            iced::widget::text::Style {
                                color: Some(secondary_text),
                            }
                        }))
                        .push(Text::new(&rec.file_name).size(16).style(move |_theme| {
                            iced::widget::text::Style {
                                color: Some(text_color),
                            }
                        }))
                )
                .push(Space::with_height(5))
                .push(
                    Column::new()
                        .spacing(5)
                        .push(Text::new("Algorithm:").size(14).style(move |_theme| {
                            iced::widget::text::Style {
                                color: Some(secondary_text),
                            }
                        }))
                        .push(Text::new(rec.algorithm.name()).size(16).style(move |_theme| {
                            iced::widget::text::Style {
                                color: Some(text_color),
                            }
                        }))
                )
                .push(Space::with_height(5))
                .push(
                    Column::new()
                        .spacing(5)
                        .push(Text::new("Computed Hash:").size(14).style(move |_theme| {
                            iced::widget::text::Style {
                                color: Some(secondary_text),
                            }
                        }))
                        .push(
                            Container::new(Text::new(&rec.computed_hash).size(14).style(move |_theme| {
                                iced::widget::text::Style {
                                    color: Some(text_color),
                                }
                            }))
                                .padding(10)
                                .width(Length::Fill)
                                .style(move |_theme| {
                                    iced::widget::container::Style {
                                        background: Some(iced::Background::Color(container_bg)),
                                        border: Border {
                                            color: border_color,
                                            width: 1.0,
                                            radius: 4.0.into(),
                                        },
                                        ..Default::default()
                                    }
                                })
                        )
                );

            if let Some(ref_hash) = &rec.reference_hash {
                details = details
                    .push(Space::with_height(5))
                    .push(
                        Column::new()
                            .spacing(5)
                            .push(Text::new("Reference Hash:").size(14).style(move |_theme| {
                                iced::widget::text::Style {
                                    color: Some(secondary_text),
                                }
                            }))
                            .push(
                                Container::new(Text::new(ref_hash).size(14).style(move |_theme| {
                                    iced::widget::text::Style {
                                        color: Some(text_color),
                                    }
                                }))
                                    .padding(10)
                                    .width(Length::Fill)
                                    .style(move |_theme| {
                                        iced::widget::container::Style {
                                            background: Some(iced::Background::Color(container_bg)),
                                            border: Border {
                                                color: border_color,
                                                width: 1.0,
                                                radius: 4.0.into(),
                                            },
                                            ..Default::default()
                                        }
                                    })
                            )
                    );
            }
        }

        let new_verification_btn = Button::new(
            Text::new("New Verification")
                .size(16)
        )
        .on_press(Message::ResetVerification)
        .padding(15)
        .width(Length::Fixed(200.0));

        let content = Column::new()
            .padding(40)
            .spacing(25)
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .push(step_indicator)
            .push(icon_text)
            .push(title_text)
            .push(Space::with_height(20))
            .push(details)
            .push(Space::with_height(30))
            .push(new_verification_btn);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .into()
    }

    fn step_indicator(&self, current: u8) -> Element<'_, Message> {
        let step1_color = if current >= 1 { Color::from_rgb(0.2, 0.5, 0.8) } else { Color::from_rgb(0.7, 0.7, 0.7) };
        let step2_color = if current >= 2 { Color::from_rgb(0.2, 0.5, 0.8) } else { Color::from_rgb(0.7, 0.7, 0.7) };
        let step3_color = if current >= 3 { Color::from_rgb(0.2, 0.5, 0.8) } else { Color::from_rgb(0.7, 0.7, 0.7) };

        let step1 = Text::new("1. Upload File")
            .size(14)
            .style(move |_theme| {
                iced::widget::text::Style {
                    color: Some(step1_color),
                }
            });

        let step2 = Text::new("2. Enter Hash")
            .size(14)
            .style(move |_theme| {
                iced::widget::text::Style {
                    color: Some(step2_color),
                }
            });

        let step3 = Text::new("3. Result")
            .size(14)
            .style(move |_theme| {
                iced::widget::text::Style {
                    color: Some(step3_color),
                }
            });

        let arrow1 = Text::new("→")
            .size(14)
            .style(|_theme| {
                iced::widget::text::Style {
                    color: Some(Color::from_rgb(0.7, 0.7, 0.7)),
                }
            });

        let arrow2 = Text::new("→")
            .size(14)
            .style(|_theme| {
                iced::widget::text::Style {
                    color: Some(Color::from_rgb(0.7, 0.7, 0.7)),
                }
            });

        Row::new()
            .spacing(10)
            .push(step1)
            .push(arrow1)
            .push(step2)
            .push(arrow2)
            .push(step3)
            .into()
    }

    fn get_algorithm_description(&self) -> &'static str {
        match self.algorithm {
            Algorithm::Blake3 => "Fast, secure cryptographic hash (Recommended)",
            Algorithm::Sha256 => "Industry standard, widely used",
            Algorithm::Sha512 => "Higher security, larger output",
            Algorithm::Sha3_256 => "Latest SHA-3 standard",
            Algorithm::Md5 => "Legacy, not recommended for security",
        }
    }

    // Theme color helpers
    fn bg_color(&self) -> Color {
        match self.theme {
            Theme::Light => Color::from_rgb(1.0, 1.0, 1.0),
            Theme::Dark => Color::from_rgb(0.11, 0.11, 0.13),
        }
    }

    fn sidebar_bg_color(&self) -> Color {
        match self.theme {
            Theme::Light => Color::from_rgb(0.95, 0.95, 0.97),
            Theme::Dark => Color::from_rgb(0.15, 0.15, 0.17),
        }
    }

    fn text_color(&self) -> Color {
        match self.theme {
            Theme::Light => Color::from_rgb(0.1, 0.1, 0.1),
            Theme::Dark => Color::from_rgb(0.9, 0.9, 0.9),
        }
    }

    fn secondary_text_color(&self) -> Color {
        match self.theme {
            Theme::Light => Color::from_rgb(0.4, 0.4, 0.4),
            Theme::Dark => Color::from_rgb(0.6, 0.6, 0.6),
        }
    }

    fn tertiary_text_color(&self) -> Color {
        match self.theme {
            Theme::Light => Color::from_rgb(0.5, 0.5, 0.5),
            Theme::Dark => Color::from_rgb(0.5, 0.5, 0.5),
        }
    }

    fn container_bg_color(&self) -> Color {
        match self.theme {
            Theme::Light => Color::from_rgb(0.95, 0.95, 0.95),
            Theme::Dark => Color::from_rgb(0.2, 0.2, 0.22),
        }
    }

    fn border_color(&self) -> Color {
        match self.theme {
            Theme::Light => Color::from_rgb(0.8, 0.8, 0.8),
            Theme::Dark => Color::from_rgb(0.3, 0.3, 0.32),
        }
    }

    fn accent_color(&self) -> Color {
        Color::from_rgb(0.2, 0.5, 0.8)
    }
}
