use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FunctionDeclarationName {
  GetWeather,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDeclaration {
  pub name: FunctionDeclarationName,
  pub description: String,
  pub parameters: serde_json::Value,
}
