import { create } from 'zustand';
import type { GameStateData } from './types';

// Re-export all types so existing imports from gameStore keep working
export type {
  TeamColors,
  TeamSeasonRecord,
  TeamMatchRolesData,
  TeamData,
  PlayerSeasonStats,
  CareerEntry,
  PlayerData,
  TransferOfferData,
  StaffContractRenewalStateData,
  StaffData,
  StaffMoraleCoreData,
  MessageAction,
  MessageActionOption,
  MessageContext,
  DelegatedRenewalCaseMessageData,
  DelegatedRenewalReportMessageData,
  PlayerSelectionOptions,
  ScoutReportData,
  MessageData,
  ManagerCareerStats,
  ManagerCareerEntry,
  FixtureData,
  StandingData,
  LeagueData,
  SeasonPhase,
  TransferWindowStatus,
  TransferWindowContextData,
  SeasonContextData,
  NewsMatchScore,
  NewsArticle,
  BoardObjective,
  ScoutingAssignment,
  GameStateData,
} from './types';

interface GameStore {
  hasActiveGame: boolean;
  managerName: string | null;
  gameState: GameStateData | null;
  isDirty: boolean;
  setGameActive: (active: boolean, managerName?: string) => void;
  setGameState: (state: GameStateData) => void;
  markClean: () => void;
  clearGame: () => void;
}

export const useGameStore = create<GameStore>((set) => ({
  hasActiveGame: false,
  managerName: null,
  gameState: null,
  isDirty: false,
  setGameActive: (active, managerName) => set({ 
    hasActiveGame: active, 
    managerName: managerName || null 
  }),
  setGameState: (state) => set({
    gameState: state,
    isDirty: true,
  }),
  markClean: () => set({ isDirty: false }),
  clearGame: () => set({
    hasActiveGame: false,
    managerName: null,
    gameState: null,
    isDirty: false,
  }),
}));
