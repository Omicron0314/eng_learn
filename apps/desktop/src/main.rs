//! Desktop composition root. Optional --data-dir permits isolated acceptance runs.
mod backend;
mod import_file;
use std::{path::PathBuf, sync::Arc};
fn main() {
    let mut arguments = std::env::args().skip(1);
    let directory = match arguments.next() {
        Some(flag) if flag == "--data-dir" => match arguments.next() {
            Some(path) => Ok(PathBuf::from(path)),
            None => {
                eprintln!("--data-dir 需要目录参数");
                std::process::exit(2);
            }
        },
        Some(_) => {
            eprintln!("用法：cijian-desktop [--data-dir 目录]");
            std::process::exit(2);
        }
        None => directories::ProjectDirs::from("org", "cijian", "cijian")
            .map(|dirs| dirs.data_dir().to_owned())
            .ok_or_else(|| {
                cijian_ports::AppError::new(
                    cijian_ports::ErrorCode::StorageUnavailable,
                    "无法确定本地数据目录，请使用 --data-dir 指定",
                )
            }),
    };
    if arguments.next().is_some() {
        eprintln!("存在多余参数");
        std::process::exit(2);
    }
    let backend = directory
        .and_then(|directory| backend::DesktopBackend::open(&directory))
        .map(|backend| Arc::new(backend) as Arc<dyn cijian_app::commands::CommandBus>);
    cijian_ui::run(gpui_kit::application(), backend);
}
