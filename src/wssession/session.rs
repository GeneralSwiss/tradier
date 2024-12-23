use crate::constants::TRADIER_SESSION_TIMEOUT;
use crate::error::Result;
use crate::{config::Config, error::Error};
use chrono::{DateTime, Duration, Utc};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use tracing::debug;

use super::session_manager::SessionManager;

/// Represents a Tradier API session, handling WebSocket streaming configuration for either
/// account or market data.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct Session<'a> {
    /// The type of session, either `Account` or `Market`.
    pub session_type: SessionType,
    /// Contains information about the WebSocket stream, including URL and session ID.
    pub stream_info: StreamInfo,
    created_at: DateTime<Utc>,
    session_manager: &'a SessionManager,
}

/// Response structure for the Tradier API session request. Holds the stream information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResponse {
    /// Contains the WebSocket stream information such as URL and session ID.
    pub stream: StreamInfo,
}

/// Holds information about the WebSocket stream, including the URL and session ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    /// WebSocket URL for the stream.
    pub url: String,
    /// Unique identifier for the session, used to maintain connection context.
    #[serde(rename = "sessionid")]
    pub session_id: String,
}

/// Specifies the type of Tradier API session, either `Market` for market data
/// or `Account` for account-related data.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionType {
    Market,
    Account,
}

impl Display for SessionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionType::Market => write!(f, "Market"),
            SessionType::Account => write!(f, "Account"),
        }
    }
}

impl<'a> Session<'a> {
    /// Creates a new `Session` instance using the specified session type and configuration,
    /// managed by the provided `SessionManager`.
    ///
    /// This method handles the creation of a Tradier WebSocket session for either market or
    /// account events. It ensures that only one session is active at any given time by leveraging
    /// the `SessionManager` for singleton enforcement.
    ///
    /// # Parameters
    /// - **`session_manager`**: A reference to the `SessionManager` that manages session state and ensures
    ///   only one active session exists at a time.
    /// - **`session_type`**: Specifies the type of session to create:
    ///   - `SessionType::Market`: For streaming market data.
    ///   - `SessionType::Account`: For streaming account-related events.
    /// - **`config`**: A reference to the `Config` object, which provides necessary information like
    ///   the API base URL and access tokens.
    ///
    /// # Returns
    /// - **`Ok(Session)`**: A new `Session` instance if the session is created successfully.
    /// - **`Err(Error)`**: An error if session creation fails.
    ///
    /// # Behavior
    /// - Acquires a lock from the `SessionManager` to ensure a singleton session.
    /// - Constructs the appropriate URL based on the session type.
    /// - Sends a POST request to the Tradier API to initialize the session.
    /// - Parses the response to extract the session stream information.
    ///
    /// # Errors
    /// This method will return an error if:
    /// - **Singleton Restriction**: Another session is already active (`SessionManager` prevents new sessions).
    /// - **Missing Access Token**: The `access_token` field in the provided `Config` is `None`.
    /// - **Network or API Issues**: The HTTP request to create the session fails due to:
    ///   - Network errors.
    ///   - Server errors (non-2xx HTTP responses).
    /// - **Malformed Response**: The API returns an invalid or unexpected response format.
    ///
    /// # Example for developers
    /// ```ignore
    /// use tradier::config::Config;
    /// use tradier::wssession::session::{Session, SessionManager, SessionType};
    /// #[tokio::main]
    /// async fn main() {
    ///     let config = Config::new();
    ///     let session_manager = SessionManager::new();
    ///
    ///     match Session::new_with_session_manager(&session_manager, SessionType::Account, &config).await {
    ///         Ok(session) => {
    ///             println!("Session created successfully!");
    ///         }
    ///         Err(e) => {
    ///             eprintln!("Failed to create session: {:?}", e);
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// # Debugging Information
    /// The method logs various debug messages, including:
    /// - The URL used to request the session.
    /// - Response headers, status code, and body.
    ///
    /// This can help diagnose issues with API connectivity or authentication.
    ///
    /// # Note
    /// The session must be explicitly released by the `SessionManager` when no longer needed to allow
    /// creation of new sessions.
    pub(crate) async fn new_with_session_manager(
        session_manager: &'a SessionManager,
        session_type: SessionType,
        config: &Config,
    ) -> Result<Self> {
        match session_manager.acquire_session() {
            Ok(_) => {
                let client = HttpClient::new();
                let url = match session_type {
                    SessionType::Market => {
                        format!("{}/v1/markets/events/session", config.rest_api.base_url)
                    }
                    SessionType::Account => {
                        format!("{}/v1/accounts/events/session", config.rest_api.base_url)
                    }
                };
                debug!("Url to use to get the Session ID: {}", url);

                let access_token = config
                    .credentials
                    .access_token
                    .as_ref()
                    .ok_or(Error::MissingAccessToken)?;

                let response = client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", access_token))
                    .header("Accept", "application/json")
                    .header("Content-Length", "0")
                    .body("")
                    .send()
                    .await?;

                let status = response.status();
                let headers = response.headers().clone();
                debug!("Response status: {}", status);
                debug!("Response headers: {:?}", headers);

                let body = response.text().await?;
                debug!("Response body: {}", body);

                if status.is_success() {
                    let session_response: SessionResponse = serde_json::from_str(&body)?;
                    Ok(Session {
                        session_type,
                        stream_info: session_response.stream,
                        created_at: Utc::now(),
                        session_manager,
                    })
                } else {
                    session_manager.release_session();
                    Err(Error::CreateSessionError(session_type, status, body))
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Checks if the session has expired based on the configured session timeout.
    ///
    /// # Returns
    /// - `true` if the session duration exceeds `TRADIER_SESSION_TIMEOUT`, otherwise `false`.
    #[allow(dead_code)]
    pub fn is_expired(&self) -> bool {
        Utc::now() - self.created_at > Duration::minutes(TRADIER_SESSION_TIMEOUT)
    }

    /// Retrieves the WebSocket URL associated with the session.
    ///
    /// # Returns
    /// - `&str`: The WebSocket URL.
    pub fn get_websocket_url(&self) -> &str {
        &self.stream_info.url
    }

    /// Retrieves the session ID associated with the session.
    ///
    /// # Returns
    /// - `&str`: The session ID.
    pub fn get_session_id(&self) -> &str {
        &self.stream_info.session_id
    }
}

#[cfg(test)]
mod tests_session {
    use super::*;
    use crate::{
        config::{Credentials, RestApiConfig, StreamingConfig},
        utils::tests::create_test_config,
    };
    use mockito::Server;
    use pretty_assertions::{assert_eq, assert_matches};

    #[tokio::test]
    async fn test_account_session_creation() {
        let mut server = Server::new_async().await;
        let json_data = r#"
        {
            "stream": {
                "url": "wss://ws.tradier.com/v1/accounts/events",
                "sessionid": "c8638963-a6d4-4fb9-9bc6-e25fbd8c60c3"
            }
        }
        "#;
        let mock = server
            .mock("POST", "/v1/accounts/events/session")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json_data)
            .create_async()
            .await;

        let config = create_test_config().server_url(&server.url()).finish();
        let session_manager = SessionManager::default();
        {
            let session =
                Session::new_with_session_manager(&session_manager, SessionType::Account, &config)
                    .await
                    .unwrap();

            assert_eq!(session.session_type, SessionType::Account);
            assert_eq!(
                session.get_websocket_url(),
                "wss://ws.tradier.com/v1/accounts/events"
            );
            assert_eq!(
                session.get_session_id(),
                "c8638963-a6d4-4fb9-9bc6-e25fbd8c60c3"
            );
        }
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_market_session_creation() {
        let mut server = Server::new_async().await;
        let json_data = r#"
        {
            "stream": {
                "url": "https://stream.tradier.com/v1/markets/events",
                "sessionid": "c8638963-a6d4-4fb9-9bc6-e25fbd8c60c3"
            }
        }
        "#;
        let mock = server
            .mock("POST", "/v1/markets/events/session")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json_data)
            .create_async()
            .await;

        let config = create_test_config().server_url(&server.url()).finish();
        let session_manager = SessionManager::default();
        let session =
            Session::new_with_session_manager(&session_manager, SessionType::Market, &config)
                .await
                .unwrap();

        assert_eq!(session.session_type, SessionType::Market);
        assert_eq!(
            session.get_websocket_url(),
            "https://stream.tradier.com/v1/markets/events"
        );
        assert_eq!(
            session.get_session_id(),
            "c8638963-a6d4-4fb9-9bc6-e25fbd8c60c3"
        );

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_multiple_session_creation() {
        let mut server = Server::new_async().await;
        let json_data = r#"
        {
            "stream": {
                "url": "https://stream.tradier.com/v1/markets/events",
                "sessionid": "test_session_id"
            }
        }
        "#;
        let mock = server
            .mock("POST", "/v1/markets/events/session")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json_data)
            .expect(1)
            .create_async()
            .await;

        let config = create_test_config().server_url(&server.url()).finish();
        let session_manager = SessionManager::default();

        // Create first session
        let session1 =
            Session::new_with_session_manager(&session_manager, SessionType::Market, &config)
                .await
                .unwrap();

        // Attempt to create second session immediately (should fail)
        let session2 =
            Session::new_with_session_manager(&session_manager, SessionType::Market, &config).await;
        assert!(
            session2.is_err(),
            "Should not be able to create a second session"
        );
        assert!(session2
            .unwrap_err()
            .to_string()
            .contains("Session already exists"));

        // Drop the first session to reset the state
        drop(session1);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_content_length_header() {
        let mut server = Server::new_async().await;
        let json_data = r#"
        {
            "stream": {
                "url": "https://stream.tradier.com/v1/markets/events",
                "sessionid": "test_session_id"
            }
        }
        "#;
        let mock = server
            .mock("POST", "/v1/markets/events/session")
            .match_header("Content-Length", "0")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json_data)
            .create_async()
            .await;

        let config = create_test_config().server_url(&server.url()).finish();
        let session_manager = SessionManager::default();
        let session =
            Session::new_with_session_manager(&session_manager, SessionType::Market, &config)
                .await
                .unwrap();

        assert_eq!(session.session_type, SessionType::Market);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_missing_access_token_error() {
        let server = Server::new_async().await;
        let config = Config {
            credentials: Credentials {
                client_id: "test_id".to_string(),
                client_secret: "test_secret".to_string(),
                access_token: None, // Missing access token
                refresh_token: None,
            },
            rest_api: RestApiConfig {
                base_url: server.url().to_string(),
                timeout: 30,
            },
            streaming: StreamingConfig {
                http_base_url: "".to_string(),
                ws_base_url: "".to_string(),
                events_path: "".to_string(),
                reconnect_interval: 5,
            },
        };

        let session_manager = SessionManager::default();
        let session_result =
            Session::new_with_session_manager(&session_manager, SessionType::Market, &config).await;
        assert!(session_result.is_err());
        assert_matches!(session_result.unwrap_err(), Error::MissingAccessToken);
    }

    #[tokio::test]
    async fn test_api_request_failure_error() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/markets/events/session")
            .with_status(500) // Simulate server error
            .with_header("content-type", "application/json")
            .with_body("Internal Server Error")
            .create_async()
            .await;

        let config = create_test_config().server_url(&server.url()).finish();
        let session_manager = SessionManager::default();
        let session_result =
            Session::new_with_session_manager(&session_manager, SessionType::Market, &config).await;
        assert!(session_result.is_err());
        if let Error::CreateSessionError(_, status, body) = session_result.unwrap_err() {
            assert_eq!(status.as_u16(), 500);
            assert_eq!(body, "Internal Server Error");
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_invalid_json_response_error() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/markets/events/session")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("{ invalid json") // Malformed JSON
            .create_async()
            .await;

        let config = create_test_config().server_url(&server.url()).finish();
        let session_manager = SessionManager::default();

        let session_result =
            Session::new_with_session_manager(&session_manager, SessionType::Market, &config).await;
        assert!(session_result.is_err());
        assert_matches!(session_result.unwrap_err(), Error::JsonParsingError(_));

        mock.assert_async().await;
    }
}
