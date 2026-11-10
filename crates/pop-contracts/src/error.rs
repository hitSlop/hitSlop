use crate::Code;
#[derive(Clone, Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct Error {
    pub code: Code,
    pub message: String,
    pub op_index: Option<usize>,
    /// Location within a checked value; independent of the batch's operation index.
    pub path: Vec<String>,
}
impl Error {
    pub fn new(code: Code, message: impl ToString) -> Self {
        Self { code, message: message.to_string(), op_index: None, path: vec![] }
    }
    pub fn at(mut self, segment: impl ToString) -> Self {
        self.path.insert(0, segment.to_string());
        self
    }
    /// RFC 6901 pointer; an empty string identifies the checked value itself.
    pub fn pointer(&self) -> String {
        self.path.iter().map(|s| format!("/{}", s.replace('~', "~0").replace('/', "~1"))).collect()
    }
}
pub type Result<T> = std::result::Result<T, Error>;
