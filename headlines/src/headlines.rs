use crate::fetch_news;
#[cfg(target_arch = "wasm32")]
use crate::fetch_web;
use serde::{ Serialize, Deserialize };
use std::sync::mpsc::{ Receiver, Sender, channel, sync_channel, SyncSender };
use eframe::egui::{
    Window,
    Color32,
    RichText,
    Layout,
    Vec2,
    Button,
    TopBottomPanel,
    TextStyle,
};

pub const PADDING: f32 = 5.0;
const WHITE: Color32 = Color32::from_rgb(255, 255, 255);
const BLACK: Color32 = Color32::from_rgb(0, 0, 0);
const RED: Color32 = Color32::from_rgb(255, 0, 0);
const CYAN: Color32 = Color32::from_rgb(0, 255, 255);

pub enum Msg {
    ApiKeySet(String)
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct HeadlinesConfig {
    pub dark_mode: bool,
    pub api_key: String
}

#[derive(Debug)]
pub struct NewsCardData {
    pub title: String,
    pub url: String,
    pub description: String
}

#[derive(Default)]
pub struct Headlines {
    pub articles: Vec<NewsCardData>,
    pub config: HeadlinesConfig,
    pub api_key_initialized: bool,
    pub news_rx: Option<Receiver<NewsCardData>>,
    pub news_tx: Option<Sender<NewsCardData>>,
    pub app_tx: Option<SyncSender<Msg>>
}

/// This function has been taken as is from the egui examples
/// Refer: https://github.com/emilk/egui/blob/7eeb292adfacd9311a420ac3ea225e2261a8f8d3/examples/custom_font/src/main.rs#L14
fn setup_custom_fonts(ctx: &eframe::egui::Context) {
    // Start with the default fonts (we will be adding to them rather than replacing them).
    let mut fonts = eframe::egui::FontDefinitions::default();

    // Install my own font (maybe supporting non-latin characters).
    // .ttf and .otf files supported.
    fonts.font_data.insert(
        "my_font".to_owned(),
        eframe::egui::FontData::from_static(include_bytes!("../MesloLGS_NF_Regular.ttf")),
    );

    // Put my font first (highest priority) for proportional text:
    fonts
        .families
        .entry(eframe::egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "my_font".to_owned());

    // Put my font as last fallback for monospace:
    fonts
        .families
        .entry(eframe::egui::FontFamily::Monospace)
        .or_default()
        .push("my_font".to_owned());

    // Tell egui to use these fonts:
    ctx.set_fonts(fonts);
}

impl Headlines {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_custom_fonts(&cc.egui_ctx);

        let mut config = HeadlinesConfig::default();

        if let Some(storage) = cc.storage {
            config = eframe::get_value(storage, "headlines").unwrap_or_default();
        }

        let articles: Vec<NewsCardData> = Vec::new();

        let api_key = config.api_key.to_string();
        let (news_tx, news_rx) = channel();
        let news_tx_ = news_tx.clone();
        let (app_tx, app_rx) = sync_channel(1);

        #[cfg(not(target_arch="wasm32"))]
        std::thread::spawn(move || {
            if !api_key.is_empty() {
                fetch_news(&api_key, &news_tx);
            } else {
                tracing::debug!("here");
                loop {
                    tracing::debug!("herehere");
                    match app_rx.recv() {
                        Ok(Msg::ApiKeySet(api_key)) => {
                            tracing::info!("received api_key msg!");
                            fetch_news(&api_key, &news_tx);
                        },
                        Err(e) => {
                            tracing::error!("failed receiving message: {}", e);
                        }
                    }
                }
            }
        });

        #[cfg(target_arch="wasm32")]
        let api_key_web = config.api_key.clone();
        #[cfg(target_arch="wasm32")]
        let news_tx_web = news_tx_.clone();
        #[cfg(target_arch="wasm32")]
        gloo_timers::callback::Timeout::new(10, move || {
            wasm_bindgen_futures::spawn_local(async {
                fetch_web(api_key_web, news_tx_web).await;
            })
        }).forget();

        #[cfg(target_arch="wasm32")]
        let news_tx_web_ = news_tx_.clone();
        #[cfg(target_arch="wasm32")]
        gloo_timers::callback::Interval::new(500, move || {
            match app_rx.try_recv() {
                Ok(Msg::ApiKeySet(api_key)) => {
                    wasm_bindgen_futures::spawn_local(fetch_web(api_key.clone(), news_tx_web_.clone()));
                }
                Err(e) => {
                    tracing::error!("failed receiving msg: {}", e);
                }
            }
        }).forget();

        Headlines {
            api_key_initialized: !config.api_key.is_empty(),
            articles,
            config,
            news_rx: Some(news_rx),
            news_tx: Some(news_tx_),
            app_tx: Some(app_tx)
        }
    }
  
    pub fn render_news_cards(&self, ui: &mut eframe::egui::Ui) {
        for a in &self.articles {
            ui.add_space(PADDING);

            let title = format!("> {}", a.title);
            if self.config.dark_mode {
                ui.colored_label(WHITE, title);
            } else {
                ui.colored_label(BLACK, title);
            }

            ui.add_space(PADDING);
            let description = RichText::new(&a.description).text_style(eframe::egui::TextStyle::Button);
            ui.label(description);
            
            if self.config.dark_mode {
                ui.style_mut().visuals.hyperlink_color = CYAN;
            } else {
                ui.style_mut().visuals.hyperlink_color = RED;
            }

            ui.add_space(PADDING);
            ui.allocate_ui_with_layout( Vec2::new(ui.available_width(), 0.0), Layout::right_to_left(), |ui| {
                ui.hyperlink_to("read more...", &a.url);
            });
            ui.add_space(PADDING);
            ui.separator();
        }
    }

    pub fn render_top_panel(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(10.);
            eframe::egui::menu::bar(ui, |ui| {
                ui.with_layout(Layout::left_to_right(), |ui| {
                    ui.label(RichText::new("((.))").text_style(TextStyle::Heading));
                });
                ui.with_layout(Layout::right_to_left(), |ui| {
                    let close_btn = ui.add(Button::new(RichText::new("X").text_style(TextStyle::Body)));
                    if close_btn.clicked() {
                        frame.quit()
                    }
                    let refresh_btn = ui.add(Button::new(RichText::new("r").text_style(TextStyle::Body)));

                    if refresh_btn.clicked() {
                        tracing::info!("Refreshing article list.");
                        self.articles = vec![];
                        if let Some(tx) = &self.news_tx {
                            let tx_ = tx.clone();
                            let api_key = self.config.api_key.clone();

                            #[cfg(not(target_arch="wasm32"))]
                            std::thread::spawn(move || {
                                // Putting a sleep here to test that the UI gets repainted even as
                                // there is a network delay in fetching the data.
                                std::thread::sleep(std::time::Duration::from_millis(2000));
                                fetch_news(&api_key, &tx_);
                            });

                            #[cfg(target_arch="wasm32")]
                            gloo_timers::callback::Timeout::new(2000, move || {
                                wasm_bindgen_futures::spawn_local(async {
                                    fetch_web(api_key, tx_).await;
                                })
                            }).forget();
                        }
                    }

                    let theme_btn = ui.add(Button::new(RichText::new("@").text_style(TextStyle::Body)));
                    if theme_btn.clicked() {
                        tracing::info!("Changing theme.");
                        self.config.dark_mode = !self.config.dark_mode;
                    }
                });
            });
            ui.add_space(10.);
        });
    }

    pub fn render_config(&mut self, ctx: &eframe::egui::Context) {
        Window::new("Configuration").show(ctx, |ui| {
            ui.label("Enter your API_KEY for newsapi.org");
            let text_input = ui.text_edit_singleline(&mut self.config.api_key);
            if text_input.lost_focus() && ui.input().key_pressed(eframe::egui::Key::Enter) {
                self.api_key_initialized = true;

                if let Some(tx) = &self.app_tx {
                    let _ = tx.send(Msg::ApiKeySet(self.config.api_key.to_string()));
                };

                tracing::info!("API key set");
            }
            ui.label("If you don't have an API key, create one at");
            ui.hyperlink("https://newsapi.org");
        });
    }

    pub fn preload_articles(&mut self) {
        if let Some(rx) = &self.news_rx {
            match rx.try_recv() {
                Ok(news) => {
                    self.articles.push(news);
                },
                // Err(_) => {}
                Err(e) => {
                    tracing::warn!("Error receiving msg: {}", e);
                }
            }
        }
    }
}

