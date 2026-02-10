use std::io;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
  #[error("CLI execution failed: {0}")]
  CliExec(#[from] io::Error),

  #[error("UTF-8 conversion error: {0}")]
  Utf8Error(#[from] std::string::FromUtf8Error),

  #[error("JSON parse error: {0}")]
  JsonError(#[from] serde_json::Error),

  #[error("CLI command failed: {0}")]
  CliFailure(String),
}
