//! Application commands and queries; coordinates domain and ports.
pub mod commands;
pub mod generation;
use cijian_domain::settings::{Settings, ValidatedSettings};
use cijian_ports::{AppError, ErrorCode, Result, SaveSettings, SettingsRepository};

pub async fn save_settings(
    repository: &impl SettingsRepository,
    mut command: SaveSettings,
    draft: Settings,
) -> Result<u64> {
    command.value = ValidatedSettings::try_from(draft)
        .map_err(|error| AppError::new(ErrorCode::InvalidInput, error.to_string()))?;
    repository.save_settings(command).await
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Today,
    Review,
    Articles,
    Article {
        id: String,
        listening: bool,
    },
    Library {
        book: Option<String>,
        search: String,
    },
    Inbox,
    Stats,
    Settings,
}
impl Route {
    pub fn section(&self) -> &'static str {
        match self {
            Self::Today => "today",
            Self::Review => "review",
            Self::Articles | Self::Article { .. } => "articles",
            Self::Library { .. } => "library",
            Self::Inbox => "inbox",
            Self::Stats => "stats",
            Self::Settings => "settings",
        }
    }
    pub fn label(&self) -> &'static str {
        match self.section() {
            "today" => "今日",
            "review" => "复习",
            "articles" => "文章",
            "library" => "词库",
            "inbox" => "生词箱",
            "stats" => "统计",
            _ => "设置",
        }
    }
    pub fn section_route(value: &str) -> Option<Self> {
        Some(match value {
            "today" => Self::Today,
            "review" => Self::Review,
            "articles" => Self::Articles,
            "library" => Self::Library {
                book: None,
                search: String::new(),
            },
            "inbox" => Self::Inbox,
            "stats" => Self::Stats,
            "settings" => Self::Settings,
            _ => return None,
        })
    }
}
#[derive(Debug, Clone)]
pub struct Navigation {
    current: Route,
    history: Vec<Route>,
}
impl Default for Navigation {
    fn default() -> Self {
        Self {
            current: Route::Today,
            history: Vec::new(),
        }
    }
}
impl Navigation {
    pub fn current(&self) -> &Route {
        &self.current
    }
    pub fn can_back(&self) -> bool {
        !self.history.is_empty()
    }
    pub fn navigate(&mut self, route: Route) {
        if route != self.current {
            self.history
                .push(std::mem::replace(&mut self.current, route));
        }
    }
    pub fn back(&mut self) {
        if let Some(route) = self.history.pop() {
            self.current = route;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detail_belongs_to_articles_and_back_retains_library_filter() {
        let mut nav = Navigation::default();
        let library = Route::Library {
            book: Some("book".into()),
            search: "test".into(),
        };
        nav.navigate(library.clone());
        nav.navigate(Route::Article {
            id: "a".into(),
            listening: true,
        });
        assert_eq!(nav.current().section(), "articles");
        nav.back();
        assert_eq!(nav.current(), &library);
    }
}
