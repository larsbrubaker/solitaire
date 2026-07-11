use super::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

struct StorageGuard {
    _store: Rc<RefCell<HashMap<String, String>>>,
}

impl Drop for StorageGuard {
    fn drop(&mut self) {
        crate::platform::clear_storage_io_for_test();
    }
}

fn install_test_storage() -> StorageGuard {
    let store = Rc::new(RefCell::new(HashMap::new()));
    let load_store = store.clone();
    let save_store = store.clone();
    crate::platform::set_storage_io(
        move |k| load_store.borrow().get(k).cloned(),
        move |k, v| {
            save_store.borrow_mut().insert(k.to_string(), v.to_string());
        },
    );
    StorageGuard { _store: store }
}

#[test]
fn app_model_loads_persisted_spider_options() {
    let _guard = install_test_storage();
    UserSettings {
        klondike_draw_count: 3,
        spider_suit_count: 1,
        spider_one_suit: Suit::Spades,
        spider_winnable_only: false,
        freecell_winnable_only: false,
        klondike_winnable_only: false,
        perf_window: PerfWindowState::default(),
    }
    .save();

    let mut model = AppModel::new();
    assert_eq!(model.klondike_draw_count, 3);
    assert_eq!(model.spider_suit_count, 1);
    assert_eq!(model.spider_one_suit, Suit::Spades);

    model.start_game_with_seed(GameKind::Spider, 7);
    let session = model.session.as_ref().unwrap();
    for cid in 9..=18u8 {
        let top = session.piles().get(cid).top().unwrap();
        assert_eq!(top.suit, Suit::Spades);
    }
}

#[test]
fn perf_window_state_round_trips_through_settings() {
    let _guard = install_test_storage();

    // Seed the settings store with a non-default perf window
    // layout (open + offset + resized).
    UserSettings {
        klondike_draw_count: 1,
        spider_suit_count: 1,
        spider_one_suit: Suit::Spades,
        spider_winnable_only: false,
        freecell_winnable_only: false,
        klondike_winnable_only: false,
        perf_window: PerfWindowState {
            visible: true,
            x: 240.0,
            y: 180.0,
            width: 480.0,
            height: 240.0,
        },
    }
    .save();

    // First model load must surface the saved perf window.
    let model = AppModel::new();
    assert!(model.show_performance_window.get());
    assert_eq!(
        model.perf_window_bounds.get(),
        Rect::new(240.0, 180.0, 480.0, 240.0)
    );

    // Simulate the agg-gui Window writing fresh bounds back into
    // the position cell after the user dragged the window — the
    // model should detect the diff and persist on the next tick.
    model
        .perf_window_bounds
        .set(Rect::new(300.0, 220.0, 480.0, 240.0));
    model.maybe_save_perf_window_settings();
    let reloaded = UserSettings::load().perf_window;
    assert_eq!(reloaded.x, 300.0);
    assert_eq!(reloaded.y, 220.0);
    assert!(reloaded.visible);

    // Subsequent ticks with no further changes are no-ops (the
    // diff guard short-circuits before touching the storage
    // backend) — verify by re-reading the same blob.
    model.maybe_save_perf_window_settings();
    let again = UserSettings::load().perf_window;
    assert_eq!(again, reloaded);
}

#[test]
fn new_deal_request_on_fresh_game_skips_confirmation() {
    let _guard = install_test_storage();
    let mut model = AppModel::new();
    model.start_game_with_seed(GameKind::Spider, 123);
    let original_seed = model.session.as_ref().unwrap().seed();

    model.request_new_deal(GameKind::Spider);

    assert_eq!(model.confirm, None);
    assert_eq!(model.screen, Screen::Game);
    assert_ne!(model.session.as_ref().unwrap().seed(), original_seed);
}

#[test]
fn new_deal_request_after_move_requires_confirmation() {
    let _guard = install_test_storage();
    let mut model = AppModel::new();
    model.start_game_with_seed(GameKind::Spider, 123);
    let original_seed = model.session.as_ref().unwrap().seed();
    apply_spider_stock_deal(&mut model);

    model.request_new_deal(GameKind::Spider);

    assert_eq!(
        model.confirm,
        Some(ConfirmAction::NewDeal(GameKind::Spider))
    );
    assert_eq!(model.session.as_ref().unwrap().seed(), original_seed);

    model.confirm_pending_action();

    assert_eq!(model.confirm, None);
    assert_eq!(model.screen, Screen::Game);
    assert_ne!(model.session.as_ref().unwrap().seed(), original_seed);
}

#[test]
fn main_menu_request_on_fresh_game_skips_confirmation() {
    let _guard = install_test_storage();
    let mut model = AppModel::new();
    model.start_game_with_seed(GameKind::Spider, 456);

    model.request_main_menu();

    assert_eq!(model.confirm, None);
    assert_eq!(model.screen, Screen::Title);
    assert!(model.session.is_none());
}

#[test]
fn main_menu_request_after_move_requires_confirmation() {
    let _guard = install_test_storage();
    let mut model = AppModel::new();
    model.start_game_with_seed(GameKind::Spider, 456);
    apply_spider_stock_deal(&mut model);

    model.request_main_menu();

    assert_eq!(model.confirm, Some(ConfirmAction::MainMenu));
    assert_eq!(model.screen, Screen::Game);
    assert!(model.session.is_some());

    model.cancel_pending_action();
    assert_eq!(model.confirm, None);
    assert_eq!(model.screen, Screen::Game);

    model.request_main_menu();
    model.confirm_pending_action();
    assert_eq!(model.screen, Screen::Title);
    assert!(model.session.is_none());
}

#[test]
fn show_spider_hint_does_not_mutate_session() {
    let _guard = install_test_storage();
    let mut model = AppModel::new();
    model.start_game_with_seed(GameKind::Spider, 123);

    let snapshot: Vec<Vec<crate::cards::Card>> = model
        .session
        .as_ref()
        .unwrap()
        .piles()
        .iter()
        .map(|p| p.cards.clone())
        .collect();

    model.show_spider_hint();

    let after: Vec<Vec<crate::cards::Card>> = model
        .session
        .as_ref()
        .unwrap()
        .piles()
        .iter()
        .map(|p| p.cards.clone())
        .collect();
    assert_eq!(snapshot, after, "Hint must not mutate piles");
    assert!(
        model.spider_hint.is_some(),
        "Fresh Spider deal always has at least the stock deal hint"
    );
}

#[test]
fn show_spider_hint_sets_no_moves_toast_on_dead_board() {
    // Mirrors the user-reported wedge: every legal cascade move
    // is sterile (duplicate-parent shuffle or a wholesale
    // relocation to an empty cascade) and the stock is empty.
    // The Hint button must drop a "No moves" toast on the model
    // instead of leaving the highlight on a misleading move.
    use crate::cards::{Card, Rank, Suit};
    let _guard = install_test_storage();
    let mut model = AppModel::new();
    model.start_game_with_seed(GameKind::Spider, 7);
    // Wipe the dealt state and rebuild the dead board by hand.
    {
        let session = model.session.as_mut().unwrap();
        let piles = session.piles_mut();
        for p in piles.iter_mut() {
            p.cards.clear();
        }
        for r in [Rank::Queen, Rank::Jack] {
            piles
                .get_mut(9)
                .cards
                .push(Card::new(Suit::Spades, r).face_up());
        }
        for r in [
            Rank::Queen,
            Rank::Jack,
            Rank::Ten,
            Rank::Nine,
            Rank::Eight,
            Rank::Seven,
            Rank::Six,
            Rank::Five,
            Rank::Four,
            Rank::Three,
            Rank::Two,
            Rank::Ace,
        ] {
            piles
                .get_mut(10)
                .cards
                .push(Card::new(Suit::Spades, r).face_up());
        }
        piles
            .get_mut(11)
            .cards
            .push(Card::new(Suit::Spades, Rank::Two).face_up());
    }

    model.show_spider_hint();

    assert!(
        model.spider_hint.is_none(),
        "dead board must not surface a hint"
    );
    assert!(
        model
            .toast
            .as_ref()
            .is_some_and(|(msg, _)| msg.contains("No moves")),
        "expected `No moves` toast, got {:?}",
        model.toast.as_ref().map(|(m, _)| m.clone())
    );
}

#[test]
fn show_spider_hint_sets_deal_toast_when_only_stock_left() {
    // User reported a board where every tableau move is sterile
    // but stock still has cards waiting to deal. The Hint button
    // must surface a StockDeal recommendation AND a "Deal more
    // cards" toast so the player understands the next action.
    use crate::cards::{Card, Rank, Suit};
    use crate::games::spider::SpiderHint;
    let _guard = install_test_storage();
    let mut model = AppModel::new();
    model.start_game_with_seed(GameKind::Spider, 11);
    {
        let session = model.session.as_mut().unwrap();
        let piles = session.piles_mut();
        for p in piles.iter_mut() {
            p.cards.clear();
        }
        // 10 cascades, each topped with a single King — Kings can
        // only land on empty cascades, and none are empty, so the
        // ranker has no tableau candidate. The stock still has
        // cards to deal, so the right hint is `StockDeal`.
        for i in 0..10u8 {
            piles
                .get_mut(9 + i)
                .cards
                .push(Card::new(Suit::Spades, Rank::King).face_up());
        }
        // Stock has at least 10 cards so a deal is legal.
        for _ in 0..10 {
            piles
                .get_mut(8)
                .cards
                .push(Card::new(Suit::Spades, Rank::Two));
        }
    }

    model.show_spider_hint();

    assert!(matches!(
        model.spider_hint,
        Some(SpiderHint::StockDeal { .. })
    ));
    assert!(
        model
            .toast
            .as_ref()
            .is_some_and(|(msg, _)| msg.contains("Deal")),
        "expected `Deal more cards` toast, got {:?}",
        model.toast.as_ref().map(|(m, _)| m.clone())
    );
}

#[test]
fn show_spider_hint_no_op_for_non_spider_games() {
    let _guard = install_test_storage();
    let mut model = AppModel::new();
    model.start_game_with_seed(GameKind::Klondike, 1);
    model.show_spider_hint();
    assert!(model.spider_hint.is_none());
}

fn apply_spider_stock_deal(model: &mut AppModel) {
    let session = model.session.as_mut().expect("test starts a game");
    let moves = session.on_pile_click(8);
    assert!(!moves.is_empty(), "fresh Spider stock should deal");
    assert!(session.try_apply_batch(moves));
    assert!(session.has_moves());
}

/// Ranks of a cascade, bottom-to-top — used to assert that a suit
/// swap leaves pile shapes untouched.
fn cascade_ranks(model: &AppModel, cid: u8) -> Vec<crate::cards::Rank> {
    model
        .session
        .as_ref()
        .unwrap()
        .piles()
        .get(cid)
        .cards
        .iter()
        .map(|c| c.rank)
        .collect()
}

#[test]
fn spider_one_suit_swap_remaps_in_place_and_preserves_undo() {
    use crate::cards::{Card, Rank, Suit};
    use crate::session::Move;
    let _guard = install_test_storage();
    let mut model = AppModel::new();
    // Defaults: 1-suit Spider, Spades.
    assert_eq!(model.spider_suit_count, 1);
    assert_eq!(model.spider_one_suit, Suit::Spades);
    model.start_game_with_seed(GameKind::Spider, 42);

    // Build a tiny controlled board: cascade 9 holds a lone Nine,
    // cascade 10 a lone Ten (both Spades). Moving the Nine onto the
    // Ten is a legal single-card cascade move and records an undo
    // entry.
    {
        let piles = model.session.as_mut().unwrap().piles_mut();
        for p in piles.iter_mut() {
            p.cards.clear();
        }
        piles
            .get_mut(9)
            .cards
            .push(Card::new(Suit::Spades, Rank::Nine).face_up());
        piles
            .get_mut(10)
            .cards
            .push(Card::new(Suit::Spades, Rank::Ten).face_up());
    }
    assert!(model
        .session
        .as_mut()
        .unwrap()
        .try_apply(Move::simple(9, 1, 10)));
    assert_eq!(cascade_ranks(&model, 9), vec![]);
    assert_eq!(cascade_ranks(&model, 10), vec![Rank::Ten, Rank::Nine]);

    // Swap the active suit. In 1-suit mode this is cosmetic: no
    // confirm dialog, no re-deal.
    model.set_spider_one_suit(Suit::Hearts);
    assert!(
        model.confirm.is_none(),
        "1-suit swap must not queue a confirm dialog"
    );
    assert_eq!(model.spider_one_suit, Suit::Hearts);

    // Every card everywhere now carries the new suit.
    for pile in model.session.as_ref().unwrap().piles().iter() {
        for card in &pile.cards {
            assert_eq!(card.suit, Suit::Hearts, "every card should be remapped");
        }
    }
    // Pile shapes/ranks are untouched by the swap.
    assert_eq!(cascade_ranks(&model, 9), vec![]);
    assert_eq!(cascade_ranks(&model, 10), vec![Rank::Ten, Rank::Nine]);

    // The new suit persisted to settings (round-trip).
    assert_eq!(UserSettings::load().spider_one_suit, Suit::Hearts);

    // Undo still reverts the last move, and the undone cards carry the
    // NEW suit (remapped history stays consistent with remapped piles).
    assert!(model.session.as_mut().unwrap().try_undo());
    assert_eq!(cascade_ranks(&model, 9), vec![Rank::Nine]);
    assert_eq!(cascade_ranks(&model, 10), vec![Rank::Ten]);
    let nine = *model
        .session
        .as_ref()
        .unwrap()
        .piles()
        .get(9)
        .top()
        .unwrap();
    assert_eq!(nine.suit, Suit::Hearts, "undone card carries the new suit");
}

#[test]
fn spider_suit_swap_while_four_suit_only_persists() {
    use crate::cards::Suit;
    let _guard = install_test_storage();
    let mut model = AppModel::new();
    // 4-suit Spider is active — the swap must not touch the session.
    model.spider_suit_count = 4;
    model.start_game_with_seed(GameKind::Spider, 7);

    let before: Vec<Vec<crate::cards::Card>> = model
        .session
        .as_ref()
        .unwrap()
        .piles()
        .iter()
        .map(|p| p.cards.clone())
        .collect();

    model.set_spider_one_suit(Suit::Hearts);

    let after: Vec<Vec<crate::cards::Card>> = model
        .session
        .as_ref()
        .unwrap()
        .piles()
        .iter()
        .map(|p| p.cards.clone())
        .collect();
    assert_eq!(before, after, "4-suit session must not be remapped");
    assert!(model.confirm.is_none());
    // The preference still persisted for the next 1-suit game.
    assert_eq!(model.spider_one_suit, Suit::Hearts);
    assert_eq!(UserSettings::load().spider_one_suit, Suit::Hearts);
}
