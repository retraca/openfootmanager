//! Hire/release finances, severance, scouting cleanup (player-parity payroll model).
use chrono::{Months, NaiveDate};

use domain::staff::{Staff, StaffRole};
use domain::team::Team;

use crate::contracts::{contract_days_remaining_public, round_up_to_nearest_thousand};
use crate::game::Game;

/// Max weeks of pay used for severance cap (half a season).
const SEVERANCE_MAX_WEEKS: i64 = 26;

/// Public wrapper for contract remaining days (positive or zero).
pub fn staff_contract_days_remaining(
    contract_end: Option<&str>,
    current_date: NaiveDate,
) -> i64 {
    contract_days_remaining_public(contract_end, current_date)
        .unwrap_or(0)
        .max(0)
}

/// Annual wage from attributes / role band, scaled by club reputation (generation & hire fallback).
pub fn proposed_initial_annual_wage_from_rep(staff: &Staff, team_reputation: u32) -> u32 {
    let ovr = (staff.attributes.coaching as u32
        + staff.attributes.judging_ability as u32
        + staff.attributes.judging_potential as u32
        + staff.attributes.physiotherapy as u32)
        / 4;
    let rep_term = 50u32.saturating_add(team_reputation / 2);
    let mut base = 12_000u32.saturating_add(ovr.saturating_mul(350));
    base = base.saturating_mul(rep_term) / 100;
    let mult = match staff.role {
        StaffRole::AssistantManager => 1.15,
        StaffRole::Coach => 1.0,
        StaffRole::Scout => 0.95,
        StaffRole::Physio => 0.9,
    };
    round_up_to_nearest_thousand(((base as f64) * mult).ceil() as u32).max(6_000)
}

/// Annual wage offer for a newly hired staff member when the pool entity has wage 0.
pub fn proposed_initial_annual_wage(staff: &Staff, team: &Team) -> u32 {
    proposed_initial_annual_wage_from_rep(staff, team.reputation)
}

/// Sets wage and contract end when hiring from the market if missing (annual wage).
pub fn ensure_staff_contract_on_hire(
    current_date: NaiveDate,
    staff: &mut Staff,
    hiring_team: &Team,
) {
    if staff.wage == 0 {
        staff.wage = proposed_initial_annual_wage(staff, hiring_team);
    }
    if staff.contract_end.is_none() {
        if let Some(end) = current_date.checked_add_months(Months::new(24)) {
            staff.contract_end = Some(end.format("%Y-%m-%d").to_string());
        }
    }
    staff.morale = staff.morale.max(60).min(100);
}

/// Cash severance when sacking staff: proportional to remaining contract, capped.
pub fn severance_for_staff_release(staff: &Staff, current_date: NaiveDate) -> i64 {
    let remaining_days = staff_contract_days_remaining(staff.contract_end.as_deref(), current_date);
    if remaining_days <= 0 || staff.wage == 0 {
        return 0;
    }

    let weekly = (staff.wage as i64 / 52).max(1);
    let owed_weeks = (remaining_days / 7).min(SEVERANCE_MAX_WEEKS);
    let full = owed_weeks * weekly;

    let tail_days = remaining_days % 7;
    let partial = (tail_days * weekly) / 7;

    full.saturating_add(partial)
}

/// Clear employment contract fields when staff becomes a free agent.
pub fn reset_staff_to_free_agent(staff: &mut Staff) {
    staff.team_id = None;
    staff.contract_end = None;
    staff.wage = 0;
    staff.morale_core.renewal_state = None;
}

/// Remove active scouting trips for this scout (scout_id references staff id).
pub fn remove_scouting_assignments_for_scout(game: &mut Game, scout_id: &str) {
    game.scouting_assignments
        .retain(|a| a.scout_id != scout_id);
}
