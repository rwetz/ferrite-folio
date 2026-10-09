#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod books;
use ferrite_design::prelude::*;
use gpui::{App, AppContext as _, ClickEvent, Context, Entity, IntoElement, KeyBinding, Render, Subscription, Task, Window, div, px, size};

struct Folio {
    palette: Entity<CommandPalette>,
    _appearance: Subscription,
    query: Entity<TextInput>,
    _query: Subscription,
    books: Vec<books::Book>,
    pages: Vec<String>,
    page: usize,
    title: String,
    status: String,
    job: Option<Task<()>>,
}
impl Folio {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| TextInput::new(window, cx).placeholder("Search title or author, then Enter"));
        let subscription = cx.subscribe(&query, |this: &mut Self, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Submit) {
                this.refresh(cx);
            }
        });
        let mut app = Self {
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
        };
        let weak = cx.weak_entity();
        app.palette.update(cx, |palette, cx| {
            let mut commands = Vec::new();
            for scheme in SCHEMES {
                commands
                    .push(command(format!("Scheme: {}", scheme.name)).group("Theme").on_run(move |_, cx| theme::set_scheme(scheme, cx)));
            }
            commands.push(command("Toggle light / dark").group("Theme").on_run(|window, cx| {
                let appearance = if theme::appearance(cx) == Appearance::Light { Appearance::Dark } else { Appearance::Light };
                theme::set_appearance(appearance, window, cx);
            }));
            commands.push(command("Search books").group("Folio").on_run(move |_, cx| {
                let _ = weak.update(cx, |this, cx| this.refresh(cx));
            }));
            palette.set_commands(commands, cx);
        });
        app.refresh(cx);
        app
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let query = self.query.read(cx).value().to_string();
        self.status = "Searching Gutendex…".into();
        self.job = Some(cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { books::search(&query) }).await;
            let _ = this.update(cx, |this, cx| {
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
        self.status = "Downloading text…".into();
        self.job = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let pages = books::download(&book)?;
                    Ok::<_, String>((book.title, pages))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
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

        let mut content: Vec<gpui::AnyElement> = vec![
            div()
                .flex()
                .gap_3()
                .child(self.query.clone())
                .child(Button::new("search").label("Search").primary().on_click(cx.listener(|this, _, _, cx| this.refresh(cx))))
                .into_any_element(),
        ];
        if self.pages.is_empty() {
            for book in &self.books {
                let selected = book.clone();
                content.push(
                    panel(book.title.clone())
                        .meta(book.author.clone())
                        .child(
                            Button::new(("read", book.id))
                                .label("Read text")
                                .secondary()
                                .disabled(book.text_url.is_none())
                                .on_click(cx.listener(move |this, _, _, cx| this.read(selected.clone(), cx))),
                        )
                        .into_any_element(),
                );
            }
        } else {
            content.push(
                div()
                    .flex()
                    .gap_3()
                    .child(Button::new("previous").label("Previous").secondary().disabled(self.page == 0).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.page = this.page.saturating_sub(1);
                            cx.notify();
                        },
                    )))
                    .child(div().child(format!("Page {} / {}", self.page + 1, self.pages.len())))
                    .child(Button::new("next").label("Next").secondary().disabled(self.page + 1 >= self.pages.len()).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.page = (this.page + 1).min(this.pages.len().saturating_sub(1));
                            cx.notify();
                        },
                    )))
                    .into_any_element(),
            );
            content.push(
                panel(self.title.clone())
                    .child(div().max_w(px(760.)).p_4().body(text::BASE).child(self.pages[self.page].clone()))
                    .into_any_element(),
            );
        }

        let menu =
            Button::new("commands").icon(Icon::Menu).ghost().small().tooltip("Commands · Ctrl+Shift+P").on_click(
                cx.listener(|this, _: &ClickEvent, window, cx| this.palette.update(cx, |palette, cx| palette.toggle(window, cx))),
            );
        window_frame().child(power_on_in(
            "power",
            div()
                .flex()
                .flex_col()
                .size_full()
                .bg(hsla(p.bg))
                .text_color(hsla(p.fg))
                .body(text::BASE)
                .on_action(
                    cx.listener(|this, _: &TogglePalette, window, cx| this.palette.update(cx, |palette, cx| palette.toggle(window, cx))),
                )
                .child(self.palette.clone())
                .child(title_bar("Folio").child(menu))
                .child(div().id("content").flex().flex_col().flex_1().min_h_0().overflow_y_scroll().gap_3().p_4().children(content))
                .child(status_bar().left_live(self.status.clone()).right(theme::scheme(cx).name.to_uppercase())),
        ))
    }
}
fn smoke() -> Result<(), String> {
    let results = books::search("Pride and Prejudice")?;
    let book = results.into_iter().find(|book| book.text_url.is_some()).ok_or("No readable book returned")?;
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
        ferrite_design::init(Appearance::Dark, cx);
        theme::apply_env(cx);
        cx.bind_keys([KeyBinding::new("ctrl-shift-p", TogglePalette, None)]);
        let options = chrome::window_options("Folio", size(px(1100.), px(800.)), cx);
        cx.open_window(options, |window, cx| {
            chrome::square_corners(window);
            chrome::power_off_on_close(window, cx);
            cx.new(|cx| Folio::new(window, cx))
        })
        .expect("failed to open window");
        cx.activate(true);
    });
}
