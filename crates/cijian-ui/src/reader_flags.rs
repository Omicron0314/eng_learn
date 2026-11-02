#[cfg(test)]
mod tests;
use super::*;
use cijian_domain::reading::{TextRange, sentences, validate_flag};
use cijian_ports::reading::SentenceFlag;
use gpui_kit::{Entity, base::input::TextareaState};
use gpui_kit_material::components::{MaterialCheckbox, MaterialDialog, TextField};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct FlagState {
    pub open: bool,
    drafts: BTreeMap<(String, String), FlagDraft>,
}
struct FlagDraft {
    note: Entity<TextareaState>,
    ranges: Vec<TextRange>,
    submission: Option<(SentenceFlag, String)>,
}
impl Shell {
    pub(super) fn prepare_flag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.reader_flags.open {
            return;
        }
        let Some(article) = &self.article else {
            return;
        };
        let key = (article.id.clone(), article.body_hash.clone());
        self.reader_flags.drafts.entry(key).or_insert_with(|| {
            let note = cx.new(|cx| TextareaState::new(window, cx));
            cx.observe(&note, |_, _, cx| cx.notify()).detach();
            FlagDraft {
                note,
                ranges: Vec::new(),
                submission: None,
            }
        });
    }
    pub(super) fn flag_saved(&mut self, article: String, body_hash: String) {
        self.reader_flags.drafts.remove(&(article, body_hash));
        self.reader_flags.open = false;
        self.notice = Some("句子反馈已保存".into());
    }
    fn submit_flag(&mut self, cx: &mut Context<Self>) {
        let Some(article) = &self.article else {
            return;
        };
        let (article, body_hash, body) = (
            article.id.clone(),
            article.body_hash.clone(),
            article.body.clone(),
        );
        let next_operation = self.operation("sentence-flag");
        let Some(draft) = self
            .reader_flags
            .drafts
            .get_mut(&(article.clone(), body_hash.clone()))
        else {
            return;
        };
        let flag = SentenceFlag {
            ranges: draft.ranges.clone(),
            note: draft.note.read(cx).value().to_string(),
        };
        if let Err(error) = validate_flag(&body, &flag.ranges, &flag.note) {
            self.error = Some(error.to_string());
            cx.notify();
            return;
        }
        let operation = match &draft.submission {
            Some((previous, operation))
                if previous.ranges == flag.ranges && previous.note == flag.note =>
            {
                operation.clone()
            }
            _ => {
                draft.submission = Some((flag.clone(), next_operation.clone()));
                next_operation
            }
        };
        self.request(
            Command::FlagSentences {
                article,
                body_hash,
                flag,
                operation,
            },
            cx,
        );
    }
    pub(super) fn flag_dialog(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        if !self.reader_flags.open {
            return div().into_any_element();
        }
        let Some(article) = &self.article else {
            return div().into_any_element();
        };
        let key = (article.id.clone(), article.body_hash.clone());
        let Some(draft) = self.reader_flags.drafts.get(&key) else {
            return div().into_any_element();
        };
        let note = draft.note.read(cx).value();
        let error = validate_flag(&article.body, &draft.ranges, &note).err();
        let mut content = div()
            .flex()
            .flex_col()
            .gap_3()
            .child("选择不自然的句子，也可以只填写备注。关闭面板会保留本次窗口中的草稿。");
        for (index, range) in sentences(&article.body).into_iter().enumerate() {
            let key = key.clone();
            content = content.child(
                MaterialCheckbox::new(
                    gpui_kit::SharedString::from(format!("flag-sentence-{index}")),
                    range.text(&article.body).unwrap_or_default().to_owned(),
                )
                .show_label(true)
                .checked(draft.ranges.contains(&range))
                .disabled(self.loading)
                .on_change(cx.listener(move |this, _, _, cx| {
                    if let Some(draft) = this.reader_flags.drafts.get_mut(&key) {
                        if draft.ranges.contains(&range) {
                            draft.ranges.retain(|value| *value != range);
                        } else {
                            draft.ranges.push(range);
                            draft.ranges.sort_by_key(|value| value.start);
                        }
                    }
                    cx.notify();
                })),
            );
        }
        content = content.child(
            TextField::multi_line("sentence-note", &draft.note)
                .label("备注（最多200字符）")
                .disabled(self.loading)
                .error(note.chars().count() > 200)
                .supporting_text(format!("{}/200", note.chars().count())),
        );
        if let Some(error) = &self.error {
            content = content.child(error.clone());
        }
        let weak = cx.entity().downgrade();
        MaterialDialog::new("sentence-flag-dialog")
            .headline("反馈不自然的句子")
            .modal(true)
            .dismiss_on_scrim(false)
            .content(content)
            .on_dismiss(move |_, cx| {
                let _ = weak.update(cx, |this, cx| {
                    if !this.loading {
                        this.reader_flags.open = false;
                        cx.notify();
                    }
                });
            })
            .action(
                material_button("close-sentence-feedback", ButtonVariant::Text)
                    .child("暂时关闭")
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.reader_flags.open = false;
                        cx.notify();
                    })),
            )
            .action(
                material_button("submit-sentence-feedback", ButtonVariant::Filled)
                    .child("提交反馈")
                    .disabled(self.loading || error.is_some())
                    .on_click(cx.listener(|this, _, _, cx| this.submit_flag(cx))),
            )
            .into_any_element()
    }
}
