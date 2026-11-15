use std::time::Duration;

#[cfg(feature = "fetch")]
use reqwest::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum MarketError {
    #[cfg(feature = "fetch")]
    #[error("Warframe.market {0}: {1}")]
    Http(StatusCode, String),
    #[error("Rate limited, next request in {} s", .0.as_secs())]
    RateLimited(Duration),
    #[error("Unauthorized")]
    Unauthorized,
    #[error(transparent)]
    Decode(#[from] serde_json::Error),
    #[cfg(feature = "fetch")]
    #[error(transparent)]
    Transport(#[from] reqwest::Error),
    #[cfg(feature = "fetch")]
    #[error("Websocket: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("Login response without a JWT authorization header")]
    MissingJwt,
}

#[cfg(feature = "fetch")]
impl MarketError {
    pub fn brief(&self) -> String {
        match self {
            Self::Http(status, body) => format!(
                "Warframe.market {status}, {} {}",
                body.len(),
                if body.len() == 1 { "byte" } else { "bytes" }
            ),
            error => error.to_string(),
        }
    }
}

pub type Result<T> = std::result::Result<T, MarketError>;

#[cfg(feature = "fetch")]
const ORDER_REJECTIONS: [(&str, &str); 5] = [
    (
        "app.order.error.exceededOrderLimitSameItem",
        "An order for this item already exists",
    ),
    (
        "app.order.error.exceededOrderLimitSamePrice",
        "An order for this item already exists",
    ),
    (
        "app.order.error.exceededOrderLimit",
        "The account has reached its order limit",
    ),
    ("app.form.field_required", "A required field was left empty"),
    ("app.form.invalid", "A value in the order was rejected"),
];

#[cfg(feature = "fetch")]
pub fn order_rejection(error: &MarketError) -> Option<&'static str> {
    let MarketError::Http(_, body) = error else {
        return None;
    };
    ORDER_REJECTIONS
        .iter()
        .find(|(code, _)| body.contains(code))
        .map(|(_, cause)| *cause)
}

#[cfg(all(test, feature = "fetch"))]
mod tests {
    use super::*;

    #[test]
    fn order_rejection_reads_the_error_code() {
        let rejected = |body: &str| MarketError::Http(StatusCode::BAD_REQUEST, body.to_owned());
        assert_eq!(
            order_rejection(&rejected(
                r#"{"error":{"itemId":["app.order.error.exceededOrderLimitSameItem"]}}"#
            )),
            Some("An order for this item already exists")
        );
        assert_eq!(
            order_rejection(&rejected(
                r#"{"error":"app.order.error.exceededOrderLimit"}"#
            )),
            Some("The account has reached its order limit")
        );
        assert_eq!(
            order_rejection(&rejected(r#"{"error":"app.errors.banned"}"#)),
            None
        );
        assert_eq!(order_rejection(&MarketError::Unauthorized), None);
    }

    #[test]
    fn brief_error() {
        let error = MarketError::Http(
            StatusCode::BAD_GATEWAY,
            "<html>upstream is down</html>".to_owned(),
        );
        assert_eq!(error.brief(), "Warframe.market 502 Bad Gateway, 29 bytes");
        assert_eq!(
            MarketError::Http(StatusCode::BAD_GATEWAY, "?".to_owned()).brief(),
            "Warframe.market 502 Bad Gateway, 1 byte"
        );
        assert_eq!(
            MarketError::Http(StatusCode::BAD_GATEWAY, String::new()).brief(),
            "Warframe.market 502 Bad Gateway, 0 bytes"
        );
        assert_eq!(
            error.to_string(),
            "Warframe.market 502 Bad Gateway: <html>upstream is down</html>"
        );
        assert_eq!(MarketError::Unauthorized.brief(), "Unauthorized");
        assert_eq!(
            MarketError::RateLimited(Duration::from_secs(30)).brief(),
            "Rate limited, next request in 30 s"
        );
    }
}
