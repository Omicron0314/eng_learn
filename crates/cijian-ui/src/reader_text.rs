#[cfg(test)]
mod tests;
use super::*;
use cijian_domain::analysis::{TokenKind, tokenize};
use cijian_domain::reading::TextRange;
use gpui_kit::Focusable;
use gpui_kit::{
    Entity,
    base::input::{BufferPoint, DisplayPoint, TextareaState},
    component::input::Textarea,
};
use gpui_kit_material::color::ColorRole as cijian_ui_color;
const READER_LINE_HEIGHT: f32 = 34.;

pub(super) struct ReaderText {
    key: (String, String),
    pub(super) input: Entity<TextareaState>,
    words: Vec<std::ops::Range<usize>>,
    session: Option<String>,
    lines: Vec<usize>,
    length: usize,
    position: usize,
    restore: Option<usize>,
    width: Option<gpui_kit::Pixels>,
    visible: bool,
    initial_save: bool,
    regrow: bool,
}
impl Shell {
    pub(super) fn prepare_reader(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(article) = self.article.as_ref().filter(
            |article| matches!(self.navigation.current(),Route::Article{id,..} if id==&article.id),
        ) else {
            return;
        };
        let key = (article.id.clone(), article.body_hash.clone());
        let reveal = self
            .source_reveal
            .as_ref()
            .filter(|(id, hash, _)| id == &key.0 && hash == &key.1)
            .map(|(_, _, position)| *position)
            .filter(|position| *position <= article.body.len());
        let session = self
            .reading_session
            .as_ref()
            .filter(|session| session.article_id == article.id);
        let session_id = session.map(|session| session.id.clone());
        let visible = !matches!(
            self.navigation.current(),
            Route::Article {
                listening: true,
                ..
            }
        ) || self.show_listening_text;
        if let Some(text) = self
            .reader_text
            .as_mut()
            .filter(|text| text.key == key && text.session == session_id)
        {
            if let Some(position) = reveal {
                text.restore = Some(position);
                text.position = position;
                self.source_reveal = None;
            }
            if visible && !text.visible {
                text.restore = Some(text.position);
            }
            text.visible = visible;
            return;
        }
        let saved = session.map_or(0, |session| session.position);
        let mut position = self
            .local_reading_position(&article.id, &article.body_hash)
            .unwrap_or(saved)
            .min(article.body.len());
        if !article.body.is_char_boundary(position) {
            position = 0;
        }
        let length = article.body.len();
        let lines = std::iter::once(0)
            .chain(
                article
                    .body
                    .match_indices('\n')
                    .map(|(offset, _)| offset + 1),
            )
            .collect();
        self.reader_scroll.set_offset(Default::default());
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
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
        self.reader_text = Some(ReaderText {
            key,
            input,
            words,
            session: session_id,
            lines,
            length,
            position,
            restore: reveal.or((position > 0).then_some(position)),
            width: None,
            visible,
            initial_save: position != saved,
            regrow: true,
        });
        if reveal.is_some() {
            self.source_reveal = None;
        }
    }
    fn capture_reader_frame(&mut self, cx: &mut Context<Self>) {
        if self.reader_close_pending() {
            return;
        }
        let viewport = self.reader_scroll.bounds();
        let Some(reader) = self.reader_text.as_mut().filter(|reader| reader.visible) else {
            return;
        };
        if reader.regrow {
            reader.regrow = false;
            let input = reader.input.clone();
            input.update(cx, |input, cx| input.set_auto_grow(1, 10_000, cx));
            return;
        }
        let input = reader.input.read(cx);
        let rows = input.display_map().display_row_count();
        let bounds = input.input_bounds();
        let Some(document) = input.text_bounds() else {
            return;
        };
        if rows == 0 || viewport.size.height <= px(0.) || bounds.size.height <= px(0.) {
            return;
        }
        let line_height = input
            .prepaint_caret_geometry()
            .map_or(px(READER_LINE_HEIGHT), |geometry| geometry.line_height());
        if reader.width != Some(bounds.size.width) {
            if reader.width.is_some() && reader.restore.is_none() {
                reader.restore = Some(reader.position);
            }
            reader.width = Some(bounds.size.width);
            reader.regrow = true;
            return;
        }
        if let Some(position) = reader.restore {
            let line = reader
                .lines
                .partition_point(|start| *start <= position)
                .saturating_sub(1);
            let row = input
                .display_map()
                .buffer_pos_to_display_pos(BufferPoint::new(line, position - reader.lines[line]))
                .row;
            let target = line_height * row;
            let inner = input.scroll_offset();
            let visible_top = -inner.y;
            let desired = if target < visible_top {
                -target
            } else if target + line_height > visible_top + bounds.size.height {
                -(target - bounds.size.height / 2.)
            } else {
                inner.y
            };
            if desired != inner.y && document.size.height > bounds.size.height + px(1.) {
                let input = reader.input.clone();
                input.update(cx, |input, cx| {
                    input.set_scroll_offset(gpui_kit::point(inner.x, desired), cx)
                });
                cx.notify();
                return;
            }
            let offset = self.reader_scroll.offset();
            self.reader_scroll.set_offset(gpui_kit::point(
                offset.x,
                offset.y + viewport.top() + px(8.) - document.top() - target,
            ));
            reader.restore = None;
            cx.notify();
            return;
        }
        if bounds.top() >= viewport.bottom() {
            return;
        }
        let position = if bounds.bottom() <= viewport.top() {
            reader.length
        } else {
            let top = viewport.top().max(bounds.top());
            let row =
                (((top - document.top()) / line_height).ceil().max(0.) as usize).min(rows - 1);
            let point = input
                .display_map()
                .display_pos_to_buffer_pos(DisplayPoint::new(row, 0));
            reader
                .lines
                .get(point.line)
                .map_or(reader.length, |start| start + point.col)
                .min(reader.length)
        };
        if position == reader.position && !reader.initial_save {
            return;
        }
        reader.position = position;
        reader.initial_save = false;
        if let Some(session) = reader.session.clone() {
            let (article, hash) = reader.key.clone();
            self.queue_reading_position(article, hash, session, position, cx);
        }
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
        let weak = cx.entity().downgrade();
        let highlight = self.lookup.as_ref().and_then(|(context, _, _)| {
            reader
                .input
                .read(cx)
                .range_to_bounds(&(context.selection.range.start..context.selection.range.end))
                .map(|bounds| (bounds.origin, bounds.size))
        });
        let (reader_size, reader_line) = if self.compact { (18., 30.) } else { (20., 34.) };
        let mut text = div()
            .id("reader-text")
            .test_support()
            .relative()
            .w_full()
            .text_size(px(reader_size))
            .line_height(px(reader_line));
        if let Some((origin, size)) = highlight {
            text = text.child(
                gpui_kit::canvas(
                    |_, _, _| (),
                    move |canvas_bounds, _, window, _| {
                        let rect = gpui_kit::Bounds {
                            origin: gpui_kit::point(
                                origin.x - canvas_bounds.origin.x - px(3.),
                                origin.y - canvas_bounds.origin.y - px(1.),
                            ),
                            size: gpui_kit::size(size.width + px(6.), size.height + px(2.)),
                        };
                        window.paint_quad(gpui_kit::quad(
                            rect,
                            gpui_kit::Corners::all(px(6.)),
                            crate::design::c(cijian_ui_color::PRIMARY_CONTAINER),
                            gpui_kit::Edges::all(px(2.)),
                            crate::design::c(cijian_ui_color::PRIMARY),
                            gpui_kit::BorderStyle::Solid,
                        ));
                    },
                )
                .absolute()
                .inset_0(),
            );
        }
        text.child(
            Textarea::new(&reader.input)
                .appearance(false)
                .readonly(true)
                .aria_label("文章正文，按Alt+Enter查看光标所在词")
                .p_0()
                .text_size(px(20.))
                .line_height(px(34.))
                .letter_spacing(px(-0.2)),
        )
        .child(
            gpui_kit::canvas(
                |_, _, _| (),
                move |_, _, _, cx| {
                    let _ = weak.update(cx, |this, cx| this.capture_reader_frame(cx));
                },
            )
            .absolute()
            .size_0(),
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
