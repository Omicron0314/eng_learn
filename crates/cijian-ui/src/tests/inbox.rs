use super::*;
use gpui_kit::{Bounds, TestAppContext, size, test::TestWindowExt};
use std::sync::Mutex;
const TARGET: &str = "Original targetword.";
fn body() -> String {
    (0..140)
        .map(|index| format!("Original filler line {index}.\n"))
        .collect::<String>()
        + TARGET
}
#[derive(Default)]
struct Bus(Mutex<Vec<Command>>);
impl CommandBus for Bus {
    fn dispatch(
        &self,
        command: Command,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = cijian_ports::Result<Response>> + Send + '_>,
    > {
        self.0.lock().unwrap().push(command.clone());
        let result = match command {
            Command::OpenArticle { id } => {
                let body = body();
                Ok(Response::Article {
                    article: Box::new(cijian_ports::articles::Article {
                        id,
                        title: "Original long article".into(),
                        body_hash: "original-source-hash".into(),
                        analysis: cijian_domain::analysis::analyze(
                            &body,
                            &Default::default(),
                            &Default::default(),
                            &Default::default(),
                            &Default::default(),
                        ),
                        body,
                        genre: cijian_domain::settings::Genre::Story,
                        tier: cijian_domain::settings::Tier::Easy,
                        diagnostics: Vec::new(),
                        created_at: 0,
                    }),
                    session: cijian_ports::reading::ReadingSession {
                        id: "original-session".into(),
                        article_id: "original-article".into(),
                        position: 0,
                        opened_at: 0,
                        completed_at: None,
                    },
                    audio: Vec::new(),
                    feedback: Vec::new(),
                })
            }
            _ => Err(AppError::new(
                cijian_ports::ErrorCode::InvalidInput,
                "原创测试只打开文章",
            )),
        };
        Box::pin(async move { result })
    }
}
#[gpui_kit::test]
fn inbox_source_link_reveals_the_marked_word_without_a_model_call(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    let text = body();
    let start = text.rfind("targetword").unwrap();
    let bus = Arc::new(Bus::default());
    let (handle, shell) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(gpui_kit::WindowBounds::Windowed(Bounds::new(
                        Default::default(),
                        size(px(1000.), px(900.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |_, cx| {
                    cx.new(|_| {
                        let mut shell = Shell::new();
                        shell.backend = Some(bus.clone());
                        shell.navigation.navigate(Route::Inbox);
                        shell.inbox_total = 1;
                        shell.inbox = vec![cijian_ports::inbox::InboxItem {
                            id: "original-inbox".into(),
                            lemma: "targetword".into(),
                            article_id: Some("original-article".into()),
                            article_title: Some("Original long article".into()),
                            context: TARGET.into(),
                            body_hash: Some("original-source-hash".into()),
                            range: Some(cijian_domain::reading::TextRange {
                                start,
                                end: start + "targetword".len(),
                            }),
                            revision: 0,
                            meaning: String::new(),
                        }];
                        shell
                    })
                },
            )
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("source-original-inbox", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        for _ in 0..6 {
            window.render_frame(cx);
        }
        let shell = shell.read(cx);
        let reader = shell.reader_text.as_ref().unwrap();
        let input = reader.input.read(cx);
        let bounds = input.range_to_bounds(&(start..start + 10)).unwrap();
        let viewport = shell.reader_scroll.bounds();
        assert!(
            bounds.top() >= viewport.top() && bounds.bottom() <= viewport.bottom(),
            "source word must be scrolled into view: {bounds:?}, {viewport:?}"
        );
        assert!(shell.navigation.current().section() == "articles");
        assert!(shell.source_reveal.is_none());
    })
    .unwrap();
    let commands = bus.0.lock().unwrap();
    assert!(
        commands
            .iter()
            .all(|command| matches!(command, Command::OpenArticle { .. }))
    );
}
