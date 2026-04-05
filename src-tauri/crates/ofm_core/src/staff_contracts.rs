//! Staff contract renewals (player-parity negotiation + board wage policy).
use chrono::{Datelike, Months, NaiveDate};
use std::collections::HashMap;

use domain::negotiation::{NegotiationFeedback, NegotiationMood};
use domain::player::{
    ContractRenewalState, RenewalSessionOutcome, RenewalSessionStatus,
};
use domain::staff::{Staff, StaffRole};
use domain::team::Team;

use crate::contract_wage_policy::{
    renewal_wage_policy_allows, renewal_wage_policy_error_message,
};
use crate::contracts::{
    RenewalDecision, RenewalFinancialProjection, RenewalOffer, RenewalOutcome,
    round_up_to_nearest_thousand,
};
use crate::finances::calc_cash_runway_weeks;
use crate::game::Game;

const RENEWAL_SESSION_STALE_DAYS: i64 = 14;

fn staff_age_on(current_date: NaiveDate, date_of_birth: &str) -> i32 {
    let Ok(dob) = NaiveDate::parse_from_str(date_of_birth, "%Y-%m-%d") else {
        return 40;
    };
    let mut age = current_date.year() - dob.year();
    if current_date.ordinal() < dob.ordinal() {
        age -= 1;
    }
    age
}

fn staff_remaining_contract_days(staff: &Staff, current_date: NaiveDate) -> i64 {
    crate::staff_ops::staff_contract_days_remaining(staff.contract_end.as_deref(), current_date)
}

fn staff_quality_tier_mult(staff: &Staff) -> f32 {
    let ovr = (staff.attributes.coaching as u32
        + staff.attributes.judging_ability as u32
        + staff.attributes.judging_potential as u32
        + staff.attributes.physiotherapy as u32) as f32
        / 4.0;
    if ovr >= 75.0 {
        1.12
    } else if ovr >= 60.0 {
        1.06
    } else if ovr <= 40.0 {
        0.95
    } else {
        1.0
    }
}

fn role_importance_mult(role: &StaffRole) -> f32 {
    match role {
        StaffRole::AssistantManager => 1.1,
        StaffRole::Coach => 1.05,
        StaffRole::Scout => 1.0,
        StaffRole::Physio => 1.0,
    }
}

pub fn expected_staff_wage(staff: &Staff, team: &Team, current_date: NaiveDate) -> u32 {
    let mut wage = staff.wage as f32;
    let age = staff_age_on(current_date, &staff.date_of_birth);
    let remaining_days = staff_remaining_contract_days(staff, current_date);

    if age <= 40 {
        wage *= 1.04;
    } else if age >= 55 {
        wage *= 0.94;
    }

    if staff.morale <= 50 {
        wage *= 1.08;
    }

    wage *= staff_quality_tier_mult(staff);
    wage *= role_importance_mult(&staff.role);

    if team.reputation < 40 {
        wage *= 1.05;
    }

    if remaining_days <= 180 {
        wage *= 1.10;
    } else if remaining_days <= 365 {
        wage *= 1.05;
    }

    let rounded = round_up_to_nearest_thousand(wage.ceil() as u32);
    rounded.max(staff.wage)
}

pub fn expected_staff_contract_years(staff: &Staff, current_date: NaiveDate) -> u32 {
    let age = staff_age_on(current_date, &staff.date_of_birth);
    if age <= 45 {
        return 3;
    }
    if age <= 52 {
        return 2;
    }
    1
}

fn minimum_acceptable_wage_staff(current_wage: u32) -> u32 {
    ((current_wage as f32) * 0.85).floor() as u32
}

fn next_renewal_round_staff(staff: &Staff, today: Option<&str>) -> u8 {
    let Some(state) = staff.morale_core.renewal_state.as_ref() else {
        return 1;
    };
    if let Some(today) = today {
        if state.last_attempt_date.as_deref() != Some(today) {
            return 1;
        }
    }
    state.conversation_round.saturating_add(1).max(1)
}

fn cool_stale_renewal_session_staff(staff: &mut Staff, current_date: NaiveDate) -> bool {
    let Some(state) = staff.morale_core.renewal_state.as_mut() else {
        return false;
    };
    if matches!(
        state.status,
        RenewalSessionStatus::Blocked | RenewalSessionStatus::Agreed | RenewalSessionStatus::Idle
    ) {
        return false;
    }
    let Some(last_attempt_date) = state.last_attempt_date.as_deref() else {
        return false;
    };
    let Ok(last_attempt) = NaiveDate::parse_from_str(last_attempt_date, "%Y-%m-%d") else {
        return false;
    };
    if (current_date - last_attempt).num_days() < RENEWAL_SESSION_STALE_DAYS {
        return false;
    }
    state.status = RenewalSessionStatus::Idle;
    state.last_outcome = None;
    state.conversation_round = 0;
    true
}

pub fn has_active_manager_block_staff(staff: &Staff, current_date: NaiveDate) -> bool {
    let Some(state) = staff.morale_core.renewal_state.as_ref() else {
        return false;
    };
    if state.status != RenewalSessionStatus::Blocked {
        return false;
    }
    let Some(blocked_until) = state.manager_blocked_until.as_deref() else {
        return true;
    };
    NaiveDate::parse_from_str(blocked_until, "%Y-%m-%d")
        .map(|blocked_until| blocked_until >= current_date)
        .unwrap_or(true)
}

fn should_manual_renewal_fail_on_relationship_staff(
    staff: &Staff,
    expected_wage: u32,
    offered_wage: u32,
) -> bool {
    let trust = staff.morale_core.manager_trust;
    let relationship_margin = if trust <= 20 {
        2_000u32
    } else if trust <= 30 {
        1_000
    } else {
        0
    };
    relationship_margin > 0 && offered_wage < expected_wage.saturating_add(relationship_margin)
}

fn build_staff_renewal_feedback(
    staff: &Staff,
    current_date: NaiveDate,
    decision: RenewalDecision,
    session_status: RenewalSessionStatus,
    round: u8,
    expected_wage: u32,
    relationship_blocked: bool,
) -> NegotiationFeedback {
    let trust = staff.morale_core.manager_trust;
    let remaining_days = staff_remaining_contract_days(staff, current_date);
    let urgency_pressure = if remaining_days <= 90 {
        24
    } else if remaining_days <= 180 {
        16
    } else if remaining_days <= 365 {
        8
    } else {
        2
    };
    let morale_pressure = if staff.morale <= 40 {
        24
    } else if staff.morale <= 60 {
        12
    } else {
        0
    };
    let trust_pressure = if trust <= 25 {
        26
    } else if trust <= 40 {
        12
    } else {
        0
    };
    let tier = staff_quality_tier_mult(staff);
    let value_pressure = if tier >= 1.1 {
        10
    } else if tier >= 1.05 {
        5
    } else {
        0
    };
    let tension = (22 + urgency_pressure + morale_pressure + trust_pressure + value_pressure)
        .clamp(10, 92) as u8;
    let patience = (100_i32 - i32::from(round.saturating_sub(1)) * 18 - i32::from(tension) / 3)
        .clamp(18, 92) as u8;

    let (mood, headline_key, detail_key) = if session_status == RenewalSessionStatus::Blocked {
        (
            NegotiationMood::Guarded,
            "staffProfile.renewalFeedbackBlockedHeadline",
            Some("staffProfile.renewalFeedbackBlockedDetail"),
        )
    } else if decision == RenewalDecision::Accepted && round >= 2 {
        (
            NegotiationMood::Positive,
            "staffProfile.renewalFeedbackAcceptedLateHeadline",
            Some("staffProfile.renewalFeedbackAcceptedLateDetail"),
        )
    } else if decision == RenewalDecision::Accepted {
        (
            NegotiationMood::Positive,
            "staffProfile.renewalFeedbackAcceptedHeadline",
            Some("staffProfile.renewalFeedbackAcceptedDetail"),
        )
    } else if relationship_blocked || tension >= 70 {
        (
            NegotiationMood::Tense,
            "staffProfile.renewalFeedbackTenseHeadline",
            Some("staffProfile.renewalFeedbackTenseDetail"),
        )
    } else if expected_wage > staff.wage || round >= 2 {
        (
            NegotiationMood::Firm,
            "staffProfile.renewalFeedbackFirmHeadline",
            Some("staffProfile.renewalFeedbackFirmDetail"),
        )
    } else {
        (
            NegotiationMood::Calm,
            "staffProfile.renewalFeedbackCalmHeadline",
            Some("staffProfile.renewalFeedbackCalmDetail"),
        )
    };

    NegotiationFeedback {
        mood,
        headline_key: headline_key.to_string(),
        detail_key: detail_key.map(str::to_string),
        tension,
        patience,
        round,
        params: HashMap::new(),
    }
}

fn renewal_outcome_staff(
    decision: RenewalDecision,
    suggested_wage: Option<u32>,
    suggested_years: Option<u32>,
    session_status: RenewalSessionStatus,
    is_terminal: bool,
    cooled_off: bool,
    feedback: Option<NegotiationFeedback>,
) -> RenewalOutcome {
    RenewalOutcome {
        decision,
        suggested_wage,
        suggested_years,
        session_status,
        is_terminal,
        cooled_off,
        feedback,
    }
}

pub fn evaluate_staff_renewal_offer(
    staff: &Staff,
    team: &Team,
    current_date: NaiveDate,
    offer: &RenewalOffer,
) -> RenewalOutcome {
    let round = next_renewal_round_staff(staff, None);
    let expected_wage = expected_staff_wage(staff, team, current_date);
    let expected_years = expected_staff_contract_years(staff, current_date);
    let minimum_wage = minimum_acceptable_wage_staff(staff.wage);

    if offer.weekly_wage < minimum_wage || offer.contract_years == 0 {
        let feedback = build_staff_renewal_feedback(
            staff,
            current_date,
            RenewalDecision::Rejected,
            RenewalSessionStatus::Stalled,
            round,
            expected_wage,
            false,
        );
        return renewal_outcome_staff(
            RenewalDecision::Rejected,
            None,
            None,
            RenewalSessionStatus::Stalled,
            false,
            false,
            Some(feedback),
        );
    }

    if offer.weekly_wage >= expected_wage && offer.contract_years >= expected_years {
        let feedback = build_staff_renewal_feedback(
            staff,
            current_date,
            RenewalDecision::Accepted,
            RenewalSessionStatus::Agreed,
            round,
            expected_wage,
            false,
        );
        return renewal_outcome_staff(
            RenewalDecision::Accepted,
            None,
            None,
            RenewalSessionStatus::Agreed,
            true,
            false,
            Some(feedback),
        );
    }

    let feedback = build_staff_renewal_feedback(
        staff,
        current_date,
        RenewalDecision::CounterOffer,
        RenewalSessionStatus::Open,
        round,
        expected_wage,
        false,
    );

    renewal_outcome_staff(
        RenewalDecision::CounterOffer,
        Some(expected_wage),
        Some(expected_years),
        RenewalSessionStatus::Open,
        false,
        false,
        Some(feedback),
    )
}

pub fn propose_staff_renewal(
    game: &mut Game,
    staff_id: &str,
    offer: RenewalOffer,
) -> Result<RenewalOutcome, String> {
    let manager_team_id = game
        .manager
        .team_id
        .clone()
        .ok_or_else(|| "No team assigned".to_string())?;

    let team = game
        .teams
        .iter()
        .find(|c| c.id == manager_team_id)
        .ok_or_else(|| "Manager team not found".to_string())?
        .clone();

    let staff_index = game
        .staff
        .iter()
        .position(|s| s.id == staff_id)
        .ok_or_else(|| "Staff member not found".to_string())?;

    if game.staff[staff_index].team_id.as_deref() != Some(team.id.as_str()) {
        return Err("Staff member does not belong to your club".to_string());
    }

    let current_date = game.clock.current_date.date_naive();
    let cooled_off = cool_stale_renewal_session_staff(&mut game.staff[staff_index], current_date);
    let today = current_date.format("%Y-%m-%d").to_string();
    let round = next_renewal_round_staff(&game.staff[staff_index], Some(today.as_str()));

    if has_active_manager_block_staff(&game.staff[staff_index], current_date) {
        return Ok(renewal_outcome_staff(
            RenewalDecision::Rejected,
            None,
            None,
            RenewalSessionStatus::Blocked,
            true,
            cooled_off,
            Some(build_staff_renewal_feedback(
                &game.staff[staff_index],
                current_date,
                RenewalDecision::Rejected,
                RenewalSessionStatus::Blocked,
                round,
                0,
                false,
            )),
        ));
    }

    if let Some(state) = game.staff[staff_index].morale_core.renewal_state.as_ref()
        && state.status == RenewalSessionStatus::Agreed
        && state.last_attempt_date.as_deref() == Some(today.as_str())
    {
        return Ok(renewal_outcome_staff(
            RenewalDecision::Rejected,
            None,
            None,
            RenewalSessionStatus::Agreed,
            true,
            cooled_off,
            Some(build_staff_renewal_feedback(
                &game.staff[staff_index],
                current_date,
                RenewalDecision::Accepted,
                RenewalSessionStatus::Agreed,
                round,
                game.staff[staff_index].wage,
                false,
            )),
        ));
    }

    let expected_wage = expected_staff_wage(&game.staff[staff_index], &team, current_date);
    let mut outcome = evaluate_staff_renewal_offer(
        &game.staff[staff_index],
        &team,
        current_date,
        &offer,
    );
    outcome.cooled_off = cooled_off;
    let relationship_blocked = should_manual_renewal_fail_on_relationship_staff(
        &game.staff[staff_index],
        expected_wage,
        offer.weekly_wage,
    );

    if relationship_blocked {
        outcome = renewal_outcome_staff(
            RenewalDecision::Rejected,
            None,
            None,
            RenewalSessionStatus::Stalled,
            false,
            cooled_off,
            Some(build_staff_renewal_feedback(
                &game.staff[staff_index],
                current_date,
                RenewalDecision::Rejected,
                RenewalSessionStatus::Stalled,
                round,
                expected_wage,
                true,
            )),
        );
    }

    if outcome.decision == RenewalDecision::Accepted {
        if !renewal_wage_policy_allows(
            game,
            &team,
            game.staff[staff_index].wage,
            offer.weekly_wage,
        ) {
            return Err(renewal_wage_policy_error_message(&team));
        }

        let new_contract_end = current_date
            .checked_add_months(Months::new(offer.contract_years * 12))
            .ok_or_else(|| "Unable to calculate new contract end date".to_string())?;

        let st = &mut game.staff[staff_index];
        st.wage = offer.weekly_wage;
        st.contract_end = Some(new_contract_end.format("%Y-%m-%d").to_string());
        let state = st
            .morale_core
            .renewal_state
            .get_or_insert_with(ContractRenewalState::default);
        state.status = RenewalSessionStatus::Agreed;
        state.manager_blocked_until = None;
        state.last_attempt_date = Some(today);
        state.last_outcome = Some(RenewalSessionOutcome::AcceptedByManager);
        state.conversation_round = round;
        return Ok(renewal_outcome_staff(
            RenewalDecision::Accepted,
            None,
            None,
            RenewalSessionStatus::Agreed,
            true,
            cooled_off,
            Some(build_staff_renewal_feedback(
                st,
                current_date,
                RenewalDecision::Accepted,
                RenewalSessionStatus::Agreed,
                round,
                expected_wage,
                false,
            )),
        ));
    }

    let st = &mut game.staff[staff_index];
    let state = st
        .morale_core
        .renewal_state
        .get_or_insert_with(ContractRenewalState::default);
    state.last_attempt_date = Some(today);
    state.conversation_round = round;

    match outcome.decision {
        RenewalDecision::Rejected => {
            state.status = outcome.session_status.clone();
            state.last_outcome = Some(RenewalSessionOutcome::RejectedByPlayer);
        }
        RenewalDecision::CounterOffer => {
            state.status = RenewalSessionStatus::Open;
            state.last_outcome = Some(RenewalSessionOutcome::Stalled);
        }
        RenewalDecision::Accepted => {}
    }

    if outcome.feedback.is_none() {
        outcome.feedback = Some(build_staff_renewal_feedback(
            st,
            current_date,
            outcome.decision.clone(),
            outcome.session_status.clone(),
            round,
            expected_wage,
            relationship_blocked,
        ));
    }

    Ok(outcome)
}

pub fn project_staff_renewal_financial_impact(
    game: &Game,
    staff_id: &str,
    offered_wage: u32,
) -> Result<RenewalFinancialProjection, String> {
    let staff = game
        .staff
        .iter()
        .find(|s| s.id == staff_id)
        .ok_or_else(|| "Staff member not found".to_string())?;
    let team_id = staff
        .team_id
        .as_deref()
        .ok_or_else(|| "Staff member has no team".to_string())?;
    let team = game
        .teams
        .iter()
        .find(|t| t.id == team_id)
        .ok_or_else(|| "Team not found".to_string())?;

    let current_bill = crate::contract_wage_policy::annual_team_wage_bill_for_projection(game, team_id);
    let projected_bill =
        crate::contract_wage_policy::projected_annual_wage_bill_for_entity(
            game,
            team_id,
            staff.wage,
            offered_wage,
        );
    let annual_wage_budget = team.wage_budget;
    let annual_soft_cap = (annual_wage_budget * 110) / 100;
    let current_weekly_wage_spend = current_bill / 52;
    let projected_weekly_wage_spend = projected_bill / 52;

    let current_cash_runway_weeks =
        calc_cash_runway_weeks(team.finance, -current_weekly_wage_spend);
    let projected_cash_runway_weeks =
        calc_cash_runway_weeks(team.finance, -projected_weekly_wage_spend);

    Ok(RenewalFinancialProjection {
        current_annual_wage_bill: current_bill,
        projected_annual_wage_bill: projected_bill,
        annual_wage_budget,
        annual_soft_cap,
        current_weekly_wage_spend,
        projected_weekly_wage_spend,
        current_cash_runway_weeks,
        projected_cash_runway_weeks,
        currently_over_budget: current_bill > annual_wage_budget,
        policy_allows: renewal_wage_policy_allows(game, team, staff.wage, offered_wage),
    })
}
