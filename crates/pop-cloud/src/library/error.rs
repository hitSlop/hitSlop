use crate::Error;
pub(super) fn failed(error: impl Into<Error>) -> Error {
    error.into()
}
