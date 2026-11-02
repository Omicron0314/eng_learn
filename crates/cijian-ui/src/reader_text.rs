#[cfg(test)]
mod tests;
use super::*;
use cijian_domain::analysis::{TokenKind, tokenize};
use cijian_domain::reading::TextRange;
use gpui_kit::Focusable;
use gpui_kit::{Entity, base::input::TextareaState, component::input::Textarea};

pub(super) struct ReaderText {
    key: (String, String),
    input: Entity<TextareaState>,
    words: Vec<std::ops::Range<usize>>,
}
impl Shell {
    pub(super) fn prepare_reader(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(article) = self.article.as_ref().filter(
            |article| matches!(self.navigation.current(),Route::Article{id,..} if id==&article.id),
        ) else {
            return;
        };
        let key = (article.id.clone(), article.body_hash.clone());
        if self
            .reader_text
            .as_ref()
            .is_some_and(|text| text.key == key)
        {
            return;
        }
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .default_value(article.body.clone())
                .auto_grow(1, 10000)
        });
        input.update(cx, |input, cx| input.set_readonly(true, cx));
        let words = tokenize(&article.body)
            .into_iter()
            .filter(|token| token.kind == TokenKind::Word)
            .map(|token| token.start..token.end)
            .collect();
        self.reader_text = Some(ReaderText { key, input, words });
    }
    fn lookup_reader_word(
        &mut self,
        position: Option<gpui_kit::Point<gpui_kit::Pixels>>,
        cx: &mut Context<Self>,
    ) {
        if self.loading && !self.lookup_loading {
            return;
        }
        let Some(reader) = &self.reader_text else {
            return;
        };
        let input = reader.input.read(cx);
        if position.is_some() && !input.selected_range().is_empty() {
            return;
        }
        let range = reader
            .words
            .iter()
            .find(|range| match position {
                Some(position) => input
                    .range_to_bounds(range)
                    .is_some_and(|bounds| bounds.contains(&position)),
                None => range.start <= input.cursor() && input.cursor() < range.end,
            })
            .cloned();
        let Some(range) = range else {
            return;
        };
        self.request(
            Command::Lookup {
                article: reader.key.0.clone(),
                body_hash: reader.key.1.clone(),
                range: TextRange {
                    start: range.start,
                    end: range.end,
                },
            },
            cx,
        );
    }
    pub(super) fn reader_text_body(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let Some(reader) = &self.reader_text else {
            return div().into_any_element();
        };
        div()
            .id("reader-text")
            .w_full()
            .text_size(px(20.))
            .line_height(px(32.))
            .child(
                Textarea::new(&reader.input)
                    .appearance(false)
                    .readonly(true)
                    .aria_label("文章正文，按Alt+Enter查看光标所在词")
                    .p_0()
                    .text_size(px(20.))
                    .line_height(px(32.)),
            )
            .on_mouse_up(
                gpui_kit::MouseButton::Left,
                cx.listener(|this, event: &gpui_kit::MouseUpEvent, _, cx| {
                    this.lookup_reader_word(Some(event.position), cx)
                }),
            )
            .on_key_down(cx.listener(|this, event: &gpui_kit::KeyDownEvent, _, cx| {
                if event.keystroke.key == "enter" && event.keystroke.modifiers.alt {
                    this.lookup_reader_word(None, cx);
                    cx.stop_propagation();
                }
            }))
            .into_any_element()
    }
    pub(super) fn dismiss_lookup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading && !self.lookup_loading {
            return;
        }
        if self.lookup_loading {
            self.request_id += 1;
            self.loading = false;
            self.lookup_loading = false;
        }
        self.lookup = None;
        self.focus_reader(window, cx);
        cx.notify();
    }
    pub(super) fn focus_reader(&self, window: &mut Window, cx: &mut App) {
        if let Some(reader) = &self.reader_text {
            window.focus(&reader.input.read(cx).focus_handle(cx), cx);
        }
    }
}
