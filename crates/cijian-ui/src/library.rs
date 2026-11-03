use super::*;
use cijian_domain::srs::State;
use cijian_ports::library::{BookEntryPage, BookView, DeletePreview};
use gpui_kit::{
    Entity,
    base::input::{InputState, TextareaState},
};
use gpui_kit_material::components::{MaterialDialog, TextField};
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
    pub import_source: Option<String>,
    pub error_page: usize,
}
pub(super) struct CardEditor {
    pub card: CardView,
    meaning: Entity<TextareaState>,
    example: Entity<TextareaState>,
}
impl CardEditor {
    fn new(card: CardView, window: &mut Window, cx: &mut Context<Shell>) -> Self {
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
        State::New => "新卡",
        State::Learning => "学习中",
        State::Review => "复习中",
        State::Relearning => "重学中",
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
    pub(super) fn library_controls(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let (selected, search) = match self.navigation.current() {
            Route::Library { book, search } => (book.clone(), search.clone()),
            _ => (None, String::new()),
        };
        let baseline = selected
            .as_ref()
            .and_then(|id| self.library_ui.books.iter().find(|book| &book.id == id))
            .is_some_and(|book| book.kind == BookKind::Baseline);
        let mut books = div().flex().flex_wrap().gap_2().child(
            material_button(
                "all-books",
                if selected.is_none() {
                    ButtonVariant::Filled
                } else {
                    ButtonVariant::Tonal
                },
            )
            .child("全部词卡")
            .on_click(cx.listener(|this, _, _, cx| {
                this.library_ui.page = 0;
                this.navigate(
                    Route::Library {
                        book: None,
                        search: String::new(),
                    },
                    cx,
                );
            })),
        );
        for (index, book) in self.library_ui.books.iter().enumerate() {
            let id = book.id.clone();
            let search = search.clone();
            let baseline = book.kind == BookKind::Baseline;
            books = books.child(
                material_button(
                    ("book-filter", index),
                    if selected.as_ref() == Some(&book.id) {
                        ButtonVariant::Filled
                    } else {
                        ButtonVariant::Tonal
                    },
                )
                .child(format!(
                    "{} · {} · {} 词",
                    book.name,
                    if baseline { "基线" } else { "学习" },
                    book.entries
                ))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.library_ui.page = 0;
                    if baseline {
                        this.library_ui.state = None;
                        this.library_ui.suspended = None;
                    }
                    this.navigate(
                        Route::Library {
                            book: Some(id.clone()),
                            search: search.clone(),
                        },
                        cx,
                    );
                })),
            );
        }
        let mut body = div()
            .flex()
            .flex_col()
            .gap_3()
            .child("我的词书")
            .child(books);
        if let Some(input) = &self.library_ui.search {
            body = body
                .child(TextField::new("library-search", input).label("搜索单词或释义"))
                .child(
                    material_button("search-library", ButtonVariant::Tonal)
                        .child("搜索")
                        .disabled(self.loading)
                        .on_click(cx.listener(|this, _, _, cx| this.search_library(cx))),
                );
        }
        body = body.child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .child(
                    material_button("library-state", ButtonVariant::Tonal)
                        .child(format!(
                            "状态：{}",
                            self.library_ui.state.map(state_label).unwrap_or("全部")
                        ))
                        .disabled(self.loading || baseline)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.library_ui.state = match this.library_ui.state {
                                None => Some(State::New),
                                Some(State::New) => Some(State::Learning),
                                Some(State::Learning) => Some(State::Review),
                                Some(State::Review) => Some(State::Relearning),
                                Some(State::Relearning) => None,
                            };
                            this.library_ui.page = 0;
                            this.request(
                                Command::LoadLibrary {
                                    filter: this.library_filter(),
                                },
                                cx,
                            );
                        })),
                )
                .child(
                    material_button("library-suspended", ButtonVariant::Tonal)
                        .child(match self.library_ui.suspended {
                            None => "暂停：全部",
                            Some(true) => "仅暂停／归档",
                            Some(false) => "仅未暂停",
                        })
                        .disabled(self.loading || baseline)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.library_ui.suspended = match this.library_ui.suspended {
                                None => Some(false),
                                Some(false) => Some(true),
                                Some(true) => None,
                            };
                            this.library_ui.page = 0;
                            this.request(
                                Command::LoadLibrary {
                                    filter: this.library_filter(),
                                },
                                cx,
                            );
                        })),
                ),
        );
        if let Some(book) = selected {
            body = body.child(
                material_button("delete-book-preview", ButtonVariant::Text)
                    .child("查看删除词书的影响")
                    .disabled(self.loading)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.request(Command::PreviewDeleteBook { book: book.clone() }, cx)
                    })),
            );
        }
        body = body.child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .child(
                    material_button("toggle-import", ButtonVariant::Filled)
                        .child(if self.library_ui.import_open {
                            "收起导入"
                        } else {
                            "导入词卡或词表"
                        })
                        .disabled(self.loading)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.library_ui.import_open = !this.library_ui.import_open;
                            cx.notify();
                        })),
                )
                .child(
                    material_button("toggle-import-report", ButtonVariant::Text)
                        .child("最近导入结果")
                        .disabled(self.report.is_none())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.library_ui.report_open = !this.library_ui.report_open;
                            cx.notify();
                        })),
                ),
        );
        let total = self
            .library_ui
            .entries
            .as_ref()
            .map(|page| page.total)
            .or_else(|| self.library.as_ref().map(|page| page.total))
            .unwrap_or(0);
        body = body.child(
            div()
                .flex()
                .gap_2()
                .child(
                    material_button("library-previous", ButtonVariant::Text)
                        .child("上一页")
                        .disabled(self.loading || self.library_ui.page == 0)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.library_ui.page = this.library_ui.page.saturating_sub(1);
                            this.request(
                                Command::LoadLibrary {
                                    filter: this.library_filter(),
                                },
                                cx,
                            );
                        })),
                )
                .child(format!(
                    "第 {} 页 / 共 {} 项",
                    self.library_ui.page + 1,
                    total
                ))
                .child(
                    material_button("library-next", ButtonVariant::Text)
                        .child("下一页")
                        .disabled(
                            self.loading || (u64::from(self.library_ui.page) + 1) * 30 >= total,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.library_ui.page = this.library_ui.page.saturating_add(1);
                            this.request(
                                Command::LoadLibrary {
                                    filter: this.library_filter(),
                                },
                                cx,
                            );
                        })),
                ),
        );
        body.into_any_element()
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
    pub(super) fn library_card(
        &self,
        card: &CardView,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let edit = card.clone();
        let toggle = card.clone();
        div()
            .rounded(px(16.))
            .p_4()
            .bg(color(ColorRole::SURFACE_CONTAINER_LOWEST))
            .flex()
            .flex_col()
            .gap_2()
            .child(card.front.clone())
            .child(card.meaning.clone())
            .child(card.example.clone())
            .child(format!(
                "{} · {}",
                state_label(card.srs.state),
                if card.archived {
                    "已归档"
                } else if card.suspended {
                    "已暂停"
                } else {
                    "正常"
                }
            ))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        material_button(
                            gpui_kit::SharedString::from(format!("edit-card-{}", card.id)),
                            ButtonVariant::Tonal,
                        )
                        .child("编辑释义与例句")
                        .disabled(self.loading)
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.library_ui.editor =
                                    Some(CardEditor::new(edit.clone(), window, cx));
                                this.library_ui.discard = false;
                                this.error = None;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        material_button(
                            gpui_kit::SharedString::from(format!("suspend-card-{}", card.id)),
                            ButtonVariant::Text,
                        )
                        .child(if card.suspended {
                            "恢复复习"
                        } else {
                            "暂停复习"
                        })
                        .disabled(self.loading || card.archived)
                        .on_click(cx.listener(move |this, _, _, cx| {
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
                        })),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn baseline_entries(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let mut body = div()
            .flex()
            .flex_col()
            .gap_2()
            .child("基线词表：这些词进入已知集合，不因此创建复习卡。");
        if let Some(page) = &self.library_ui.entries {
            for entry in &page.entries {
                body = body.child(
                    div()
                        .p_3()
                        .rounded(px(12.))
                        .bg(color(ColorRole::SURFACE_CONTAINER_LOWEST))
                        .child(format!("{} · 原始词条：{}", entry.lemma, entry.raw)),
                );
                if let Some(card) = &entry.card {
                    body = body.child(self.library_card(card, cx));
                }
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
    pub(super) fn library_dialogs(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        if self.library_ui.discard {
            let weak = cx.entity().downgrade();
            return MaterialDialog::new("discard-card-draft")
                .headline("放弃未保存的词卡修改？")
                .modal(true)
                .dismiss_on_scrim(false)
                .dismiss_on_escape(true)
                .on_dismiss(move |_, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.library_ui.discard = false;
                        this.library_ui.pending_close = false;
                        this.library_ui.pending_route = None;
                        this.library_ui.pending_back = false;
                        cx.notify();
                    });
                })
                .action(
                    material_button("keep-card-draft", ButtonVariant::Tonal)
                        .child("继续编辑")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.library_ui.discard = false;
                            this.library_ui.pending_close = false;
                            this.library_ui.pending_route = None;
                            this.library_ui.pending_back = false;
                            cx.notify();
                        })),
                )
                .action(
                    material_button("discard-card-draft-confirm", ButtonVariant::Text)
                        .child("放弃修改")
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
                        })),
                )
                .into_any_element();
        }
        if let Some(editor) = &self.library_ui.editor {
            let weak = cx.entity().downgrade();
            let mut dialog = MaterialDialog::new("edit-card-dialog")
                .headline(format!("编辑 {}", editor.card.front))
                .modal(true)
                .content(
                    TextField::multi_line("card-meaning", &editor.meaning)
                        .label("中文释义")
                        .max_length(1000),
                )
                .content(
                    TextField::multi_line("card-example", &editor.example)
                        .label("例句")
                        .max_length(5000),
                )
                .on_dismiss(move |_, cx| {
                    let _ = weak.update(cx, |this, cx| this.close_card_editor(cx));
                })
                .action(
                    material_button("cancel-card-edit", ButtonVariant::Text)
                        .child("取消")
                        .disabled(self.loading)
                        .on_click(cx.listener(|this, _, _, cx| this.close_card_editor(cx))),
                )
                .action(
                    material_button("save-card-edit", ButtonVariant::Filled)
                        .child("保存修改")
                        .disabled(self.loading || !editor.dirty(cx))
                        .on_click(cx.listener(|this, _, _, cx| {
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
                        })),
                );
            if let Some(error) = &self.error {
                dialog = dialog.supporting_text(error.clone());
            }
            return dialog.into_any_element();
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
