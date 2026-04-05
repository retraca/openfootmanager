use log::info;
use tauri::State;

use ofm_core::game::Game;
use ofm_core::staff_ops::{
    ensure_staff_contract_on_hire, remove_scouting_assignments_for_scout,
    reset_staff_to_free_agent, severance_for_staff_release,
};
use ofm_core::state::StateManager;

#[tauri::command]
pub fn hire_staff(state: State<'_, StateManager>, staff_id: String) -> Result<Game, String> {
    hire_staff_internal(&state, &staff_id)
}

fn hire_staff_internal(state: &StateManager, staff_id: &str) -> Result<Game, String> {
    info!("[cmd] hire_staff: staff_id={}", staff_id);
    let mut game = state
        .get_game(|g| g.clone())
        .ok_or("No active game session".to_string())?;

    let team_id = game
        .manager
        .team_id
        .clone()
        .ok_or("No team assigned".to_string())?;

    let hiring_team = game
        .teams
        .iter()
        .find(|t| t.id == team_id)
        .cloned()
        .ok_or_else(|| "Team not found".to_string())?;
    let current_date = game.clock.current_date.date_naive();

    let staff = game
        .staff
        .iter_mut()
        .find(|s| s.id == staff_id)
        .ok_or("Staff member not found".to_string())?;

    if staff.team_id.is_some() {
        return Err("Staff member already employed by a team".to_string());
    }

    staff.team_id = Some(team_id.clone());
    ensure_staff_contract_on_hire(current_date, staff, &hiring_team);

    state.set_game(game.clone());
    Ok(game)
}

#[cfg(test)]
mod tests {
    use super::{hire_staff_internal, release_staff_internal};
    use chrono::{TimeZone, Utc};
    use domain::manager::Manager;
    use domain::staff::{Staff, StaffAttributes, StaffRole};
    use domain::team::Team;
    use ofm_core::clock::GameClock;
    use ofm_core::game::Game;
    use ofm_core::state::StateManager;

    fn make_team() -> Team {
        let mut team = Team::new(
            "team-1".to_string(),
            "User FC".to_string(),
            "USR".to_string(),
            "England".to_string(),
            "London".to_string(),
            "User Ground".to_string(),
            25_000,
        );
        team.manager_id = Some("manager-1".to_string());
        team
    }

    fn make_staff() -> Staff {
        let mut staff = Staff::new(
            "staff-1".to_string(),
            "Alex".to_string(),
            "Coach".to_string(),
            "1985-01-01".to_string(),
            StaffRole::Coach,
            StaffAttributes {
                coaching: 70,
                judging_ability: 50,
                judging_potential: 50,
                physiotherapy: 30,
            },
        );
        staff.wage = 12_000;
        staff
    }

    fn make_employed_staff() -> Staff {
        let mut staff = make_staff();
        staff.team_id = Some("team-1".to_string());
        staff.contract_end = Some("2027-08-01".to_string());
        staff
    }

    fn make_game() -> Game {
        let clock = GameClock::new(Utc.with_ymd_and_hms(2026, 8, 1, 12, 0, 0).unwrap());
        let mut manager = Manager::new(
            "manager-1".to_string(),
            "Test".to_string(),
            "Manager".to_string(),
            "1980-01-01".to_string(),
            "England".to_string(),
        );
        manager.hire("team-1".to_string());

        Game::new(
            clock,
            manager,
            vec![make_team()],
            vec![],
            vec![make_staff()],
            vec![],
        )
    }

    fn make_game_with_employed_staff() -> Game {
        let clock = GameClock::new(Utc.with_ymd_and_hms(2026, 8, 1, 12, 0, 0).unwrap());
        let mut manager = Manager::new(
            "manager-1".to_string(),
            "Test".to_string(),
            "Manager".to_string(),
            "1980-01-01".to_string(),
            "England".to_string(),
        );
        manager.hire("team-1".to_string());

        Game::new(
            clock,
            manager,
            vec![make_team()],
            vec![],
            vec![make_employed_staff()],
            vec![],
        )
    }

    #[test]
    fn hire_staff_internal_updates_state() {
        let state = StateManager::new();
        state.set_game(make_game());

        let response = hire_staff_internal(&state, "staff-1").expect("response");
        let staff = response
            .staff
            .iter()
            .find(|staff| staff.id == "staff-1")
            .unwrap();
        let team = response
            .teams
            .iter()
            .find(|team| team.id == "team-1")
            .unwrap();

        assert_eq!(staff.team_id.as_deref(), Some("team-1"));
        assert_eq!(team.season_expenses, 0);

        let stored_game = state.get_game(|game| game.clone()).expect("stored game");
        let stored_staff = stored_game
            .staff
            .iter()
            .find(|staff| staff.id == "staff-1")
            .expect("stored staff should exist");
        let stored_team = stored_game
            .teams
            .iter()
            .find(|team| team.id == "team-1")
            .expect("stored team should exist");
        assert_eq!(stored_staff.team_id.as_deref(), Some("team-1"));
        assert_eq!(stored_team.season_expenses, 0);
    }

    #[test]
    fn release_staff_internal_updates_state() {
        let state = StateManager::new();
        state.set_game(make_game_with_employed_staff());

        let before_finance = state
            .get_game(|g| g.clone())
            .unwrap()
            .teams
            .iter()
            .find(|t| t.id == "team-1")
            .unwrap()
            .finance;

        let response = release_staff_internal(&state, "staff-1").expect("response");
        let staff = response
            .staff
            .iter()
            .find(|staff| staff.id == "staff-1")
            .unwrap();

        assert!(staff.team_id.is_none());
        assert_eq!(staff.wage, 0);
        let team = response.teams.iter().find(|t| t.id == "team-1").unwrap();
        assert!(team.season_expenses > 0);
        assert_eq!(team.finance, before_finance - team.season_expenses);

        let stored_game = state.get_game(|game| game.clone()).expect("stored game");
        let stored_staff = stored_game
            .staff
            .iter()
            .find(|staff| staff.id == "staff-1")
            .expect("stored staff should exist");
        assert!(stored_staff.team_id.is_none());
    }
}

#[tauri::command]
pub fn release_staff(state: State<'_, StateManager>, staff_id: String) -> Result<Game, String> {
    release_staff_internal(&state, &staff_id)
}

fn release_staff_internal(state: &StateManager, staff_id: &str) -> Result<Game, String> {
    info!("[cmd] release_staff: staff_id={}", staff_id);
    let mut game = state
        .get_game(|g| g.clone())
        .ok_or("No active game session".to_string())?;

    let team_id = game
        .manager
        .team_id
        .clone()
        .ok_or("No team assigned".to_string())?;

    let current_date = game.clock.current_date.date_naive();
    let severance = {
        let staff_ref = game
            .staff
            .iter()
            .find(|s| s.id == staff_id)
            .ok_or("Staff member not found".to_string())?;
        if staff_ref.team_id.as_deref() != Some(&team_id) {
            return Err("Staff member does not belong to your team".to_string());
        }
        severance_for_staff_release(staff_ref, current_date)
    };

    remove_scouting_assignments_for_scout(&mut game, staff_id);

    if severance > 0 {
        if let Some(team) = game.teams.iter_mut().find(|t| t.id == team_id) {
            team.finance -= severance;
            team.season_expenses += severance;
        }
    }

    let staff = game
        .staff
        .iter_mut()
        .find(|s| s.id == staff_id)
        .ok_or_else(|| "Staff member not found".to_string())?;
    reset_staff_to_free_agent(staff);

    state.set_game(game.clone());
    Ok(game)
}
