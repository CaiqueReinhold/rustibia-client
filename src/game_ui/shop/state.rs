use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::items::ItemId;
use crate::network::events::{ShopClosed, ShopReceived};

pub const MAX_TRADE_AMOUNT: u16 = 100;

/// One offer's identity: the same container sold empty and sold full are two wares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ware {
    pub item_id: ItemId,
    pub subtype: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShopOffer {
    pub item_id: ItemId,
    pub subtype: u8,
    pub weight: u32,
    pub buy: u32,
    pub sell: u32,
}

impl ShopOffer {
    pub fn ware(&self) -> Ware {
        Ware {
            item_id: self.item_id,
            subtype: self.subtype,
        }
    }

    pub fn price(&self, tab: ShopTab) -> u32 {
        match tab {
            ShopTab::Buy => self.buy,
            ShopTab::Sell => self.sell,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShopTab {
    #[default]
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Shop {
    pub npc_name: String,
    pub money: u64,
    pub offers: Vec<ShopOffer>,
    pub goods: HashMap<Ware, u16>,
}

#[derive(Resource, Default)]
pub struct ShopState(pub Option<Shop>);

#[derive(Debug, Clone, PartialEq)]
pub struct OfferRow {
    pub ware: Ware,
    pub name: String,
    pub price: u32,
    pub weight: u32,
    /// How many the player could trade now; 0 dims the box.
    pub max: u16,
}

pub fn rows(
    shop: &Shop,
    tab: ShopTab,
    search: &str,
    free_capacity: u32,
    name_of: impl Fn(Ware) -> String,
) -> Vec<OfferRow> {
    let search = search.trim().to_lowercase();
    shop.offers
        .iter()
        .filter(|offer| offer.price(tab) > 0)
        .map(|offer| OfferRow {
            ware: offer.ware(),
            name: name_of(offer.ware()),
            price: offer.price(tab),
            weight: offer.weight,
            max: max_amount(shop, tab, offer, free_capacity),
        })
        .filter(|row| row.name.to_lowercase().contains(&search))
        .collect()
}

fn max_amount(shop: &Shop, tab: ShopTab, offer: &ShopOffer, free_capacity: u32) -> u16 {
    let limit = match tab {
        ShopTab::Buy => {
            let by_money = shop.money / offer.buy.max(1) as u64;
            let by_capacity = match offer.weight {
                0 => u64::MAX,
                weight => (free_capacity / weight) as u64,
            };
            by_money.min(by_capacity)
        }
        ShopTab::Sell => shop.goods.get(&offer.ware()).copied().unwrap_or(0) as u64,
    };
    limit.min(MAX_TRADE_AMOUNT as u64) as u16
}

pub fn kept_selection(rows: &[OfferRow], selected: Option<Ware>) -> Option<Ware> {
    selected.filter(|ware| rows.iter().any(|row| row.ware == *ware && row.max > 0))
}

/// `weight` is in hundredths of an ounce, the server's unit.
pub fn ounces(weight: u32) -> String {
    format!("{}.{:02} oz", weight / 100, weight % 100)
}

pub fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

pub fn on_shop_received(event: On<ShopReceived>, mut state: ResMut<ShopState>) {
    state.0 = Some(Shop {
        npc_name: event.npc_name.clone(),
        money: event.money,
        offers: event.offers.clone(),
        goods: event
            .goods
            .iter()
            .map(|&(item_id, subtype, amount)| (Ware { item_id, subtype }, amount))
            .collect(),
    });
}

pub fn on_shop_closed(_event: On<ShopClosed>, mut state: ResMut<ShopState>) {
    state.0 = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROPE: Ware = Ware {
        item_id: ItemId(3003),
        subtype: 0,
    };
    const FLASK: Ware = Ware {
        item_id: ItemId(283),
        subtype: 0,
    };

    fn a_shop(money: u64, goods: &[(Ware, u16)]) -> Shop {
        Shop {
            npc_name: "Xodet".to_string(),
            money,
            offers: vec![
                ShopOffer {
                    item_id: ROPE.item_id,
                    subtype: 0,
                    weight: 1800,
                    buy: 50,
                    sell: 0,
                },
                ShopOffer {
                    item_id: FLASK.item_id,
                    subtype: 0,
                    weight: 0,
                    buy: 0,
                    sell: 5,
                },
            ],
            goods: goods.iter().copied().collect(),
        }
    }

    fn name_of(ware: Ware) -> String {
        if ware == ROPE {
            "rope"
        } else {
            "empty potion flask"
        }
        .to_string()
    }

    fn ids(rows: &[OfferRow]) -> Vec<Ware> {
        rows.iter().map(|row| row.ware).collect()
    }

    #[test]
    fn each_tab_lists_the_offers_priced_in_its_direction() {
        let shop = a_shop(1000, &[]);

        assert_eq!(
            ids(&rows(&shop, ShopTab::Buy, "", 100_000, name_of)),
            [ROPE]
        );
        assert_eq!(
            ids(&rows(&shop, ShopTab::Sell, "", 100_000, name_of)),
            [FLASK]
        );
    }

    #[test]
    fn buying_is_capped_by_money_capacity_and_a_hundred() {
        let rope =
            |money, capacity| rows(&a_shop(money, &[]), ShopTab::Buy, "", capacity, name_of)[0].max;

        assert_eq!(rope(149, 100_000), 2);
        assert_eq!(rope(10_000, 5_400), 3);
        assert_eq!(rope(1_000_000, 10_000_000), 100);
        assert_eq!(rope(49, 100_000), 0);
    }

    #[test]
    fn selling_is_capped_by_what_is_carried_and_an_uncarried_offer_stays_listed() {
        let carried = rows(&a_shop(0, &[(FLASK, 140)]), ShopTab::Sell, "", 0, name_of);
        let none = rows(&a_shop(0, &[]), ShopTab::Sell, "", 0, name_of);

        assert_eq!(carried[0].max, 100);
        assert_eq!((ids(&none), none[0].max), (vec![FLASK], 0));
    }

    #[test]
    fn the_search_matches_any_part_of_the_name_in_any_case() {
        let shop = a_shop(1000, &[]);

        assert_eq!(
            ids(&rows(&shop, ShopTab::Buy, " RoP", 100_000, name_of)),
            [ROPE]
        );
        assert!(rows(&shop, ShopTab::Buy, "flask", 100_000, name_of).is_empty());
    }

    #[test]
    fn a_selection_survives_only_while_its_offer_can_be_traded() {
        let tradeable = rows(&a_shop(1000, &[]), ShopTab::Buy, "", 100_000, name_of);
        let broke = rows(&a_shop(0, &[]), ShopTab::Buy, "", 100_000, name_of);

        assert_eq!(kept_selection(&tradeable, Some(ROPE)), Some(ROPE));
        assert_eq!(kept_selection(&broke, Some(ROPE)), None);
        assert_eq!(kept_selection(&tradeable, Some(FLASK)), None);
    }

    #[test]
    fn weight_reads_in_ounces_with_two_decimals() {
        assert_eq!(ounces(1800), "18.00 oz");
        assert_eq!(ounces(5), "0.05 oz");
    }

    #[test]
    fn large_numbers_are_grouped_in_thousands() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(9_300), "9,300");
        assert_eq!(grouped(515_086), "515,086");
        assert_eq!(grouped(1_250_000), "1,250,000");
    }

    #[test]
    fn the_same_container_is_listed_and_counted_once_per_fluid() {
        let empty = Ware {
            item_id: ItemId(2874),
            subtype: 0,
        };
        let milk = Ware {
            item_id: ItemId(2874),
            subtype: 9,
        };
        let offer = |ware: Ware| ShopOffer {
            item_id: ware.item_id,
            subtype: ware.subtype,
            weight: 180,
            buy: 0,
            sell: 5,
        };
        let shop = Shop {
            npc_name: "Xodet".to_string(),
            money: 0,
            offers: vec![offer(empty), offer(milk)],
            goods: [(empty, 4)].into_iter().collect(),
        };

        let rows = rows(&shop, ShopTab::Sell, "", 0, |ware| format!("{ware:?}"));

        assert_eq!(
            rows.iter()
                .map(|row| (row.ware, row.max))
                .collect::<Vec<_>>(),
            vec![(empty, 4), (milk, 0)]
        );
    }
}
