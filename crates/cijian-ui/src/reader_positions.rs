#[cfg(test)]
mod tests;
use super::*;
use gpui_kit_material::components::MaterialDialog;
use std::collections::BTreeMap;
#[derive(Clone)]
struct PositionWrite {
    article: String,
    body_hash: String,
    session: String,
    position: usize,
    revision: u64,
}
impl PositionWrite {
    fn command(&self) -> Command {
        Command::SaveReadingPosition {
            article: self.article.clone(),
            body_hash: self.body_hash.clone(),
            session: self.session.clone(),
            position: self.position,
            revision: self.revision,
        }
    }
}
#[derive(Default)]
pub(super) struct Positions {
    pending: BTreeMap<String, PositionWrite>,
    inflight: BTreeMap<String, u64>,
    latest: BTreeMap<(String, String), usize>,
    next_revision: u64,
    epoch: u64,
    error: Option<String>,
    closing: Option<gpui_kit::AnyWindowHandle>,
    close_failed: bool,
    quit: Option<gpui_kit::Subscription>,
}
impl Shell {
    pub(super) fn reader_close_pending(&self) -> bool {
        self.positions.closing.is_some()
    }
    pub(super) fn initialize_positions(&mut self, cx: &mut Context<Self>) {
        if self.positions.quit.is_some() {
            return;
        }
        self.positions.quit = Some(cx.on_app_quit(|this, cx| {
            let pending = this.positions.pending.values().cloned().collect::<Vec<_>>();
            let backend = this.backend.clone();
            // GPUI gives quit observers a bounded shutdown interval; no UI-thread IO.
            cx.background_executor().spawn(async move {
                if let Some(backend) = backend {
                    for write in pending {
                        let _ = backend.dispatch(write.command()).await;
                    }
                }
            })
        }));
    }
    pub(super) fn local_reading_position(&self, article: &str, body_hash: &str) -> Option<usize> {
        self.positions
            .latest
            .get(&(article.into(), body_hash.into()))
            .copied()
    }
    pub(super) fn queue_reading_position(
        &mut self,
        article: String,
        body_hash: String,
        session: String,
        position: usize,
        cx: &mut Context<Self>,
    ) {
        self.positions
            .latest
            .insert((article.clone(), body_hash.clone()), position);
        self.positions.next_revision += 1;
        let revision = self.positions.next_revision;
        self.positions.pending.insert(
            session.clone(),
            PositionWrite {
                article,
                body_hash,
                session,
                position,
                revision,
            },
        );
        self.positions.epoch += 1;
        let epoch = self.positions.epoch;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(250))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.positions.epoch == epoch {
                    this.flush_positions(cx);
                }
            });
        })
        .detach();
    }
    pub(super) fn flush_positions(&mut self, cx: &mut Context<Self>) {
        let Some(backend) = self.backend.clone() else {
            return;
        };
        let writes = self
            .positions
            .pending
            .values()
            .filter(|write| self.positions.inflight.get(&write.session) != Some(&write.revision))
            .cloned()
            .collect::<Vec<_>>();
        for write in writes {
            self.positions
                .inflight
                .insert(write.session.clone(), write.revision);
            let backend = backend.clone();
            let command = write.command();
            let task = cx
                .background_executor()
                .spawn(async move { backend.dispatch(command).await });
            cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                    if this.positions.inflight.get(&write.session) == Some(&write.revision) {
                        this.positions.inflight.remove(&write.session);
                    }
                    if this
                        .positions
                        .pending
                        .get(&write.session)
                        .is_some_and(|pending| pending.revision == write.revision)
                    {
                        match result {
                            Ok(Response::ReadingPositionSaved {
                                session,
                                position,
                                revision,
                            }) if session == write.session
                                && position == write.position
                                && revision == write.revision =>
                            {
                                this.positions.pending.remove(&write.session);
                                if let Some(session) = this
                                    .reading_session
                                    .as_mut()
                                    .filter(|session| session.id == write.session)
                                {
                                    session.position = write.position;
                                }
                            }
                            Err(error) => {
                                this.positions.error = Some(error.message);
                                this.positions.close_failed = this.positions.closing.is_some();
                            }
                            _ => {
                                this.positions.error = Some("阅读位置未获得保存确认".into());
                                this.positions.close_failed = this.positions.closing.is_some();
                            }
                        }
                    }
                    if this.positions.pending.is_empty() {
                        this.positions.error = None;
                        if !this.positions.close_failed
                            && let Some(window) = this.positions.closing.take()
                        {
                            let _ = window.update(cx, |_, window, _| window.remove_window());
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }
    pub(super) fn close_reader_when_saved(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.positions.pending.is_empty() {
            return true;
        }
        self.positions.closing = Some(window.window_handle());
        if self.positions.error.is_some() {
            self.positions.close_failed = true;
            cx.notify();
        } else {
            self.flush_positions(cx);
        }
        false
    }
    pub(super) fn position_notice(&self, cx: &mut Context<Self>) -> Option<gpui_kit::AnyElement> {
        let error = self.positions.error.as_ref()?;
        Some(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(format!("部分阅读位置尚未保存：{error}"))
                .child(
                    material_button("retry-reading-position", ButtonVariant::Text)
                        .child("重试保存位置")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.positions.error = None;
                            this.flush_positions(cx);
                            cx.notify();
                        })),
                )
                .into_any_element(),
        )
    }
    pub(super) fn position_dialog(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        if !self.positions.close_failed {
            return div().into_any_element();
        }
        let weak = cx.entity().downgrade();
        MaterialDialog::new("position-close-dialog")
            .headline("阅读位置尚未保存")
            .supporting_text("可以重试，也可以直接关闭；尚未保存的阅读位置可能丢失。")
            .modal(true)
            .dismiss_on_scrim(false)
            .on_dismiss(move |_, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.positions.close_failed = false;
                    this.positions.closing = None;
                    cx.notify();
                });
            })
            .action(
                material_button("keep-reader-open", ButtonVariant::Text)
                    .child("继续使用")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.positions.close_failed = false;
                        this.positions.closing = None;
                        cx.notify();
                    })),
            )
            .action(
                material_button("retry-position-close", ButtonVariant::Filled)
                    .child("重试并关闭")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.positions.error = None;
                        this.positions.close_failed = false;
                        this.flush_positions(cx);
                        cx.notify();
                    })),
            )
            .action(
                material_button("discard-position-close", ButtonVariant::Text)
                    .child("直接关闭")
                    .on_click(cx.listener(|this, _, window, _| {
                        this.positions.pending.clear();
                        this.positions.epoch += 1;
                        this.positions.closing = None;
                        this.positions.close_failed = false;
                        window.remove_window();
                    })),
            )
            .into_any_element()
    }
}
