use std::io;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
  #[error("CLI execution failed: {0}")]
  CliExec(#[from] io::Error),

  #[error("UTF-8 conversion error: {0}")]
  Utf8Error(#[from] std::string::FromUtf8Error),
}
