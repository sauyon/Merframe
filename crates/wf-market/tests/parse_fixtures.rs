use wf_market::{
    Item, Order, OrderType, Polarity, PriceTable, User, UserPrivate, UserStatus, envelope,
    parse_chats, parse_v1_auction, parse_v1_auctions,
};

#[allow(
    clippy::unwrap_used,
    reason = "a test helper outside a #[test] function"
)]
fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn items_list() {
    let json = fixture("items.json");
    let items = envelope::<Vec<Item>>(&json).unwrap();
    assert_eq!(items.len(), 23);
    let secura = items
        .iter()
        .find(|item| item.slug == "secura_dual_cestra")
        .unwrap();
    assert_eq!(
        secura.game_ref,
        "/Lotus/Weapons/Syndicates/PerrinSequence/Pistols/PSDualCestra"
    );
    let localization = secura.i18n.get("en").unwrap();
    assert_eq!(localization.name, "Secura Dual Cestra");
    let arcane = items
        .iter()
        .find(|item| item.slug == "arcane_barrier")
        .unwrap();
    assert_eq!(arcane.bulk_tradable, Some(true));
    assert_eq!(arcane.max_rank, Some(5));
    assert_eq!(secura.bulk_tradable, None);
    assert_eq!(secura.subtypes, None);
    assert_eq!(secura.max_amber_stars, None);
    assert_eq!(secura.max_cyan_stars, None);
    let relic = items
        .iter()
        .find(|item| item.slug == "meso_c1_relic")
        .unwrap();
    assert_eq!(
        relic.subtypes,
        Some(
            ["intact", "exceptional", "flawless", "radiant"]
                .map(String::from)
                .to_vec()
        )
    );
    let fish = items.iter().find(|item| item.slug == "cuthol").unwrap();
    assert_eq!(
        fish.subtypes,
        Some(["small", "medium", "large"].map(String::from).to_vec())
    );
    let sculpture = items
        .iter()
        .find(|item| item.slug == "ayatan_orta_sculpture")
        .unwrap();
    assert_eq!(sculpture.max_amber_stars, Some(2));
    assert_eq!(sculpture.max_cyan_stars, Some(1));
    assert_eq!(sculpture.subtypes, None);
}

#[test]
fn single_item_envelope() {
    let json = fixture("items.json");
    let items = envelope::<Vec<Item>>(&json).unwrap();
    let raw = serde_json::to_string(&serde_json::json!({
        "apiVersion": "0.25.0",
        "data": items[0],
        "error": null,
    }))
    .unwrap();
    let item = envelope::<Item>(&raw).unwrap();
    assert_eq!(item.id, items[0].id);
}

#[test]
fn orders_for_item() {
    let json = fixture("orders_item.json");
    let orders = envelope::<Vec<Order>>(&json).unwrap();
    assert_eq!(orders.len(), 12);
    let first = &orders[0];
    assert_eq!(first.item_id, "54aae292e7798909064f1575");
    assert!(
        matches!(first.order_type, OrderType::Sell) || matches!(first.order_type, OrderType::Buy)
    );
    let user = first.user.as_ref().unwrap();
    assert_eq!(user.locale, "en");
}

#[test]
fn own_orders_without_user() {
    let orders = envelope::<Vec<Order>>(
        r#"{"apiVersion":"0.25.0","data":[{"id":"6a99fc2d90780835bc710a10","type":"sell","platinum":3,"quantity":1,"visible":true,"createdAt":"2026-09-03T23:01:01Z","updatedAt":"2026-09-08T12:22:59Z","itemId":"54a73e65e779893a797fff32"}],"error":null}"#,
    )
    .unwrap();
    assert_eq!(orders.len(), 1);
    assert!(orders[0].user.is_none());
}

#[test]
fn my_orders() {
    let json = fixture("orders_my.json");
    let orders = envelope::<Vec<Order>>(&json).unwrap();
    assert_eq!(orders.len(), 2);
    assert_eq!(
        orders[0].user.as_ref().map(|user| user.slug.as_str()),
        Some("merframetester")
    );
}

#[test]
fn me() {
    let json = fixture("me.json");
    let me = envelope::<UserPrivate>(&json).unwrap();
    assert_eq!(me.slug, "merframetester");
    assert_eq!(me.ingame_name, "MerframeTester");
    assert_eq!(me.tier, "none");
    assert_eq!(me.mastery_rank, 16);
    assert!(me.verification);
}

#[test]
fn sparse_me() {
    let me = envelope::<UserPrivate>(
        r#"{"apiVersion":"0.25.0","data":{"id":"6a5daf0f000000000000d001","tier":"none","ingameName":"Sparse","slug":"sparse","masteryRank":12,"verification":true},"error":null}"#,
    )
    .unwrap();
    assert_eq!(me.ingame_name, "Sparse");
    assert_eq!(me.mastery_rank, 12);
    assert!(me.verification);
}

#[test]
fn unverified_me() {
    let me = envelope::<UserPrivate>(
        r#"{"apiVersion":"0.25.0","data":{"id":"6a5daf0f000000000000d002","tier":"none","ingameName":"Unverified","slug":"unverified","masteryRank":2,"verification":false},"error":null}"#,
    )
    .unwrap();
    assert!(!me.verification);
}

#[test]
fn public_user() {
    let json = fixture("user.json");
    let user = envelope::<User>(&json).unwrap();
    assert_eq!(user.slug, "testtenno");
    assert_eq!(user.status, Some(UserStatus::Offline));
}

#[test]
fn v1_auction() {
    let json = fixture("auction.json");
    let auction = parse_v1_auction(&json).unwrap();
    assert_eq!(auction.item.polarity, Polarity::Naramon);
    assert_eq!(auction.item.weapon_url_name, "okina");
}

#[test]
fn my_auctions() {
    let json = fixture("auctions_my.json");
    let auctions = parse_v1_auctions(&json).unwrap();
    assert_eq!(auctions.len(), 2);

    let direct = &auctions[0];
    assert_eq!(direct.id, "6a9f08ab51f7f20eeeb6add2");
    assert!(direct.is_direct_sell);
    assert_eq!(direct.buyout_price, Some(500));
    assert_eq!(direct.starting_price, 500);
    assert!(direct.visible);
    assert!(!direct.private);
    assert_eq!(direct.item.weapon_url_name, "okina");
    assert_eq!(direct.item.name, "acri-vexicak");
    assert_eq!(direct.item.attributes.len(), 4);

    let bidding = &auctions[1];
    assert!(!bidding.is_direct_sell);
    assert_eq!(bidding.buyout_price, None);
    assert_eq!(bidding.starting_price, 120);
    assert!(!bidding.visible);
    assert!(bidding.private);
    assert_eq!(bidding.item.weapon_url_name, "kuva_bramma");
    assert_eq!(bidding.created, "2026-08-30T09:14:02.000+00:00");
    assert_eq!(bidding.updated, "2026-09-06T21:02:44.000+00:00");
}

#[test]
fn malformed_riven_auction_fails() {
    let json =
        r#"{"payload":{"auctions":[{"id":"6a9f08ab51f7f20eeeb6add2","item":{"type":"riven"}}]}}"#;
    let error = parse_v1_auctions(json).expect_err("decode error");
    assert!(matches!(error, wf_market::MarketError::Decode(_)));
}

#[test]
fn empty_auction_list() {
    let auctions = parse_v1_auctions(r#"{"payload":{"auctions":[]}}"#).unwrap();
    assert!(auctions.is_empty());
}

#[test]
fn malformed_envelope() {
    let error = envelope::<Order>("not json").expect_err("decode error");
    assert!(matches!(error, wf_market::MarketError::Decode(_)));
}

#[test]
fn bulk_price_table() {
    let json = fixture("prices_bulk.json");
    let table = serde_json::from_str::<PriceTable>(&json).unwrap();
    assert_eq!(table.updated_at, 1_757_410_800);
    assert_eq!(table.count, 10);
    assert_eq!(table.items.len(), table.count);

    let set = table.items["braton_prime_set"];
    assert_eq!(set.sell_r0, Some(45));
    assert_eq!(set.sell_max, None);
    assert_eq!(set.buy_r0, Some(30));

    let arcane = table.items["arcane_energize"];
    assert_eq!(arcane.sell_r0, Some(55));
    assert_eq!(arcane.sell_max, Some(940));
    assert_eq!(arcane.buy_r0, Some(40));

    let unsold = table.items["vasca_kavat_imprint"];
    assert_eq!(unsold.sell_r0, None);
    assert_eq!(unsold.buy_r0, None);
}

#[test]
fn price_entry_volume_shapes() {
    let json = fixture("prices_bulk.json");
    let table = serde_json::from_str::<PriceTable>(&json).unwrap();
    let statistics = table.items["primed_flow"];
    assert_eq!(statistics.sell_r0, Some(20));
    assert_eq!(statistics.sell_max, Some(210));
    assert_eq!(statistics.buy_r0, Some(12));

    let orders = table.items["primed_continuity"];
    assert_eq!(orders.sell_r0, Some(12));
    assert_eq!(orders.sell_max, Some(165));
    assert_eq!(orders.buy_r0, Some(6));
}

#[test]
fn empty_price_entry() {
    let table = serde_json::from_str::<PriceTable>(
        r#"{"updated_at":1757410800,"count":1,"items":{"forma_bp":{}}}"#,
    )
    .unwrap();
    let entry = table.items["forma_bp"];
    assert_eq!(entry.sell_r0, None);
    assert_eq!(entry.sell_max, None);
    assert_eq!(entry.buy_r0, None);
}

#[test]
fn chat_list_unread() {
    let json = fixture("chats.json");
    let chats = parse_chats(&json).unwrap();
    assert_eq!(chats.len(), 2);
    let unread: u32 = chats.iter().map(|chat| chat.unread_count).sum();
    assert_eq!(unread, 2);
    assert_eq!(chats[0].unread_count, 2);
    assert_eq!(chats[1].unread_count, 0);
}
