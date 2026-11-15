use crate::error::Result;
use crate::models::{ApiEnvelope, Auction, Chat, V1Payload};

pub fn envelope<T>(json: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let envelope: ApiEnvelope<T> = serde_json::from_str(json)?;
    Ok(envelope.data)
}

pub fn v1_payload<T>(json: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let payload: V1Payload<T> = serde_json::from_str(json)?;
    Ok(payload.payload)
}

#[derive(Debug, serde::Deserialize)]
struct V1AuctionPayload {
    auction: Auction,
}

#[derive(Debug, serde::Deserialize)]
struct V1AuctionsPayload {
    auctions: Vec<serde_json::Value>,
}

pub fn parse_v1_auction(json: &str) -> Result<Auction> {
    Ok(v1_payload::<V1AuctionPayload>(json)?.auction)
}

pub fn parse_v1_auctions(json: &str) -> Result<Vec<Auction>> {
    v1_payload::<V1AuctionsPayload>(json)?
        .auctions
        .into_iter()
        .filter(|auction| auction["item"]["type"] == "riven")
        .map(|auction| Ok(serde_json::from_value(auction)?))
        .collect()
}

#[derive(Debug, serde::Deserialize)]
struct V1ChatsPayload {
    chats: Vec<Chat>,
}

pub fn parse_chats(json: &str) -> Result<Vec<Chat>> {
    Ok(v1_payload::<V1ChatsPayload>(json)?.chats)
}
