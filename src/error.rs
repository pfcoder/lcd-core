/// General error define
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MinerError {
    #[error("Miner not support")]
    MinerNotSupportError,

    #[error("HTTP Error")]
    HttpError,

    #[error("Auth error")]
    AuthError,

    #[error("Read Avalon Config Error")]
    ReadAvalonConfigError,

    #[error("Feishu Parser JSON Error")]
    FeishuParserJsonError,

    #[error("Read Time Config Error")]
    ReadTimeConfigError,

    #[error("TCP Read Error")]
    TcpReadError,

    #[error("Ping Error")]
    PingFiledError,

    #[error("Poolin Api Regex Error")]
    PoolinApiRegexError,

    #[error("Poolin Api Request Error")]
    PoolinApiRequestError,

    #[error("Pool Type Not Detected")]
    PoolTypeNotDetected,

    #[error(transparent)]
    SQLiteError(#[from] rusqlite::Error),

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    #[error(transparent)]
    ToStrError(#[from] reqwest::header::ToStrError),

    #[error(transparent)]
    UriError(#[from] http::uri::InvalidUri),

    #[error(transparent)]
    JsonParseError(#[from] serde_json::Error),

    #[error(transparent)]
    CurlError(#[from] curl::Error),

    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),

    #[error(transparent)]
    SerdeUrlEncodedError(#[from] serde_urlencoded::ser::Error),

    #[error(transparent)]
    TimeParserError(#[from] chrono::ParseError),

    #[error(transparent)]
    JoinError(#[from] tokio::task::JoinError),

    #[error(transparent)]
    StdIoError(#[from] std::io::Error),
}
