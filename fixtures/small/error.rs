#[derive(Debug)]
pub enum AppError {
    Io(std::io::Error),
    Parse(String),
    NotFound(String),
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err)
    }
}
