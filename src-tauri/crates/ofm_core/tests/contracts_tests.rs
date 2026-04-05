use chrono::{TimeZone, Utc};
use domain::manager::Manager;
use domain::player::{
    ContractRenewalState, Player, PlayerAttributes, Position, RenewalSessionStatus,
};
use domain::staff::{Staff, StaffAttributes, StaffRole};
use domain::team::Team;
use ofm_core::clock::GameClock;
use ofm_core::contracts::{
    DelegatedRenewalOptions, DelegatedRenewalResultStatus, RenewalDecision, RenewalOffer,
    delegate_renewals, evaluate_renewal_offer, propose_renewal,
};
use ofm_core::game::Game;

fn default_attrs() -> PlayerAttributes {
    PlayerAttributes {
        pace: 60,
        stamina: 60,
        strength: 60,
        agility: 60,
        passing: 60,
        shooting: 60,
        tackling: 60,
        dribbling: 60,
        defending: 60,
        positioning: 60,
        vision: 60,
        decisions: 60,
        composure: 60,
        aggression: 60,
        teamwork: 60,
        leadership: 60,
        handling: 30,
        reflexes: 30,
        aerial: 60,
    }
}

fn make_player() -> Player {
    let mut player = Player::new(
        "player-1".to_string(),
        "J. Smith".to_string(),
        "John Smith".to_string(),
        "2000-01-01".to_string(),
        "England".to_string(),
        Position::Forward,
        default_attrs(),
    );
    player.team_id = Some("team-1".to_string());
    player.contract_end = Some("2026-10-15".to_string());
    player.wage = 12_000;
    player.morale = 75;
    player.market_value = 350_000;
    player
}

fn make_player_with(id: &str, wage: u32, contract_end: &str) -> Player {
    let mut player = make_player();
    player.id = id.to_string();
    player.match_name = id.to_string();
    player.full_name = format!("Player {}", id);
    player.wage = wage;
    player.contract_end = Some(contract_end.to_string());
    player
}

fn make_team() -> Team {
    let mut team = Team::new(
        "team-1".to_string(),
        "Alpha FC".to_string(),
        "ALP".to_string(),
        "England".to_string(),
        "London".to_string(),
        "Alpha Ground".to_string(),
        30_000,
    );
    team.manager_id = Some("manager-1".to_string());
    team.reputation = 50;
    team.wage_budget = 50_000;
    team
}

fn make_assistant_manager() -> Staff {
    let mut staff = Staff::new(
        "staff-1".to_string(),
        "Alex".to_string(),
        "Assistant".to_string(),
        "1985-01-01".to_string(),
        StaffRole::AssistantManager,
        StaffAttributes {
            coaching: 82,
            judging_ability: 76,
            judging_potential: 74,
            physiotherapy: 30,
        },
    );
    staff.team_id = Some("team-1".to_string());
    staff
}

fn make_game() -> Game {
    let clock = GameClock::new(Utc.with_ymd_and_hms(2026, 8, 1, 12, 0, 0).unwrap());
    let mut manager = Manager::new(
        "manager-1".to_string(),
        "Jane".to_string(),
        "Doe".to_string(),
        "1980-01-01".to_string(),
        "England".to_string(),
    );
    manager.hire("team-1".to_string());

    Game::new(
        clock,
        manager,
        vec![make_team()],
        vec![make_player()],
        vec![],
        vec![],
    )
}

#[test]
fn accepted_offer_updates_wage_and_term_correctly() {
    let mut game = make_game();

    let outcome = propose_renewal(
        &mut game,
        "player-1",
        RenewalOffer {
            weekly_wage: 15_000,
            contract_years: 3,
        },
    )
    .expect("renewal should succeed");

    assert!(matches!(outcome.decision, RenewalDecision::Accepted));
    assert!(outcome.feedback.is_some());
    let player = game.players.iter().find(|p| p.id == "player-1").unwrap();
    assert_eq!(player.wage, 15_000);
    assert_eq!(player.contract_end.as_deref(), Some("2029-08-01"));
}

#[test]
fn rejected_offer_leaves_state_unchanged() {
    let mut game = make_game();
    let original_wage = game.players[0].wage;
    let original_end = game.players[0].contract_end.clone();

    let outcome = propose_renewal(
        &mut game,
        "player-1",
        RenewalOffer {
            weekly_wage: 7_000,
            contract_years: 1,
        },
    )
    .expect("renewal should return a decision");

    assert!(matches!(outcome.decision, RenewalDecision::Rejected));
    assert_eq!(game.players[0].wage, original_wage);
    assert_eq!(game.players[0].contract_end, original_end);
}

#[test]
fn counter_offer_returns_understandable_feedback() {
    let mut game = make_game();

    let outcome = propose_renewal(
        &mut game,
        "player-1",
        RenewalOffer {
            weekly_wage: 13_000,
            contract_years: 2,
        },
    )
    .expect("renewal should return a counter offer");

    assert!(matches!(outcome.decision, RenewalDecision::CounterOffer));
    assert_eq!(outcome.suggested_wage, Some(14_000));
    assert_eq!(outcome.suggested_years, Some(3));
    let feedback = outcome.feedback.expect("feedback should be present");
    assert_eq!(feedback.round, 1);
    assert!(feedback.tension > 0);
}

#[test]
fn high_value_star_expects_more_than_fringe_player() {
    let current_date = Utc
        .with_ymd_and_hms(2026, 8, 1, 12, 0, 0)
        .unwrap()
        .date_naive();
    let team = make_team();

    let mut star = make_player();
    star.contract_end = Some("2028-08-01".to_string());
    star.market_value = 2_500_000;
    star.attributes.pace = 88;
    star.attributes.shooting = 90;
    star.attributes.dribbling = 87;

    let mut fringe = make_player();
    fringe.contract_end = Some("2028-08-01".to_string());
    fringe.market_value = 80_000;
    fringe.attributes.pace = 50;
    fringe.attributes.shooting = 48;
    fringe.attributes.dribbling = 49;

    let offer = RenewalOffer {
        weekly_wage: 14_000,
        contract_years: 3,
    };

    let star_outcome = evaluate_renewal_offer(&star, &team, current_date, &offer);
    let fringe_outcome = evaluate_renewal_offer(&fringe, &team, current_date, &offer);

    assert!(matches!(fringe_outcome.decision, RenewalDecision::Accepted));
    assert!(matches!(
        star_outcome.decision,
        RenewalDecision::CounterOffer
    ));
    assert!(star_outcome.suggested_wage > fringe_outcome.suggested_wage);
}

#[test]
fn low_morale_player_becomes_harder_to_renew_than_content_player() {
    let current_date = Utc
        .with_ymd_and_hms(2026, 8, 1, 12, 0, 0)
        .unwrap()
        .date_naive();
    let team = make_team();

    let mut content_player = make_player();
    content_player.contract_end = Some("2028-08-01".to_string());
    content_player.morale = 85;

    let mut unhappy_player = make_player();
    unhappy_player.contract_end = Some("2028-08-01".to_string());
    unhappy_player.morale = 35;

    let offer = RenewalOffer {
        weekly_wage: 13_000,
        contract_years: 3,
    };

    let content_outcome = evaluate_renewal_offer(&content_player, &team, current_date, &offer);
    let unhappy_outcome = evaluate_renewal_offer(&unhappy_player, &team, current_date, &offer);

    assert!(matches!(
        content_outcome.decision,
        RenewalDecision::Accepted
    ));
    assert!(matches!(
        unhappy_outcome.decision,
        RenewalDecision::CounterOffer
    ));
}

#[test]
fn shorter_remaining_term_increases_renewal_demands() {
    let current_date = Utc
        .with_ymd_and_hms(2026, 8, 1, 12, 0, 0)
        .unwrap()
        .date_naive();
    let team = make_team();

    let mut secure_player = make_player();
    secure_player.contract_end = Some("2028-08-01".to_string());

    let mut expiring_player = make_player();
    expiring_player.contract_end = Some("2026-10-01".to_string());

    let offer = RenewalOffer {
        weekly_wage: 13_000,
        contract_years: 3,
    };

    let secure_outcome = evaluate_renewal_offer(&secure_player, &team, current_date, &offer);
    let expiring_outcome = evaluate_renewal_offer(&expiring_player, &team, current_date, &offer);

    assert!(matches!(secure_outcome.decision, RenewalDecision::Accepted));
    assert!(matches!(
        expiring_outcome.decision,
        RenewalDecision::CounterOffer
    ));
}

#[test]
fn low_manager_trust_player_can_refuse_manual_renewal_even_at_fair_terms() {
    let mut game = make_game();
    game.players[0].morale_core.manager_trust = 18;

    let outcome = propose_renewal(
        &mut game,
        "player-1",
        RenewalOffer {
            weekly_wage: 15_000,
            contract_years: 3,
        },
    )
    .expect("renewal should produce an outcome");

    assert!(matches!(outcome.decision, RenewalDecision::Rejected));
}

#[test]
fn manager_block_prevents_manual_renewal_until_it_expires() {
    let mut game = make_game();
    game.players[0].morale_core.renewal_state = Some(ContractRenewalState {
        status: RenewalSessionStatus::Blocked,
        manager_blocked_until: Some("2026-09-01".to_string()),
        last_attempt_date: None,
        last_assistant_attempt_date: None,
        last_outcome: None,
        conversation_round: 0,
    });

    let outcome = propose_renewal(
        &mut game,
        "player-1",
        RenewalOffer {
            weekly_wage: 16_000,
            contract_years: 3,
        },
    )
    .expect("renewal should produce an outcome");

    assert!(matches!(outcome.decision, RenewalDecision::Rejected));
    assert_eq!(outcome.session_status, RenewalSessionStatus::Blocked);
    assert!(outcome.is_terminal);
}

#[test]
fn stale_manual_renewal_talks_cool_off_and_restart_from_round_one() {
    let mut game = make_game();
    game.players[0].morale_core.renewal_state = Some(ContractRenewalState {
        status: RenewalSessionStatus::Open,
        manager_blocked_until: None,
        last_attempt_date: Some("2026-08-10".to_string()),
        last_assistant_attempt_date: None,
        last_outcome: None,
        conversation_round: 3,
    });
    game.clock = GameClock::new(Utc.with_ymd_and_hms(2026, 8, 26, 12, 0, 0).unwrap());

    let outcome = propose_renewal(
        &mut game,
        "player-1",
        RenewalOffer {
            weekly_wage: 13_000,
            contract_years: 2,
        },
    )
    .expect("renewal should produce an outcome");

    assert!(outcome.cooled_off);
    let feedback = outcome.feedback.expect("feedback should be present");
    assert_eq!(feedback.round, 1);

    let renewal_state = game.players[0]
        .morale_core
        .renewal_state
        .as_ref()
        .expect("renewal state should be stored");
    assert_eq!(renewal_state.conversation_round, 1);
    assert_eq!(
        renewal_state.last_attempt_date.as_deref(),
        Some("2026-08-26")
    );
}

#[test]
fn assistant_can_complete_routine_delegate_renewal_even_when_manager_trust_is_low() {
    let mut game = make_game();
    game.staff.push(make_assistant_manager());
    game.players[0].morale_core.manager_trust = 24;
    game.players[0].morale = 74;

    let report = delegate_renewals(
        &mut game,
        DelegatedRenewalOptions {
            player_ids: Some(vec!["player-1".to_string()]),
            staff_ids: None,
            max_wage_increase_pct: 35,
            max_contract_years: 3,
        },
    )
    .expect("assistant delegation should return a report");

    assert_eq!(report.success_count, 1);
    assert_eq!(report.failure_count, 0);
    assert_eq!(report.stalled_count, 0);
    assert_eq!(report.cases.len(), 1);
    assert_eq!(report.cases[0].player_id, "player-1");
    assert_eq!(
        report.cases[0].status,
        DelegatedRenewalResultStatus::Successful
    );

    let player = game
        .players
        .iter()
        .find(|player| player.id == "player-1")
        .unwrap();
    assert_eq!(player.contract_end.as_deref(), Some("2029-08-01"));
    assert!(player.wage >= 14_000);

    let report_message = game
        .messages
        .iter()
        .find(|message| message.id.starts_with("delegated_renewals_"))
        .expect("assistant delegation should create an inbox report");
    assert_eq!(
        report_message.subject_key.as_deref(),
        Some("be.msg.delegatedRenewals.subject")
    );
    assert_eq!(
        report_message.body_key.as_deref(),
        Some("be.msg.delegatedRenewals.body")
    );
    assert_eq!(
        report_message.sender_key.as_deref(),
        Some("be.sender.assistantManager")
    );
    let structured_report = report_message
        .context
        .delegated_renewal_report
        .as_ref()
        .expect("assistant report should carry structured i18n-safe case data");
    assert_eq!(structured_report.success_count, 1);
    assert_eq!(structured_report.cases.len(), 1);
    assert_eq!(structured_report.cases[0].status, "successful");
}

#[test]
fn renewal_is_blocked_when_offer_pushes_healthy_club_far_over_soft_cap() {
    let mut game = make_game();
    game.teams[0].wage_budget = 200_000;

    let err = propose_renewal(
        &mut game,
        "player-1",
        RenewalOffer {
            weekly_wage: 250_000,
            contract_years: 3,
        },
    )
    .expect_err("renewal should be blocked by wage policy");

    assert!(err.contains("board wage policy"));
}

#[test]
fn renewal_allows_small_increase_for_legacy_over_budget_saves() {
    let mut game = make_game();
    game.teams[0].wage_budget = 50_000;
    game.players[0].wage = 48_000;
    game.players
        .push(make_player_with("player-2", 40_000, "2027-06-30"));

    let outcome = propose_renewal(
        &mut game,
        "player-1",
        RenewalOffer {
            weekly_wage: 70_000,
            contract_years: 2,
        },
    );

    assert!(
        outcome.is_ok(),
        "legacy saves should allow manageable wage increases without policy lock"
    );
}

#[test]
fn renewal_blocks_large_worsening_for_legacy_over_budget_saves() {
    let mut game = make_game();
    game.teams[0].wage_budget = 50_000;
    game.players[0].wage = 48_000;
    game.players
        .push(make_player_with("player-2", 40_000, "2027-06-30"));

    let err = propose_renewal(
        &mut game,
        "player-1",
        RenewalOffer {
            weekly_wage: 120_000,
            contract_years: 3,
        },
    )
    .expect_err("large worsening should still be blocked");

    assert!(err.contains("board wage policy"));
}
