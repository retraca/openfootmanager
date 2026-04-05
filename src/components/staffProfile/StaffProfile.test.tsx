import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { GameStateData, StaffData, TeamData } from "../../store/gameStore";
import StaffProfile from "./StaffProfile";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("../../services/staffService", () => ({
  releaseStaff: vi.fn(),
}));

vi.mock("../playerProfile/PlayerProfileRenewalModal", () => ({
  default: () => <div data-testid="renewal-modal-mock">Renewal modal</div>,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, string>) => {
      if (key === "finances.perWeekSuffix") return "/wk";
      if (key === "finances.contractExpiresOn") return `Expires ${options?.date ?? ""}`;
      if (key === "playerProfile.contractInfo") return "Contract";
      if (key === "playerProfile.noContract") return "No contract";
      if (key === "playerProfile.contractRisk") return "Contract risk";
      if (key === "playerProfile.weeklyWage") return "Weekly wage";
      if (key === "common.back") return "Back";
      if (key === "staff.roles.Coach") return "Coach";
      if (key === "common.age") return "Age";
      if (key.startsWith("finances.contractRisk")) return key;
      return key;
    },
    i18n: { language: "en" },
  }),
}));

function createTeam(overrides: Partial<TeamData> = {}): TeamData {
  return {
    id: "team-1",
    name: "Alpha FC",
    short_name: "ALP",
    country: "GB",
    city: "London",
    stadium_name: "Alpha Ground",
    stadium_capacity: 30000,
    finance: 500000,
    manager_id: "manager-1",
    reputation: 50,
    wage_budget: 50000,
    transfer_budget: 250000,
    season_income: 0,
    season_expenses: 0,
    formation: "4-4-2",
    play_style: "Balanced",
    training_focus: "General",
    training_intensity: "Balanced",
    training_schedule: "Balanced",
    founded_year: 1900,
    colors: { primary: "#000000", secondary: "#ffffff" },
    starting_xi_ids: [],
    form: [],
    history: [],
    ...overrides,
  };
}

function createStaff(overrides: Partial<StaffData> = {}): StaffData {
  return {
    id: "staff-1",
    first_name: "Alex",
    last_name: "Coach",
    date_of_birth: "1980-01-01",
    nationality: "GB",
    role: "Coach",
    attributes: {
      coaching: 70,
      judging_ability: 50,
      judging_potential: 55,
      physiotherapy: 30,
    },
    team_id: "team-1",
    specialization: null,
    wage: 104_000,
    contract_end: "2027-06-30",
    ...overrides,
  };
}

function createGameState(staff: StaffData): GameStateData {
  return {
    clock: {
      current_date: "2026-08-10T12:00:00Z",
      start_date: "2026-07-01T12:00:00Z",
    },
    manager: {
      id: "manager-1",
      first_name: "Jane",
      last_name: "Doe",
      date_of_birth: "1980-01-01",
      nationality: "GB",
      reputation: 50,
      satisfaction: 50,
      fan_approval: 50,
      team_id: "team-1",
      career_stats: {
        matches_managed: 0,
        wins: 0,
        draws: 0,
        losses: 0,
        trophies: 0,
        best_finish: null,
      },
      career_history: [],
    },
    teams: [createTeam()],
    players: [],
    staff: [staff],
    messages: [],
    news: [],
    league: null,
    scouting_assignments: [],
    board_objectives: [],
  };
}

describe("StaffProfile", () => {
  it("renders staff name, role, and contract section", () => {
    const onClose = vi.fn();
    render(
      <StaffProfile
        staff={createStaff()}
        gameState={createGameState(createStaff())}
        isOwnClub
        onClose={onClose}
      />,
    );

    expect(screen.getByRole("heading", { name: "Alex Coach" })).toBeInTheDocument();
    expect(screen.getByText(/Coach.*Age 46/)).toBeInTheDocument();
    expect(screen.getByText("Contract")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(onClose).toHaveBeenCalledOnce();
  });
});
