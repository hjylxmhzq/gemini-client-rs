use gemini_client::{GeminiClient, GeminiModel, UserMessage};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut client = GeminiClient::new("AIzaSyA0h7cfIsH8ltSmVR01mUNXgxeTBv2gp9k", GeminiModel::Pro);
  client.send_message(UserMessage::new("help me write some js code")).await.unwrap();
  Ok(())
}
