use super::base::{FunctionDeclaration, FunctionDeclarationName};

pub fn get_weather_fn() -> FunctionDeclaration {
  FunctionDeclaration {
    name: FunctionDeclarationName::GetWeather,
    description: String::from("Get the weather for a given location"),
    parameters: serde_json::json!({
      "type": "OBJECT",
      "properties": {
        "location": {
          "type": "STRING",
          "description": "The city and state, e.g. San Francisco, CA"
        },
        "date": {
          "type": "STRING",
          "description": "The date for the weather in ISO date format, e.g. 1970-01-01T00:00:00.000Z"
        }
      },
      "required": ["description"]
    }),
  }
}
