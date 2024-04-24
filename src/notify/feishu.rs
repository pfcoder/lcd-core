use serde_json::{json, Value};

use crate::error::MinerError;

/// feishu api to query sheet
use std::sync::Mutex;

lazy_static! {
    static ref APP_ID: Mutex<Option<String>> = Mutex::new(None);
    static ref APP_SECRET: Mutex<Option<String>> = Mutex::new(None);
    static ref BOT: Mutex<Option<String>> = Mutex::new(None);
}

pub fn init(app_id: &str, app_secret: &str, bot: &str) {
    *APP_ID.lock().unwrap() = Some(app_id.to_string());
    *APP_SECRET.lock().unwrap() = Some(app_secret.to_string());
    *BOT.lock().unwrap() = Some(bot.to_string());
}

async fn get_access_token() -> Result<String, MinerError> {
    let url = format!("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal/");
    let client = reqwest::Client::new();
    let res: Value = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&json!({
            "app_id": APP_ID.lock().unwrap().as_ref().unwrap(),
            "app_secret": APP_SECRET.lock().unwrap().as_ref().unwrap(),
        })) // Convert JSON body to string
        .send()
        .await?
        .json()
        .await?;

    Ok(res["tenant_access_token"].as_str().unwrap().to_string())
}

pub async fn query_sheet(sheets_id: &str, sheet_id: &str) -> Result<Value, MinerError> {
    let token = get_access_token().await?;
    let url = format!(
        "https://open.feishu.cn/open-apis/sheets/v2/spreadsheets/{}/values/{}",
        sheets_id, sheet_id
    );
    let client = reqwest::Client::new();
    let res = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .json()
        .await?;

    Ok(res)
}

pub async fn notify(msg: &str) {
    let url = format!(
        "https://open.feishu.cn/open-apis/bot/v2/hook/{}",
        BOT.lock().unwrap().as_ref().unwrap()
    );
    let client = reqwest::Client::new();
    let _ = client
        .post(url)
        .header("Content-Type", "application/json")
        .json(&json!({
            "msg_type": "text",
            "content": {
                "text": msg
            }
        })) // Convert JSON body to string
        .send()
        .await;
}

//test
#[cfg(test)]
mod tests {
    use log::info;

    use super::*;

    lazy_static! {
        static ref SETUP: () = {
            env_logger::init();
            let cli_id = std::env::var("CLIENT_ID").expect("CLIENT_ID is not set in env");
            let secret = std::env::var("SECRET").expect("SECRET is not set in env");
            let bot = std::env::var("BOT").expect("BOT is not set in env");
            init(&cli_id, &secret, &bot);
        };
    }

    #[tokio::test]
    async fn test_get_access_token() {
        let _ = &*SETUP;
        let token = get_access_token().await.unwrap();
        info!("token: {}", token);
        assert!(token.len() > 0);
    }

    #[tokio::test]
    async fn test_query_sheet() {
        let _ = &*SETUP;
        let res = query_sheet("PwjYsZoefh6rXZt3mIucC9XmnZb", "IiekOA")
            .await
            .unwrap();
        info!("res: {:?}", res);
        assert_eq!(res["code"], 0);
    }

    #[tokio::test]
    async fn test_notify() {
        let _ = &*SETUP;
        notify("hello test").await;
        assert!(true);
    }
}
