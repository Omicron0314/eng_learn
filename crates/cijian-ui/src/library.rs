use super::*;
use crate::design::{self, Icon};
use crate::stats::study_date;
use chrono::Datelike;
use cijian_domain::srs::State;
use cijian_ports::library::{BookEntryPage, BookView, DeletePreview};
use gpui_kit::{
    Entity, FocusHandle,
    base::input::{InputState, TextareaState},
};
use gpui_kit_material::components::{
    MaterialDialog, MaterialMenu, MaterialMenuItem, MenuGroup, MenuPopup, TextField,
};
#[derive(Default)]
pub(super) struct LibraryState {
    pub books: Vec<BookView>,
    pub entries: Option<BookEntryPage>,
    pub scope: Option<(Option<String>, String)>,
    pub page: u32,
    pub state: Option<State>,
    pub suspended: Option<bool>,
    pub search: Option<Entity<InputState>>,
    pub applied_search: Option<String>,
    pub import_name: Option<Entity<InputState>>,
    pub file_handle: Option<String>,
    pub target: Option<String>,
    pub editor: Option<CardEditor>,
    pub deletion: Option<DeletePreview>,
    pub discard: bool,
    pub pending_route: Option<Route>,
    pub pending_close: bool,
    pub pending_back: bool,
    pub import_open: bool,
    pub report_open: bool,
    pub suspended_open: bool,
    pub suspended_focus: Option<FocusHandle>,
    pub import_source: Option<String>,
    /// Set while the report shown belongs to the import that just ran, so a
    /// background library reload cannot swap in an older stored report.
    pub report_pinned: bool,
    pub error_page: usize,
}
pub(super) struct CardEditor {
    pub card: CardView,
    meaning: Entity<TextareaState>,
    example: Entity<TextareaState>,
}
impl CardEditor {
    pub(super) fn new(card: CardView, window: &mut Window, cx: &mut Context<Shell>) -> Self {
        let meaning =
            cx.new(|cx| TextareaState::new(window, cx).default_value(card.meaning.clone()));
        let example =
            cx.new(|cx| TextareaState::new(window, cx).default_value(card.example.clone()));
        cx.observe(&meaning, |_, _, cx| cx.notify()).detach();
        cx.observe(&example, |_, _, cx| cx.notify()).detach();
        Self {
            card,
            meaning,
            example,
        }
    }
    pub fn dirty(&self, cx: &App) -> bool {
        self.meaning.read(cx).value().as_ref() != self.card.meaning
            || self.example.read(cx).value().as_ref() != self.card.example
    }
}
fn state_label(state: State) -> &'static str {
    match state {
        State::New => "新词",
        State::Learning => "学习中",
        State::Review => "复习",
        State::Relearning => "重新学习",
    }
}
impl Shell {
    pub(super) fn go_back(&mut self, cx: &mut Context<Self>) {
        if !self.navigation.can_back() {
            return;
        }
        let mut target = self.navigation.clone();
        target.back();
        self.navigate_to(target.current().clone(), true, cx);
    }
    pub(super) fn prepare_library_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Route::Library { search, .. } = self.navigation.current() else {
            return;
        };
        if self.library_ui.applied_search.as_ref() != Some(search) {
            if let Some(input) = &self.library_ui.search {
                input.update(cx, |input, cx| input.set_value(search.clone(), window, cx));
            } else {
                let input = cx.new(|cx| InputState::new(window, cx).default_value(search.clone()));
                cx.subscribe(&input, |this, _, event, cx| {
                    if matches!(event, gpui_kit::base::input::InputEvent::PressEnter { .. })
                        && !this.loading
                        && matches!(this.navigation.current(), Route::Library { .. })
                    {
                        this.search_library(cx);
                    }
                })
                .detach();
                self.library_ui.search = Some(input);
            }
            self.library_ui.applied_search = Some(search.clone());
        }
        if let Some(file) = &self.selected
            && self.library_ui.file_handle.as_ref() != Some(&file.handle)
        {
            self.library_ui.import_name =
                Some(cx.new(|cx| InputState::new(window, cx).default_value(file.name.clone())));
            self.library_ui.file_handle = Some(file.handle.clone());
        }
    }
    pub(super) fn search_library(&mut self, cx: &mut Context<Self>) {
        let Some(search) = &self.library_ui.search else {
            return;
        };
        let query = search.read(cx).value().trim().to_owned();
        let book = match self.navigation.current() {
            Route::Library { book, .. } => book.clone(),
            _ => None,
        };
        self.library_ui.page = 0;
        self.navigate(
            Route::Library {
                book,
                search: query,
            },
            cx,
        );
    }
    pub(super) fn import_controls(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let mut body = div().flex().flex_col().gap_2().child("导入目标").child(
            material_button(
                "import-new-book",
                if self.library_ui.target.is_none() {
                    ButtonVariant::Filled
                } else {
                    ButtonVariant::Tonal
                },
            )
            .child("新词书／同名同类合并")
            .disabled(self.loading)
            .on_click(cx.listener(|this, _, _, cx| {
                this.library_ui.target = None;
                cx.notify();
            })),
        );
        for (index, book) in self
            .library_ui
            .books
            .iter()
            .filter(|book| book.kind == self.import_kind)
            .enumerate()
        {
            let id = book.id.clone();
            body = body.child(
                material_button(
                    ("import-target", index),
                    if self.library_ui.target.as_ref() == Some(&book.id) {
                        ButtonVariant::Filled
                    } else {
                        ButtonVariant::Tonal
                    },
                )
                .child(format!("合并到 {}", book.name))
                .disabled(self.loading)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.library_ui.target = Some(id.clone());
                    cx.notify();
                })),
            );
        }
        if let Some(name) = &self.library_ui.import_name {
            body = body.child(
                TextField::new("import-book-name", name)
                    .label("词书名称")
                    .disabled(self.loading || self.library_ui.target.is_some()),
            );
        }
        body.into_any_element()
    }
    pub(super) fn import_report(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let mut body = div().flex().flex_col().gap_2();
        if let Some(report) = &self.report {
            body = body.child(format!(
                "最近一次导入结果：{}",
                self.library_ui
                    .import_source
                    .as_deref()
                    .unwrap_or("当前文件")
            ));
            body = body
                .child(format!(
                    "识别 {} 行；新增 {} 个词、{} 张词卡、{} 条词书成员",
                    report.parsed, report.words_added, report.cards_added, report.entries_added
                ))
                .child(format!(
                    "重复 {}；短语 {}；跳过 {}",
                    report.duplicates, report.phrases, report.skipped
                ));
            for error in report
                .errors
                .iter()
                .skip(self.library_ui.error_page * 20)
                .take(20)
            {
                body = body.child(format!("第 {} 行：{}", error.line, error.reason));
            }
            if !report.errors.is_empty() {
                body = body.child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            material_button("errors-previous", ButtonVariant::Text)
                                .child("上一页问题")
                                .disabled(self.library_ui.error_page == 0)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.library_ui.error_page =
                                        this.library_ui.error_page.saturating_sub(1);
                                    cx.notify();
                                })),
                        )
                        .child(
                            material_button("errors-next", ButtonVariant::Text)
                                .child("下一页问题")
                                .disabled(
                                    (self.library_ui.error_page + 1) * 20 >= report.errors.len(),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.library_ui.error_page += 1;
                                    cx.notify();
                                })),
                        ),
                );
            }
        }
        body.into_any_element()
    }
    pub(super) fn close_card_editor(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        if self
            .library_ui
            .editor
            .as_ref()
            .is_some_and(|editor| editor.dirty(cx))
        {
            self.library_ui.discard = true;
        } else {
            self.library_ui.editor = None;
        }
        cx.notify();
    }
    /// Switching cards must never silently replace an unsaved draft: the
    /// discard dialog asks first, and its scrim keeps rows from reacting.
    fn open_card_editor(&mut self, card: CardView, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading || self.library_ui.discard {
            return;
        }
        if self
            .library_ui
            .editor
            .as_ref()
            .is_some_and(|editor| editor.card.id != card.id && editor.dirty(cx))
        {
            self.library_ui.discard = true;
            cx.notify();
            return;
        }
        self.library_ui.editor = Some(CardEditor::new(card, window, cx));
        self.error = None;
        cx.notify();
    }
    pub(super) fn library_dialogs(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        if self.library_ui.discard {
            return div()
                .id("discard-card-draft-scrim")
                .absolute()
                .inset_0()
                .occlude()
                .flex()
                .items_center()
                .justify_center()
                .bg(design::alpha(design::c(ColorRole::INVERSE_SURFACE), 0.42))
                .child(
                    design::shaped(
                        [32., 32., 32., 32.],
                        design::c(ColorRole::SURFACE_CONTAINER_HIGH),
                    )
                    .w(px(420.))
                    .p(px(28.))
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(design::title(24., 32., 700.).child("放弃未保存的词卡修改？"))
                    .child(
                        design::muted_text(14., 20.).child(
                            "词卡释义和例句的修改尚未保存。继续编辑会保留当前输入；放弃会恢复上次保存的内容。",
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(div().flex_1())
                            .child(
                                div()
                                    .id("keep-card-draft")
                                    .test_support()
                                    .role(gpui_kit::Role::Button)
                                    .aria_label("继续编辑")
                                    .cursor_pointer()
                                    .h(px(40.))
                                    .px(px(20.))
                                    .rounded(px(20.))
                                    .bg(design::c(ColorRole::SECONDARY_CONTAINER))
                                    .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                                    .flex()
                                    .items_center()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.library_ui.discard = false;
                                        this.library_ui.pending_close = false;
                                        this.library_ui.pending_route = None;
                                        this.library_ui.pending_back = false;
                                        cx.notify();
                                    }))
                                    .child("继续编辑"),
                            )
                            .child(
                                div()
                                    .id("discard-card-draft-confirm")
                                    .test_support()
                                    .role(gpui_kit::Role::Button)
                                    .aria_label("放弃修改")
                                    .cursor_pointer()
                                    .h(px(40.))
                                    .px(px(16.))
                                    .rounded(px(20.))
                                    .text_color(design::c(ColorRole::ERROR))
                                    .flex()
                                    .items_center()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.library_ui.editor = None;
                                        this.library_ui.discard = false;
                                        let pending = this.library_ui.pending_route.take();
                                        let back = std::mem::take(&mut this.library_ui.pending_back);
                                        if std::mem::take(&mut this.library_ui.pending_close) {
                                            if this.close_reader_when_saved(window, cx) {
                                                window.remove_window();
                                            }
                                            return;
                                        }
                                        if back {
                                            this.go_back(cx);
                                        } else if let Some(route) = pending {
                                            this.navigate(route, cx);
                                        }
                                        cx.notify();
                                    }))
                                    .child("放弃修改"),
                            ),
                    ),
                )
                .into_any_element();
        }
        if let Some(preview) = &self.library_ui.deletion {
            let weak = cx.entity().downgrade();
            let mut dialog=MaterialDialog::new("delete-book-dialog").headline(format!("删除「{}」？",self.library_ui.books.iter().find(|book|book.id==preview.book_id).map(|book|book.name.as_str()).unwrap_or("已变更的词书"))).modal(true)
                .supporting_text(format!("移除 {} 条词书成员；保留 {} 张共享卡；删除 {} 张无历史卡；归档 {} 张有历史卡。归档卡不会被普通恢复开关重新启用。",preview.removed_entries,preview.shared_cards,preview.deleted_cards,preview.archived_cards))
                .on_dismiss(move|_,cx|{let _=weak.update(cx,|this,cx|{if !this.loading{this.library_ui.deletion=None;cx.notify();}});})
                .action(material_button("cancel-book-delete",ButtonVariant::Text).child("取消").disabled(self.loading).on_click(cx.listener(|this,_,_,cx|{this.library_ui.deletion=None;cx.notify();})))
                .action(material_button("refresh-book-delete",ButtonVariant::Tonal).child("刷新影响预览").disabled(self.loading).on_click(cx.listener(|this,_,_,cx|{if let Some(preview)=&this.library_ui.deletion{this.request(Command::PreviewDeleteBook{book:preview.book_id.clone()},cx);}})))
                .action(material_button("confirm-book-delete",ButtonVariant::Filled).child("确认删除").disabled(self.loading).on_click(cx.listener(|this,_,_,cx|{if let Some(preview)=&this.library_ui.deletion{this.request(Command::DeleteBook{book:preview.book_id.clone(),fingerprint:preview.fingerprint.clone(),operation:this.operation("delete-book")},cx);}})));
            if let Some(error) = &self.error {
                dialog = dialog.content(error.clone());
            }
            return dialog.into_any_element();
        }
        div().into_any_element()
    }
}

impl Shell {
    fn due_label(&self, due: i64) -> String {
        let today = study_date(&self.settings).and_hms_opt(0, 0, 0).unwrap();
        let today_ms = today.and_utc().timestamp_millis();
        let days = (due - today_ms).div_euclid(86_400_000);
        match days {
            0 => "今天".into(),
            1 => "明天".into(),
            2..=6 => {
                let date =
                    chrono::DateTime::from_timestamp_millis(due).map(|value| value.date_naive());
                date.map(|date| format!("{}/{}", date.month(), date.day()))
                    .unwrap_or_else(|| "—".into())
            }
            _ => chrono::DateTime::from_timestamp_millis(due)
                .map(|value| {
                    let date = value.date_naive();
                    format!("{:02}/{:02}", date.month(), date.day())
                })
                .unwrap_or_else(|| "—".into()),
        }
    }

    fn library_nav(
        &self,
        cx: &mut Context<Self>,
        selected: Option<&String>,
    ) -> gpui_kit::AnyElement {
        let mut nav = design::pane()
            .w(px(if self.viewport_width < 1200. {
                240.
            } else {
                280.
            }))
            .flex_shrink_0()
            .h_full()
            .p(px(24.))
            .px(px(12.))
            .gap(px(4.));
        nav = nav.child(
            div()
                .px(px(12.))
                .pb(px(14.))
                .child(design::title(36., 44., 780.).child("词库")),
        );
        nav = nav.child(
            div()
                .id("toggle-import")
                .test_support()
                .role(gpui_kit::Role::Button)
                .aria_label("导入词表")
                .cursor_pointer()
                .mx(px(8.))
                .mb(px(14.))
                .h(px(56.))
                .rounded(px(20.))
                .bg(design::c(ColorRole::SECONDARY_CONTAINER))
                .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                .flex()
                .items_center()
                .justify_start()
                .gap(px(14.))
                .px(px(16.))
                .text_size(px(15.))
                .font_weight(gpui_kit::FontWeight(650.))
                .on_click(cx.listener(|this, _, _, cx| {
                    if !this.library_ui.import_open {
                        this.word_list = true;
                        this.import_kind = BookKind::Baseline;
                        this.library_ui.target = None;
                        this.selected = None;
                    }
                    this.library_ui.import_open = !this.library_ui.import_open;
                    cx.notify();
                }))
                .child(design::icon_role(
                    Icon::Upload,
                    24.,
                    ColorRole::ON_SECONDARY_CONTAINER,
                ))
                .child("导入词表"),
        );
        nav = nav.child(
            design::mono(12., 16., 650.)
                .px(px(16.))
                .pb(px(6.))
                .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                .child("词书"),
        );
        let all_selected = selected.is_none();
        nav = nav.child(
            div()
                .id("all-books")
                .role(gpui_kit::Role::Button)
                .aria_label("全部词卡")
                .cursor_pointer()
                .h(px(56.))
                .px(px(16.))
                .rounded(px(28.))
                .flex()
                .items_center()
                .gap(px(14.))
                .when(all_selected, |row| {
                    row.bg(design::c(ColorRole::SECONDARY_CONTAINER))
                        .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                        .font_weight(gpui_kit::FontWeight(700.))
                })
                .when(!all_selected, |row| {
                    row.text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.library_ui.page = 0;
                    this.navigate(
                        Route::Library {
                            book: None,
                            search: String::new(),
                        },
                        cx,
                    );
                }))
                .child(design::icon_role(
                    Icon::Layers,
                    24.,
                    ColorRole::ON_SURFACE_VARIANT,
                ))
                .child(div().flex_1().text_left().child("全部词卡"))
                .child(
                    design::text(13., 18., 400.).child(
                        self.library
                            .as_ref()
                            .map(|page| page.total.to_string())
                            .unwrap_or_else(|| "—".into()),
                    ),
                ),
        );
        for book in &self.library_ui.books {
            let id = book.id.clone();
            let kind = book.kind;
            let is_selected = selected == Some(&book.id);
            nav = nav.child(
                div()
                    .id(gpui_kit::SharedString::from(format!(
                        "book-filter-{}",
                        book.id
                    )))
                    .role(gpui_kit::Role::Button)
                    .aria_label(book.name.clone())
                    .cursor_pointer()
                    .h(px(56.))
                    .px(px(16.))
                    .rounded(px(28.))
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .when(is_selected, |row| {
                        row.bg(design::c(ColorRole::SECONDARY_CONTAINER))
                            .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                    })
                    .when(!is_selected, |row| {
                        row.text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.library_ui.page = 0;
                        if kind == BookKind::Baseline {
                            this.library_ui.state = None;
                            this.library_ui.suspended = None;
                        }
                        this.navigate(
                            Route::Library {
                                book: Some(id.clone()),
                                search: String::new(),
                            },
                            cx,
                        );
                    }))
                    .child(design::icon_role(
                        Icon::Book,
                        24.,
                        ColorRole::ON_SURFACE_VARIANT,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(
                                design::text(15., 20., 500.)
                                    .text_color(design::c(ColorRole::ON_SURFACE))
                                    .child(book.name.clone()),
                            )
                            .child(design::muted_text(12., 16.).child(format!(
                                "{} {}",
                                book.entries,
                                if book.kind == BookKind::Baseline {
                                    "词条"
                                } else {
                                    "词卡"
                                }
                            ))),
                    )
                    .child(
                        div()
                            .w(px(40.))
                            .h(px(40.))
                            .rounded(px(20.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(design::icon_role(
                                Icon::More,
                                18.,
                                ColorRole::ON_SURFACE_VARIANT,
                            )),
                    ),
            );
        }
        nav = nav.child(div().flex_1());
        nav = nav.child(
            div()
                .m(px(8.))
                .p(px(14.))
                .px(px(16.))
                .rounded(px(20.))
                .bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST))
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(design::text(13., 18., 650.).child("支持的格式"))
                .child(design::muted_text(12., 16.).child(
                    "词卡 CSV · 兼容旧版 .apkg · 词表 TXT / CSV（UTF-8）· 词包 .cijianpack",
                )),
        );
        nav.into_any_element()
    }

    fn library_search(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let Some(input) = &self.library_ui.search else {
            return div().into_any_element();
        };
        let value = input.read(cx).value().to_string();
        let leading = div()
            .id("search-library")
            .test_support()
            .role(gpui_kit::Role::Button)
            .aria_label("搜索")
            .cursor_pointer()
            .flex()
            .items_center()
            .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
            .on_click(cx.listener(|this, _, _, cx| this.search_library(cx)))
            .child(design::icon_role(
                Icon::Search,
                24.,
                ColorRole::ON_SURFACE_VARIANT,
            ))
            .into_any_element();
        div()
            .relative()
            .h(px(56.))
            .w_full()
            .rounded(px(28.))
            .bg(design::c(ColorRole::SURFACE_CONTAINER_HIGH))
            .flex()
            .items_center()
            .gap(px(16.))
            .px(px(20.))
            .child(leading)
            .child(
                if value.is_empty() {
                    design::muted_text(16., 22.).child("搜索单词或释义")
                } else {
                    design::text(16., 22., 400.)
                        .text_color(design::c(ColorRole::ON_SURFACE))
                        .child(value)
                }
                .flex_1()
                .min_w_0()
                .text_ellipsis(),
            )
            .child(
                design::mono(12., 16., 400.)
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(8.))
                    .bg(design::c(ColorRole::SURFACE_CONTAINER_LOW))
                    .text_color(design::c(ColorRole::OUTLINE))
                    .child("/"),
            )
            .child(
                div().absolute().inset_0().opacity(0.).child(
                    TextField::new("library-search", input)
                        .shape(gpui_kit_material::components::TextFieldShape::Rounded)
                        .w_full()
                        .h_full(),
                ),
            )
            .into_any_element()
    }

    fn library_editor(&self, cx: &mut Context<Self>) -> Option<gpui_kit::AnyElement> {
        let editor = self.library_ui.editor.as_ref()?;
        let mut pane = design::pane()
            .w(px(340.))
            .flex_shrink_0()
            .h_full()
            .p(px(20.))
            .px(px(22.))
            .pb(px(22.))
            .gap(px(16.))
            .bg(design::c(ColorRole::SURFACE_CONTAINER));
        pane = pane.child(
            div()
                .flex()
                .items_center()
                .child(
                    design::muted_text(14., 20.)
                        .font_weight(gpui_kit::FontWeight(650.))
                        .flex_1()
                        .child("编辑词卡"),
                )
                .child(
                    div()
                        .id("close-card-editor")
                        .test_support()
                        .role(gpui_kit::Role::Button)
                        .aria_label("关闭词卡编辑")
                        .cursor_pointer()
                        .w(px(40.))
                        .h(px(40.))
                        .rounded(px(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                        .on_click(cx.listener(|this, _, _, cx| this.close_card_editor(cx)))
                        .child(design::icon_role(
                            Icon::Close,
                            24.,
                            ColorRole::ON_SURFACE_VARIANT,
                        )),
                ),
        );
        pane = pane.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    design::title(34., 40., 760.)
                        .font_family("Cijian Display")
                        .child(editor.card.front.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(
                            design::tag(
                                ColorRole::TERTIARY_CONTAINER,
                                ColorRole::ON_TERTIARY_CONTAINER,
                            )
                            .child(state_label(editor.card.srs.state)),
                        )
                        .child(
                            design::muted_text(13., 18.).child(self.due_label(editor.card.srs.due)),
                        ),
                ),
        );
        pane = pane.child(
            div()
                .flex()
                .flex_col()
                .child(
                    TextField::multi_line("card-meaning", &editor.meaning)
                        .label("释义")
                        .max_length(1000)
                        .w_full(),
                )
                .child(
                    design::muted_text(12., 16.)
                        .px(px(16.))
                        .child("保存后来源标记为“用户编辑”"),
                ),
        );
        pane = pane.child(
            div().flex().flex_col().child(
                TextField::multi_line("card-example", &editor.example)
                    .label("例句")
                    .max_length(5000)
                    .w_full(),
            ),
        );
        let suspended = editor.card.suspended;
        pane = pane.child(
            div()
                .id("editor-suspend")
                .role(gpui_kit::Role::Button)
                .aria_label("暂停这张卡")
                .cursor_pointer()
                .rounded(px(20.))
                .bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST))
                .p(px(14.))
                .px(px(16.))
                .flex()
                .items_center()
                .gap(px(12.))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.library_ui.discard {
                        return;
                    }
                    let Some(editor) = &this.library_ui.editor else {
                        return;
                    };
                    this.request(
                        Command::EditLibraryCard {
                            card: editor.card.id.clone(),
                            revision: editor.card.revision,
                            meaning: editor.meaning.read(cx).value().to_string(),
                            example: editor.example.read(cx).value().to_string(),
                            suspended: !editor.card.suspended,
                            operation: this.operation("suspend"),
                        },
                        cx,
                    );
                }))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .child(design::text(15., 20., 650.).child("暂停这张卡"))
                        .child(design::muted_text(12., 16.).child("暂停后不再进入复习队列")),
                )
                .child(
                    div()
                        .w(px(52.))
                        .h(px(32.))
                        .rounded(px(16.))
                        .bg(design::c(if suspended {
                            ColorRole::PRIMARY
                        } else {
                            ColorRole::SURFACE_CONTAINER_HIGHEST
                        }))
                        .relative()
                        .child(
                            div()
                                .absolute()
                                .top(px(4.))
                                .left(px(if suspended { 24. } else { 4. }))
                                .w(px(24.))
                                .h(px(24.))
                                .rounded(px(12.))
                                .bg(design::c(if suspended {
                                    ColorRole::ON_PRIMARY
                                } else {
                                    ColorRole::OUTLINE
                                })),
                        ),
                ),
        );
        pane = pane.child(div().flex_1());
        let dirty = editor.dirty(cx);
        pane = pane.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().flex_1())
                .child(
                    div()
                        .id("cancel-card-edit")
                        .test_support()
                        .role(gpui_kit::Role::Button)
                        .aria_label("取消")
                        .cursor_pointer()
                        .h(px(40.))
                        .px(px(12.))
                        .rounded(px(20.))
                        .flex()
                        .items_center()
                        .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                        .on_click(cx.listener(|this, _, _, cx| this.close_card_editor(cx)))
                        .child("取消"),
                )
                .child(
                    div()
                        .id("save-card-edit")
                        .test_support()
                        .role(gpui_kit::Role::Button)
                        .aria_label("保存词卡")
                        .cursor_pointer()
                        .h(px(56.))
                        .px(px(28.))
                        .rounded(px(28.))
                        .bg(design::c(ColorRole::PRIMARY))
                        .text_color(design::c(ColorRole::ON_PRIMARY))
                        .text_size(px(16.))
                        .font_weight(gpui_kit::FontWeight(650.))
                        .flex()
                        .items_center()
                        .when(self.loading || !dirty, |button| button.opacity(0.38))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.loading {
                                return;
                            }
                            if let Some(editor) = &this.library_ui.editor {
                                this.request(
                                    Command::EditLibraryCard {
                                        card: editor.card.id.clone(),
                                        revision: editor.card.revision,
                                        meaning: editor.meaning.read(cx).value().to_string(),
                                        example: editor.example.read(cx).value().to_string(),
                                        suspended: editor.card.suspended,
                                        operation: this.operation("edit-card"),
                                    },
                                    cx,
                                );
                            }
                        }))
                        .child("保存词卡"),
                ),
        );
        Some(pane.into_any_element())
    }

    fn library_editor_placeholder(&self) -> gpui_kit::AnyElement {
        design::pane()
            .w(px(340.))
            .flex_shrink_0()
            .h_full()
            .p(px(20.))
            .px(px(22.))
            .gap(px(16.))
            .bg(design::c(ColorRole::SURFACE_CONTAINER))
            .child(
                design::muted_text(14., 20.)
                    .font_weight(gpui_kit::FontWeight(650.))
                    .child("编辑词卡"),
            )
            .child(
                div().flex_1().flex().items_center().justify_center().child(
                    design::muted_text(13., 18.).child("选择一张词卡，查看并编辑释义与例句。"),
                ),
            )
            .into_any_element()
    }

    pub(super) fn library_body(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let (selected, search) = match self.navigation.current() {
            Route::Library { book, search } => (book.clone(), search.clone()),
            _ => (None, String::new()),
        };
        let _ = search;
        let baseline = selected
            .as_ref()
            .and_then(|id| self.library_ui.books.iter().find(|book| &book.id == id))
            .is_some_and(|book| book.kind == BookKind::Baseline);
        let editor_open = self.library_ui.editor.is_some() && !self.library_ui.discard;
        let wide = self.viewport_width >= 1200.;
        let compact = self.compact;

        let mut center = design::pane()
            .flex_1()
            .min_w_0()
            .h_full()
            .p(px(24.))
            .px(px(if compact { 16. } else { 28. }))
            .gap(px(16.));
        center = center.child(self.library_search(cx));
        let mut filters = div().flex().items_center().gap(px(8.)).flex_wrap();
        for (label, value) in [
            ("全部状态", None),
            ("新词", Some(State::New)),
            ("学习中", Some(State::Learning)),
            ("复习", Some(State::Review)),
            ("重新学习", Some(State::Relearning)),
        ] {
            let selected_state = self.library_ui.state == value;
            filters = filters.child(
                div()
                    .id(gpui_kit::SharedString::from(match value {
                        None => "library-state-all".to_owned(),
                        Some(State::New) => "library-state-new".to_owned(),
                        Some(State::Learning) => "library-state-learning".to_owned(),
                        Some(State::Review) => "library-state-review".to_owned(),
                        Some(State::Relearning) => "library-state-relearning".to_owned(),
                    }))
                    .role(gpui_kit::Role::Button)
                    .aria_label(label)
                    .cursor_pointer()
                    .when(baseline, |chip| chip.opacity(0.38))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if baseline || this.loading {
                            return;
                        }
                        this.library_ui.state = value;
                        this.library_ui.page = 0;
                        this.request(
                            Command::LoadLibrary {
                                filter: this.library_filter(),
                            },
                            cx,
                        );
                    }))
                    .child(design::chip(selected_state, label, None)),
            );
        }
        filters = filters.child(
            div()
                .w(px(1.))
                .h(px(24.))
                .bg(design::c(ColorRole::OUTLINE_VARIANT)),
        );
        let suspended_label = match self.library_ui.suspended {
            None => "暂停／归档：全部",
            Some(false) => "暂停／归档：仅未暂停",
            Some(true) => "暂停／归档：仅已暂停",
        };
        let suspended_focus = self
            .library_ui
            .suspended_focus
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        let suspended_trigger = div()
            .id("library-suspended")
            .test_support()
            .role(gpui_kit::Role::Button)
            .aria_label(suspended_label)
            .track_focus(&suspended_focus)
            .tab_stop(true)
            .cursor_pointer()
            .when(baseline, |chip| chip.opacity(0.38))
            .on_click(cx.listener(|this, event: &gpui_kit::ClickEvent, _, cx| {
                if matches!(event, gpui_kit::ClickEvent::Keyboard(_)) {
                    this.library_ui.suspended_open = !this.library_ui.suspended_open;
                    cx.notify();
                }
            }))
            .child(design::chip(
                false,
                suspended_label,
                Some(if self.library_ui.suspended_open {
                    Icon::Up
                } else {
                    Icon::Down
                }),
            ));
        let suspended_menu = MaterialMenu::new("library-suspended-menu")
            .width(280.)
            .group(
                MenuGroup::new().label("暂停与归档").items(
                    [
                        (None, "全部词卡", "包含已暂停与归档的词卡"),
                        (Some(false), "仅未暂停", "隐藏已暂停与归档的词卡"),
                        (Some(true), "仅已暂停／归档", "只显示暂停或归档的词卡"),
                    ]
                    .into_iter()
                    .map(|(value, label, supporting)| {
                        MaterialMenuItem::new(label)
                            .supporting_text(supporting)
                            .selected(self.library_ui.suspended == value)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.library_ui.suspended = value;
                                this.library_ui.suspended_open = false;
                                this.library_ui.page = 0;
                                this.request(
                                    Command::LoadLibrary {
                                        filter: this.library_filter(),
                                    },
                                    cx,
                                );
                            }))
                    }),
                ),
            );
        filters = filters.child(
            MenuPopup::new("library-suspended-popup", suspended_trigger, suspended_menu)
                .open(self.library_ui.suspended_open)
                .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                    this.library_ui.suspended_open = *open;
                    cx.notify();
                })),
        );
        filters = filters.child(div().flex_1());
        let total = self
            .library_ui
            .entries
            .as_ref()
            .map(|page| page.total)
            .or_else(|| self.library.as_ref().map(|page| page.total))
            .unwrap_or(0);
        filters = filters.child(design::muted_text(13., 18.).child(format!("共 {total} 词")));
        center = center.child(filters);

        let mut table = div()
            .id("library-table")
            .flex()
            .flex_col()
            .gap(px(2.))
            .flex_1()
            .min_h_0()
            .rounded(px(24.))
            .bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST))
            .p(px(6.))
            .px(px(8.));
        let mut header = div()
            .flex()
            .items_center()
            .px(px(14.))
            .pt(px(8.))
            .pb(px(6.))
            .text_size(px(12.))
            .line_height(px(16.))
            .font_weight(gpui_kit::FontWeight(650.))
            .text_color(design::c(ColorRole::ON_SURFACE_VARIANT));
        header = header
            .child(div().flex_1().child("单词 / 释义"))
            .child(div().w(px(120.)).child("来源"))
            .child(div().w(px(96.)).child("状态"))
            .child(div().w(px(80.)).child("下次到期"))
            .child(div().w(px(84.)));
        table = table.child(header);
        let mut rows_body = div()
            .id("library-rows")
            .flex()
            .flex_col()
            .gap(px(2.))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();

        let mut rows = 0usize;
        if let Some(page) = &self.library_ui.entries {
            for entry in &page.entries {
                rows += 1;
                let mut row = div()
                    .flex()
                    .items_center()
                    .px(px(14.))
                    .py(px(10.))
                    .gap(px(0.))
                    .rounded(px(16.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                design::text(16., 22., 650.)
                                    .text_ellipsis()
                                    .child(entry.lemma.clone()),
                            )
                            .child(
                                design::muted_text(13., 18.)
                                    .text_ellipsis()
                                    .child(entry.raw.clone()),
                            ),
                    );
                if let Some(card) = &entry.card {
                    row = row.child(
                        div().w(px(120.)).child(
                            design::muted_text(13., 18.)
                                .text_ellipsis()
                                .child(crate::study::source_label(&card.text_source)),
                        ),
                    );
                    row = row.child(div().w(px(96.)).child(self.state_tag(card)));
                    row = row.child(
                        design::mono(13., 18., 400.)
                            .w(px(80.))
                            .child(self.due_label(card.srs.due)),
                    );
                    row = row.child(self.library_row_actions(card, cx));
                } else {
                    row = row
                        .child(div().w(px(120.)))
                        .child(
                            div().w(px(96.)).child(
                                design::tag(
                                    ColorRole::SECONDARY_CONTAINER,
                                    ColorRole::ON_SECONDARY_CONTAINER,
                                )
                                .child("基线"),
                            ),
                        )
                        .child(div().w(px(80.)))
                        .child(div().w(px(84.)));
                }
                rows_body = rows_body.child(row);
            }
        } else if let Some(page) = &self.library {
            for card in &page.cards {
                rows += 1;
                rows_body = rows_body.child(self.library_row(card, cx));
            }
        }
        if rows == 0 {
            rows_body = rows_body.child(design::muted_text(14., 20.).p(px(16.)).child(
                if self.loading {
                    "正在读取词卡…"
                } else {
                    "没有符合条件的词卡"
                },
            ));
        }
        table = table.child(rows_body);
        table = table.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .p(px(8.))
                .px(px(6.))
                .child(div().flex_1())
                .child(
                    design::mono(13., 18., 400.)
                        .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                        .child(if rows == 0 {
                            format!("0 / {total}")
                        } else {
                            format!(
                                "{}–{} / {}",
                                self.library_ui.page * 30 + 1,
                                (self.library_ui.page * 30 + rows as u32).min(total as u32),
                                total
                            )
                        }),
                )
                .child(self.page_button(
                    "library-previous",
                    Icon::Back,
                    self.library_ui.page == 0,
                    -1,
                    cx,
                ))
                .child(self.page_button(
                    "library-next",
                    Icon::Chev,
                    (u64::from(self.library_ui.page) + 1) * 30 >= total,
                    1,
                    cx,
                )),
        );
        center = center.child(table);
        if let Some(selected) = &selected {
            let book = selected.clone();
            center = center.child(
                div()
                    .id("delete-book-preview")
                    .role(gpui_kit::Role::Button)
                    .aria_label("查看删除词书的影响")
                    .cursor_pointer()
                    .h(px(36.))
                    .flex()
                    .items_center()
                    .text_color(design::c(ColorRole::ERROR))
                    .text_size(px(13.))
                    .child("查看删除词书的影响")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.request(Command::PreviewDeleteBook { book: book.clone() }, cx)
                    })),
            );
        }

        let mut main = div().flex().flex_1().min_w_0().h_full().gap(px(12.));
        if !compact && !(editor_open && !wide) {
            main = main.child(self.library_nav(cx, selected.as_ref()));
        }
        main = main.child(center);
        if editor_open {
            if let Some(editor) = self.library_editor(cx) {
                main = main.child(editor);
            }
        } else if wide {
            main = main.child(self.library_editor_placeholder());
        }
        if compact && editor_open {
            // The editor is the whole page on a phone-sized window, but
            // confirmation surfaces still live above it.
            if let Some(editor) = self.library_editor(cx) {
                return div()
                    .relative()
                    .flex()
                    .flex_col()
                    .w_full()
                    .h_full()
                    .child(editor)
                    .child(div().absolute().inset_0().child(self.library_dialogs(cx)))
                    .into_any_element();
            }
        }
        // Paint the page first: absolute overlays added afterwards receive
        // presses above it instead of letting the library table swallow them.
        let mut body = div().relative().flex().flex_col().w_full().h_full();
        body = body.child(main);
        if self.library_ui.import_open {
            body = body.child(self.library_import_overlay(cx));
        }
        if self.library_ui.report_open {
            body = body.child(self.library_report_overlay(cx));
        }
        body = body.child(div().absolute().inset_0().child(self.library_dialogs(cx)));
        body.into_any_element()
    }

    fn state_tag(&self, card: &CardView) -> gpui_kit::AnyElement {
        if card.suspended {
            return design::tag(ColorRole::WARNING_BG, ColorRole::ON_WARNING)
                .child("已暂停")
                .into_any_element();
        }
        let (role_bg, role_fg) = match card.srs.state {
            State::New => (
                ColorRole::SURFACE_CONTAINER_HIGH,
                ColorRole::ON_SURFACE_VARIANT,
            ),
            State::Learning => (
                ColorRole::TERTIARY_CONTAINER,
                ColorRole::ON_TERTIARY_CONTAINER,
            ),
            State::Review => (
                ColorRole::PRIMARY_CONTAINER,
                ColorRole::ON_PRIMARY_CONTAINER,
            ),
            State::Relearning => (ColorRole::ERROR_CONTAINER, ColorRole::ON_ERROR_CONTAINER),
        };
        design::tag(role_bg, role_fg)
            .child(state_label(card.srs.state))
            .into_any_element()
    }

    fn library_row_actions(&self, card: &CardView, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let edit = card.clone();
        let toggle = card.clone();
        let selected = self
            .library_ui
            .editor
            .as_ref()
            .is_some_and(|editor| editor.card.id == card.id);
        div()
            .w(px(84.))
            .flex()
            .gap(px(2.))
            .child(
                div()
                    .id(gpui_kit::SharedString::from(format!(
                        "edit-card-{}",
                        card.id
                    )))
                    .test_support()
                    .role(gpui_kit::Role::Button)
                    .aria_label("编辑词卡")
                    .cursor_pointer()
                    .w(px(40.))
                    .h(px(40.))
                    .rounded(px(20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(selected, |button| {
                        button
                            .bg(design::c(ColorRole::PRIMARY))
                            .text_color(design::c(ColorRole::ON_PRIMARY))
                    })
                    .when(!self.loading && !selected, |button| {
                        button.hover(|style| style.bg(design::c(ColorRole::PRIMARY)))
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.open_card_editor(edit.clone(), window, cx);
                    }))
                    .child(design::icon_role(
                        Icon::Edit,
                        18.,
                        if selected {
                            ColorRole::ON_PRIMARY
                        } else {
                            ColorRole::ON_SURFACE_VARIANT
                        },
                    )),
            )
            .child(
                div()
                    .id(gpui_kit::SharedString::from(format!(
                        "suspend-card-{}",
                        card.id
                    )))
                    .role(gpui_kit::Role::Button)
                    .aria_label(if toggle.suspended {
                        "恢复复习"
                    } else {
                        "暂停复习"
                    })
                    .cursor_pointer()
                    .w(px(40.))
                    .h(px(40.))
                    .rounded(px(20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(toggle.suspended, |button| {
                        button.text_color(design::c(ColorRole::PRIMARY))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        if this.loading {
                            return;
                        }
                        this.request(
                            Command::EditLibraryCard {
                                card: toggle.id.clone(),
                                revision: toggle.revision,
                                meaning: toggle.meaning.clone(),
                                example: toggle.example.clone(),
                                suspended: !toggle.suspended,
                                operation: this.operation("suspend"),
                            },
                            cx,
                        )
                    }))
                    .child(design::icon_role(
                        if toggle.suspended {
                            Icon::Play
                        } else {
                            Icon::Pausec
                        },
                        18.,
                        if toggle.suspended {
                            ColorRole::PRIMARY
                        } else {
                            ColorRole::ON_SURFACE_VARIANT
                        },
                    )),
            )
            .into_any_element()
    }

    fn library_row(&self, card: &CardView, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let row = div()
            .id(gpui_kit::SharedString::from(format!(
                "library-card-{}",
                card.id
            )))
            .test_support()
            .flex()
            .items_center()
            .px(px(14.))
            .py(px(10.))
            .rounded(px(16.))
            .cursor_pointer()
            .hover(|row| row.bg(design::c(ColorRole::SURFACE_CONTAINER)))
            .when(card.suspended, |row| row.opacity(0.6))
            .on_click(cx.listener({
                let edit = card.clone();
                move |this, _, window, cx| this.open_card_editor(edit.clone(), window, cx)
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        design::text(16., 22., 650.)
                            .text_ellipsis()
                            .child(card.front.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .min_w_0()
                            .w_full()
                            .child(
                                design::muted_text(13., 18.)
                                    .id(gpui_kit::SharedString::from(format!(
                                        "library-meaning-{}",
                                        card.id
                                    )))
                                    .test_support()
                                    .flex_1()
                                    .min_w_0()
                                    .text_ellipsis()
                                    .child(if card.meaning.is_empty() {
                                        "释义未生成".to_owned()
                                    } else {
                                        card.meaning.clone()
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .id(gpui_kit::SharedString::from(format!(
                        "library-source-{}",
                        card.id
                    )))
                    .test_support()
                    .w(px(120.))
                    .child(
                        design::muted_text(13., 18.)
                            .text_ellipsis()
                            .child(crate::study::source_label(&card.text_source)),
                    ),
            )
            .child(div().w(px(96.)).child(self.state_tag(card)))
            .child(
                design::mono(13., 18., 400.)
                    .w(px(80.))
                    .child(if card.suspended {
                        "—".to_owned()
                    } else {
                        self.due_label(card.srs.due)
                    }),
            )
            .child(self.library_row_actions(card, cx));
        row.into_any_element()
    }

    fn page_button(
        &self,
        id: &'static str,
        icon: Icon,
        disabled: bool,
        delta: i32,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        div()
            .id(id)
            .role(gpui_kit::Role::Button)
            .aria_label(if delta < 0 { "上一页" } else { "下一页" })
            .cursor_pointer()
            .w(px(40.))
            .h(px(40.))
            .rounded(px(20.))
            .border_1()
            .border_color(design::c(ColorRole::OUTLINE_VARIANT))
            .flex()
            .items_center()
            .justify_center()
            .when(disabled, |button| button.opacity(0.38))
            .on_click(cx.listener(move |this, _, _, cx| {
                if disabled || this.loading {
                    return;
                }
                if delta < 0 {
                    this.library_ui.page = this.library_ui.page.saturating_sub(1);
                } else {
                    this.library_ui.page = this.library_ui.page.saturating_add(1);
                }
                this.request(
                    Command::LoadLibrary {
                        filter: this.library_filter(),
                    },
                    cx,
                );
            }))
            .child(design::icon_role(icon, 18., ColorRole::ON_SURFACE_VARIANT))
            .into_any_element()
    }

    fn library_import_overlay(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let mut dialog = design::shaped(
            [32., 32., 32., 32.],
            design::c(ColorRole::SURFACE_CONTAINER_HIGH),
        )
        .w(px(620.))
        .max_w_full()
        .max_h(gpui_kit::relative(1.))
        .p(px(24.))
        .px(px(32.))
        .pb(px(24.))
        .flex()
        .flex_col()
        .gap(px(16.));
        dialog = dialog.child(
            div()
                .flex()
                .items_center()
                .child(
                    design::title(24., 32., 700.)
                        .flex_1()
                        .child("导入词卡或词表"),
                )
                .child(
                    div()
                        .id("close-import")
                        .test_support()
                        .role(gpui_kit::Role::Button)
                        .aria_label("关闭导入")
                        .cursor_pointer()
                        .w(px(40.))
                        .h(px(40.))
                        .rounded(px(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.library_ui.import_open = false;
                            cx.notify();
                        }))
                        .child(design::icon_role(
                            Icon::Close,
                            24.,
                            ColorRole::ON_SURFACE_VARIANT,
                        )),
                ),
        );

        // The target list grows with the library, so only the settings scroll;
        // the file actions stay pinned below them.
        let mut content = div()
            .id("import-dialog-content")
            .flex()
            .flex_col()
            .gap(px(16.))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        let word_pack = self.selected.as_ref().is_some_and(|file| file.word_pack);
        if word_pack {
            // A pack already names its book and declares baseline or study, so
            // asking again would promise a choice the import silently ignores.
            content = content.child(design::muted_text(13., 18.).child(
                "词包自带词语、释义、双口音音标与录音；导入后成为一本新词书，用途以包内声明为准。",
            ));
        } else {
            content = content.child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(self.import_mode_button(false, "词卡", cx))
                    .child(self.import_mode_button(true, "词表", cx)),
            );
            if self.word_list {
                content = content.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(8.))
                        .child(self.import_kind_button(BookKind::Baseline, "基线：作为已知词", cx))
                        .child(self.import_kind_button(BookKind::Study, "学习：加入复习", cx)),
                );
            }
            content = content.child(self.import_controls(cx));
        }
        dialog = dialog.child(content);

        let mut footer = div().flex().flex_col().gap(px(10.));
        if let Some(selected) = &self.selected {
            footer = footer
                .child(design::muted_text(13., 18.).child(format!("已选择：{}", selected.name)));
        }
        footer = footer.child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(8.))
                .child(
                    div()
                        .id("choose-file")
                        .test_support()
                        .role(gpui_kit::Role::Button)
                        .aria_label("选择文件")
                        .cursor_pointer()
                        .h(px(48.))
                        .px(px(20.))
                        .rounded(px(24.))
                        .bg(design::c(ColorRole::SECONDARY_CONTAINER))
                        .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .when(self.loading, |button| button.opacity(0.38))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.request(
                                Command::ChooseImport {
                                    word_list: this.word_list,
                                },
                                cx,
                            )
                        }))
                        .child(design::icon_role(
                            Icon::Upload,
                            20.,
                            ColorRole::ON_SECONDARY_CONTAINER,
                        ))
                        .child(if self.word_list {
                            "选择词表 CSV / TXT / 词包"
                        } else {
                            "选择词卡 CSV / APKG / 词包"
                        }),
                )
                .child(
                    div()
                        .id("import-file")
                        .test_support()
                        .role(gpui_kit::Role::Button)
                        .aria_label("开始导入")
                        .cursor_pointer()
                        .h(px(48.))
                        .px(px(24.))
                        .rounded(px(24.))
                        .bg(design::c(ColorRole::PRIMARY))
                        .text_color(design::c(ColorRole::ON_PRIMARY))
                        .flex()
                        .items_center()
                        .when(self.loading || self.selected.is_none(), |button| {
                            button.opacity(0.38)
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.loading {
                                return;
                            }
                            if let Some(file) = this.selected.clone() {
                                this.request(
                                    Command::ImportSelected {
                                        target_book: this.library_ui.target.clone(),
                                        handle: file.handle,
                                        name: this
                                            .library_ui
                                            .import_name
                                            .as_ref()
                                            .map(|input| input.read(cx).value().trim().to_owned())
                                            .unwrap_or(file.name),
                                        kind: this.import_kind,
                                        word_list: this.word_list,
                                    },
                                    cx,
                                );
                            }
                        }))
                        .child("开始导入"),
                ),
        );
        dialog = dialog.child(footer);
        div()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .p(px(if self.compact { 12. } else { 24. }))
            .bg(design::alpha(design::c(ColorRole::INVERSE_SURFACE), 0.42))
            .child(dialog)
            .into_any_element()
    }

    fn import_mode_button(
        &self,
        word_list: bool,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        div()
            .id(gpui_kit::SharedString::from(if word_list {
                "mode-wordlist"
            } else {
                "mode-cards"
            }))
            .test_support()
            .role(gpui_kit::Role::Button)
            .aria_label(label)
            .cursor_pointer()
            .flex_1()
            .h(px(40.))
            .rounded(px(8.))
            .bg(design::c(if self.word_list == word_list {
                ColorRole::SECONDARY
            } else {
                ColorRole::SURFACE_CONTAINER_HIGH
            }))
            .text_color(design::c(if self.word_list == word_list {
                ColorRole::SURFACE_CONTAINER_LOWEST
            } else {
                ColorRole::ON_SURFACE_VARIANT
            }))
            .flex()
            .items_center()
            .justify_center()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.word_list = word_list;
                this.import_kind = BookKind::Study;
                this.library_ui.target = None;
                this.selected = None;
                cx.notify();
            }))
            .child(label)
            .into_any_element()
    }

    fn import_kind_button(
        &self,
        kind: BookKind,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        div()
            .id(gpui_kit::SharedString::from(
                if kind == BookKind::Baseline {
                    "kind-baseline"
                } else {
                    "kind-study"
                },
            ))
            .test_support()
            .role(gpui_kit::Role::Button)
            .aria_label(label)
            .cursor_pointer()
            .flex_1()
            .h(px(40.))
            .rounded(px(8.))
            .bg(design::c(if self.import_kind == kind {
                ColorRole::SECONDARY
            } else {
                ColorRole::SURFACE_CONTAINER_HIGH
            }))
            .text_color(design::c(if self.import_kind == kind {
                ColorRole::SURFACE_CONTAINER_LOWEST
            } else {
                ColorRole::ON_SURFACE_VARIANT
            }))
            .flex()
            .items_center()
            .justify_center()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.import_kind = kind;
                this.library_ui.target = None;
                cx.notify();
            }))
            .child(label)
            .into_any_element()
    }

    fn library_report_overlay(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let report = self.import_report(cx);
        div()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(design::alpha(design::c(ColorRole::INVERSE_SURFACE), 0.42))
            .child(
                design::shaped(
                    [32., 32., 32., 32.],
                    design::c(ColorRole::SURFACE_CONTAINER_HIGH),
                )
                .w(px(560.))
                .max_w_full()
                .p(px(28.))
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .child(design::title(24., 32., 700.).flex_1().child("最近导入结果"))
                        .child(
                            div()
                                .id("toggle-import-report")
                                .role(gpui_kit::Role::Button)
                                .aria_label("关闭导入结果")
                                .cursor_pointer()
                                .w(px(40.))
                                .h(px(40.))
                                .rounded(px(20.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.library_ui.report_open = false;
                                    this.library_ui.report_pinned = false;
                                    cx.notify();
                                }))
                                .child(design::icon_role(
                                    Icon::Close,
                                    24.,
                                    ColorRole::ON_SURFACE_VARIANT,
                                )),
                        ),
                )
                .child(
                    div()
                        .id("import-report-body")
                        .max_h(px(420.))
                        .overflow_y_scroll()
                        .child(report),
                ),
            )
            .into_any_element()
    }
}
