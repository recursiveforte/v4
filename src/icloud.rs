use std::error::Error;
use std::sync::Arc;
use reqwest::cookie::{Jar as CookieJar};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const SIGN_IN_ENDPOINT: &str = "https://idmsa.apple.com/appleauth/auth/signin";
const SETUP_ENDPOINT: &str = "https://setup.icloud.com/setup/ws/1/accountLogin";

pub trait ICloudSessionOps {
    async fn new(username: String, password: String) -> Result<ICloudSession, Box<dyn Error>>;
    async fn get_locations(&mut self) -> Result<Vec<FindMyDevice>, Box<dyn Error>>;
}

#[derive(Debug)]
pub struct ICloudSession {
    account_country_code: Option<String>,
    session_token: Option<String>,
    client: reqwest::Client,
    web_service_urls: WebServiceUrls,
    username: String,
    password: String
}

#[derive(Debug)]
struct WebServiceUrls {
    findme: Option<String>
}

impl WebServiceUrls {
    pub fn new() -> WebServiceUrls {
        return WebServiceUrls {
            findme: None
        }
    }
}

impl ICloudSessionOps for ICloudSession {
    async fn new(username: String, password: String) -> Result<ICloudSession, Box<dyn Error>> {
        let cookies = Arc::from(CookieJar::default());

        let mut state = ICloudSession {
            account_country_code: None,
            session_token: None,
            client: reqwest::ClientBuilder::new().cookie_provider(cookies.clone()).build()?,
            web_service_urls: WebServiceUrls::new(),
            username, password
        };

        state.auth_step1().await?;
        state.auth_step2().await?;

        Ok(state)
    }


    async fn get_locations(&mut self) -> Result<Vec<FindMyDevice>, Box<dyn Error>> {
        #[derive(Serialize, Deserialize)]
        struct RefreshClientBodyParams {
            #[serde(rename = "clientContext")]
            client_context: RefreshClientContext
        }

        #[derive(Serialize, Deserialize)]
        struct RefreshClientContext {
            fmly: bool,
            #[serde(rename = "shouldLocate")]
            should_locate: bool,
            #[serde(rename = "selectedDevice")]
            selected_device: String,
            #[serde(rename = "deviceListVersion")]
            device_list_version: u32
        }

        let post = | session: &ICloudSession | -> Result<_, Box<dyn Error>> {
            Ok(session.client
                .post(
                    session.web_service_urls.findme.to_owned().ok_or("need findme url!")?
                        + "/fmipservice/client/web/refreshClient")
                .header("Content-Type", "application/json")
                .header("Accept", "*/*")
                .header("Origin", "https://www.icloud.com")
                .json(&RefreshClientBodyParams {
                    client_context: RefreshClientContext {
                        fmly: false,
                        should_locate: true,
                        selected_device: "all".to_string(),
                        device_list_version: 1
                    }
                })
                .send())
        };

        let mut res = post(self)?.await?;

        if res.status() == 450 || res.status() == 421 {
            if res.status() == 450 {
                self.auth_step1().await?;
            }

            self.auth_step2().await?;
            
            res = post(self)?.await?;
        }

        #[derive(Serialize, Deserialize, Debug)]
        struct FindMyResponse {
            content: Vec<FindMyDevice>
        }
        let data: FindMyResponse = res.json().await?;

        Ok(data.content)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FindMyDevice {
    pub name: String,
    pub id: String,
    pub location: Option<FindMyDeviceLocation>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FindMyDeviceLocation {
    pub latitude: f64,
    pub longitude: f64,
    #[serde(rename = "timeStamp")]
    pub timestamp: i64
}

impl ICloudSession {

    async fn auth_step1(&mut self) -> Result<(), Box<dyn Error>> {
        #[derive(Serialize, Deserialize)]
        struct LoginBodyParams {
            #[serde(rename = "rememberMe")]
            remember_me: bool,
            #[serde(rename = "accountName")]
            account_name: String,
            password: String,
        }

        #[derive(Serialize, Deserialize)]
        struct LoginQueryParams {
            #[serde(rename = "isRememberMeEnabled")]
            is_remember_me_enabled: bool 
        }

        let res = self.client.post(SIGN_IN_ENDPOINT)
            .header("Content-Type", "application/json")
            .header("Accept", "*/*")
            .header("X-Apple-OAuth-Client-Id", "d39ba9916b7251055b22c7f910e2ea796ee65e98b2ddecea8f5dde8d9d1a815d")
            .header("X-Apple-OAuth-Client-Type", "firstPartyAuth")
            .header("X-Apple-OAuth-Redirect-URI", "https://www.icloud.com")
            .header("X-Apple-OAuth-Require-Grant-Code", "true")
            .header("X-Apple-OAuth-Response-Mode", "web_message")
            .header("X-Apple-OAuth-Response-Type", "code")
            .header("X-Apple-OAuth-State", "auth-cb0b80f8-7134-11ef-bfeb-fae44b21d45c")
            .header("X-Apple-Widget-Key", "d39ba9916b7251055b22c7f910e2ea796ee65e98b2ddecea8f5dde8d9d1a815d")
            .json(&LoginBodyParams {
                remember_me: true,
                account_name: self.username.clone(),
                password: self.password.clone(),
            })
            .query(&LoginQueryParams { is_remember_me_enabled: true })
            .send().await?;

        if let Some(header) = res.headers().get("X-Apple-I-Rscd") {
            if header.to_str()? != "409" {
                return Err("Authentication failed.".into())
            }
        }

        self.session_token =
            Some(res.headers().get("X-Apple-Session-Token")
                .ok_or("Missing session token!")?.to_str()?.to_string());

        self.account_country_code =
            Some(res.headers().get("X-Apple-ID-Account-Country")
                .ok_or("Missing account country!")?.to_str()?.to_string());

        Ok(())
    }


    async fn auth_step2(&mut self) -> Result<(), Box<dyn Error>>{
        #[derive(Serialize, Deserialize)]
        struct SetupBodyParams {
            #[serde(rename = "accountCountryCode")]
            account_country_code: String,
            #[serde(rename = "dsWebAuthToken")]
            ds_web_auth_token: String,
            extended_login: bool,
        }

        let res = self.client.post(SETUP_ENDPOINT)
            .header("Content-Type", "application/json")
            .header("Accept", "*/*")
            .header("Origin", "https://www.icloud.com")
            .json(&SetupBodyParams {
                account_country_code: self.account_country_code.clone().ok_or("missing country code!")?,
                ds_web_auth_token: self.session_token.clone().ok_or("missing session token!")?,
                extended_login: true
            })
            .send().await?;

        let data: Value = res.json().await?;

        if let Some(url) = data.get("webservices")
            .and_then(|data| data.get("findme"))
            .and_then(|data| data.get("url"))
            .and_then(|data| data.as_str()) {
            self.web_service_urls.findme = Some(url.to_string())
        }

        Ok(())
    }
}
