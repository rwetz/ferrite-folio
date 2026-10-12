#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod books;
mod prefs;
mod prefs_store;
mod probe;
mod settings_ui;
mod worker;
use ferrite_design::prelude::*;
use gpui::{
    App, AppContext as _, ClickEvent, Context, Entity, IntoElement, KeyBinding, Render,
    Subscription, Task, Window, div, px, size,
};

struct Folio {
    palette: Entity<CommandPalette>,
    settings: Entity<settings_ui::Settings>,
    _appearance: Subscription,
    query: Entity<TextInput>,
    _query: Subscription,
    books: Vec<books::Book>,
    pages: Vec<String>,
    page: usize,
    title: String,
    status: String,
    job: Option<Task<()>>,
    busy: bool,
}
impl Folio {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx
            .new(|cx| TextInput::new(window, cx).placeholder("Search title or author, then Enter"));
        let subscription = cx.subscribe(&query, |this: &mut Self, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Submit) {
                this.refresh(cx);
            }
        });
        let mut app = Self {
            settings: cx.new(|_| settings_ui::Settings::new()),
            palette: cx.new(|cx| CommandPalette::new(window, cx)),
            _appearance: theme::follow_system(window),
            query,
            _query: subscription,
            books: Vec::new(),
            pages: Vec::new(),
            page: 0,
            title: "Library".into(),
            status: "Ready".into(),
            job: None,
            busy: false,
        };
        let weak = cx.weak_entity();
        app.palette.update(cx, |palette, cx| {
            let mut commands = Vec::new();
            for scheme in SCHEMES {
                commands.push(
                    command(format!("Scheme: {}", scheme.name))
                        .group("Theme")
                        .on_run(move |_, cx| prefs::scheme(scheme, cx)),
                );
            }
            commands.push(
                command("Toggle light / dark")
                    .group("Theme")
                    .on_run(|window, cx| {
                        let appearance = if theme::appearance(cx) == Appearance::Light {
                            Appearance::Dark
                        } else {
                            Appearance::Light
                        };
                        prefs::appearance(appearance, window, cx);
                    }),
            );
            commands.push(command("Search books").group("Folio").on_run(move |_, cx| {
                let _ = weak.update(cx, |this, cx| this.refresh(cx));
            }));
            commands.push(settings_ui::settings_command(app.settings.clone()));
            palette.set_commands(commands, cx);
        });
        app.refresh(cx);
        app
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let query = self.query.read(cx).value().to_string();
        self.status = "Searching the book catalogue…".into();
        self.busy = true;
        self.job = Some(cx.spawn(async move |this, cx| {
            let result = worker::run(move || books::search(&query)).await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(books) => {
                        this.status = format!("{} books found", books.len());
                        this.books = books;
                        this.pages.clear();
                    }
                    Err(error) => this.status = error,
                };
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn read(&mut self, book: books::Book, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.status = "Downloading text…".into();
        self.busy = true;
        self.job = Some(cx.spawn(async move |this, cx| {
            let result = worker::run(move || {
                let pages = books::download(&book)?;
                Ok::<_, String>((book.title, pages))
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok((title, pages)) => {
                        this.title = title;
                        this.pages = pages;
                        this.page = 0;
                        this.status = "Reading".into();
                    }
                    Err(error) => this.status = error,
                };
                cx.notify();
            });
        }));
        cx.notify();
    }
}
impl Render for Folio {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        if !self.busy && !self.books.is_empty() {
            probe::record(format!("Library: {} books", self.books.len()));
        }

        let mut content: Vec<gpui::AnyElement> = vec![
            div()
                .flex()
                .gap_3()
                .child(self.query.clone())
                .child(
                    Button::new("search")
                        .label("Search")
                        .primary()
                        .disabled(self.busy)
                        .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                )
                .into_any_element(),
        ];
        content.push(
            div()
                .p_3()
                .bg(hsla(p.surface))
                .text_color(hsla(p.fg_dim))
                .child(self.status.clone())
                .into_any_element(),
        );
        if self.pages.is_empty() {
            if self.books.is_empty() && !self.busy {
                content.push(panel("Library").child(div().p_4().child("Search a title or author, or leave the search empty to browse classics.")).into_any_element());
            }
            for book in &self.books {
                let selected = book.clone();
                content.push(
                    panel("Book")
                        .meta(format!("#{}", book.id))
                        .child(div().body(text::BASE).child(book.title.clone()))
                        .child(div().text_color(hsla(p.fg_dim)).child(book.author.clone()))
                        .child(
                            Button::new(("read", book.id))
                                .label("Read text")
                                .secondary()
                                .disabled(self.busy || book.text_url.is_none())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.read(selected.clone(), cx)
                                })),
                        )
                        .into_any_element(),
                );
            }
        } else {
            content.push(
                div()
                    .flex()
                    .gap_3()
                    .items_center()
                    .flex_wrap()
                    .child(
                        Button::new("library")
                            .label("Library")
                            .secondary()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.pages.clear();
                                this.status = format!("{} books found", this.books.len());
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("previous")
                            .label("Previous")
                            .secondary()
                            .disabled(self.page == 0)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.page = this.page.saturating_sub(1);
                                cx.notify();
                            })),
                    )
                    .child(div().child(format!("Page {} / {}", self.page + 1, self.pages.len())))
                    .child(
                        Button::new("next")
                            .label("Next")
                            .secondary()
                            .disabled(self.page + 1 >= self.pages.len())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.page = (this.page + 1).min(this.pages.len().saturating_sub(1));
                                cx.notify();
                            })),
                    )
                    .into_any_element(),
            );
            content.push(
                panel("Reading")
                    .child(
                        div()
                            .body(text::BASE)
                            .text_color(hsla(p.accent))
                            .child(self.title.clone()),
                    )
                    .child(
                        div().w_full().flex().justify_center().child(
                            div()
                                .w_full()
                                .max_w(px(760.))
                                .p_4()
                                .body(text::BASE)
                                .child(self.pages[self.page].clone()),
                        ),
                    )
                    .into_any_element(),
            );
        }

        let menu = Button::new("commands")
            .icon(Icon::Menu)
            .ghost()
            .small()
            .tooltip("Commands · Ctrl+Shift+P")
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.palette
                    .update(cx, |palette, cx| palette.toggle(window, cx))
            }));
        window_frame().child(power_on_in(
            "power",
            div()
                .flex()
                .flex_col()
                .size_full()
                .bg(hsla(p.bg))
                .text_color(hsla(p.fg))
                .body(text::BASE)
                .on_action(cx.listener(|this, _: &TogglePalette, window, cx| {
                    this.palette
                        .update(cx, |palette, cx| palette.toggle(window, cx))
                }))
                .on_action(cx.listener(|this, _: &settings_ui::ToggleSettings, _, cx| {
                    this.settings.update(cx, |s, cx| s.toggle(cx));
                }))
                .child(self.settings.clone())
                .child(self.palette.clone())
                .child(
                    title_bar("Folio")
                        .child(settings_ui::button(self.settings.clone()))
                        .child(menu),
                )
                .child(
                    // A visible bar, so a long page reads as "there's more".
                    scroll_area("content").flex_1().min_h_0().child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .p_4()
                            .children(content),
                    ),
                )
                .child(
                    status_bar()
                        .left_live(self.status.clone())
                        .right(theme::scheme(cx).name.to_uppercase()),
                ),
        ))
    }
}
fn smoke() -> Result<(), String> {
    let results = books::search("Pride and Prejudice")?;
    let book = results
        .into_iter()
        .find(|book| book.text_url.is_some())
        .ok_or("No readable book returned")?;
    let pages = books::download(&book)?;
    println!("Gutenberg: {} ({} pages)", book.title, pages.len());
    Ok(())
}

fn main() {
    if std::env::args().any(|arg| arg == "--version") {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return;
    }
    if std::env::args().any(|arg| arg == "--smoke") {
        if let Err(error) = smoke() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    gpui_platform::application().run(|cx: &mut App| {
        prefs::init(cx);
        cx.bind_keys([
            KeyBinding::new("ctrl-shift-p", TogglePalette, None),
            KeyBinding::new("ctrl-,", settings_ui::ToggleSettings, None),
        ]);
        let options =
            chrome::remembered_window_options("folio", "Folio", size(px(1100.), px(800.)), cx);
        cx.open_window(options, |window, cx| {
            chrome::square_corners(window);
            chrome::remember_window("folio", window, cx);
            chrome::power_off_on_close(window, cx);
            cx.new(|cx| Folio::new(window, cx))
        })
        .expect("failed to open window");
        cx.activate(true);
    });
}
