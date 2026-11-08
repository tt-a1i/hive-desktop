use crate::language::{english, tr};

pub(crate) enum Status {
    Message(&'static str),
    Interaction { backend: &'static str, phase: &'static str },
    InvalidInteraction(&'static str),
}
impl From<&'static str> for Status {
    fn from(key: &'static str) -> Self { Self::Message(key) }
}
impl Status {
    pub(crate) fn is_background_connected(&self) -> bool {
        matches!(self, Self::Message("后台已连接"))
    }
    pub(crate) fn text(&self) -> String {
        match self {
            Self::Message(key) => tr(key).to_owned(),
            Self::Interaction { backend, phase } => format!("{} {}",tr(backend),tr(phase)),
            Self::InvalidInteraction(backend) => if english() {
                format!("Invalid {} interaction; stop this turn to continue",tr(backend))
            } else { format!("{} 交互格式错误，可停止本轮",tr(backend)) },
        }
    }
}

pub(crate) enum ErrorNotice {
    Message(&'static str),
    Raw(String),
    Detail(&'static str, String),
}
impl From<&'static str> for ErrorNotice {
    fn from(key: &'static str) -> Self { Self::Message(key) }
}
impl From<String> for ErrorNotice {
    fn from(error: String) -> Self { Self::Raw(error) }
}
impl ErrorNotice {
    pub(crate) fn text(&self) -> String {
        match self {
            Self::Message(key) => tr(key).to_owned(),
            Self::Raw(error) => error.clone(),
            Self::Detail(key,error) => format!("{}: {error}",tr(key)),
        }
    }
}
