import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft } from "lucide-react";
import { useTranslation } from "react-i18next";
import { calcAge, getContractRiskLevel } from "../../lib/helpers";
import { formatPlayerWage } from "../playerProfile/PlayerProfile.helpers";
import PlayerProfileRenewalModal from "../playerProfile/PlayerProfileRenewalModal";
import {
  getRenewalStatusClassName,
  getRenewalStatusMessage,
  type NegotiationFeedbackData,
  type RenewalProjectionData,
  type RenewalResponseData,
  type RenewalStatus,
  shouldDisableRenewalSubmit,
} from "../playerProfile/PlayerProfile.renewal";
import { Badge, Button, Card, CardBody, CardHeader } from "../ui";
import type { GameStateData, StaffData } from "../../store/gameStore";
import { releaseStaff } from "../../services/staffService";
import { countryName } from "../../lib/countries";

function staffOvr(s: StaffData): number {
  return Math.round(
    (s.attributes.coaching +
      s.attributes.judging_ability +
      s.attributes.judging_potential +
      s.attributes.physiotherapy) /
      4,
  );
}

interface StaffProfileProps {
  staff: StaffData;
  gameState: GameStateData;
  isOwnClub: boolean;
  startWithRenewalModal?: boolean;
  onClose: () => void;
  onGameUpdate?: (g: GameStateData) => void;
}

export default function StaffProfile({
  staff,
  gameState,
  isOwnClub,
  startWithRenewalModal = false,
  onClose,
  onGameUpdate,
}: StaffProfileProps): JSX.Element | null {
  const { t, i18n } = useTranslation();
  const weeklySuffix = t("finances.perWeekSuffix", "/wk");
  const [showRenewalModal, setShowRenewalModal] = useState(false);
  const [renewalWage, setRenewalWage] = useState("");
  const [renewalLength, setRenewalLength] = useState("2");
  const [renewalSubmitting, setRenewalSubmitting] = useState(false);
  const [renewalStatus, setRenewalStatus] = useState<RenewalStatus>("idle");
  const [renewalError, setRenewalError] = useState<string | null>(null);
  const [renewalSuggestedWage, setRenewalSuggestedWage] = useState<number | null>(
    null,
  );
  const [renewalSuggestedYears, setRenewalSuggestedYears] = useState<number | null>(
    null,
  );
  const [renewalSessionStatus, setRenewalSessionStatus] =
    useState<RenewalResponseData["session_status"]>("idle");
  const [renewalIsTerminal, setRenewalIsTerminal] = useState(false);
  const [renewalCooledOff, setRenewalCooledOff] = useState(false);
  const [renewalFeedback, setRenewalFeedback] =
    useState<NegotiationFeedbackData | null>(null);
  const [renewalProjection, setRenewalProjection] =
    useState<RenewalProjectionData["projection"] | null>(null);
  const [hasConsumedInitialRenewalIntent, setHasConsumedInitialRenewalIntent] =
    useState(false);
  const [releaseLoading, setReleaseLoading] = useState(false);

  const ovr = staffOvr(staff);
  const age = calcAge(staff.date_of_birth);
  const contractRiskLevel = getContractRiskLevel(
    staff.contract_end,
    gameState.clock.current_date,
  );
  const contractRiskLabel =
    contractRiskLevel === "critical"
      ? t("finances.contractRiskCritical")
      : contractRiskLevel === "warning"
        ? t("finances.contractRiskWarning")
        : t("finances.contractRiskStable");

  const renewalOfferedWage = Number(renewalWage);
  const renewalOfferedYears = Number(renewalLength);
  const isRenewalWageValid =
    Number.isFinite(renewalOfferedWage) && renewalOfferedWage > 0;
  const isRenewalLengthValid =
    Number.isInteger(renewalOfferedYears) && renewalOfferedYears > 0;
  const renewalViolatesSoftCap =
    isRenewalWageValid &&
    renewalProjection !== null &&
    !renewalProjection.policy_allows;
  const renewalSubmitDisabled = shouldDisableRenewalSubmit({
    renewalSubmitting,
    renewalIsTerminal,
    isRenewalWageValid,
    isRenewalLengthValid,
    renewalViolatesSoftCap,
  });
  const renewalStatusMessage = getRenewalStatusMessage(
    {
      renewalSessionStatus,
      renewalStatus,
      renewalSuggestedWage,
      renewalSuggestedYears,
      renewalError,
    },
    t,
  );
  const renewalStatusClassName = getRenewalStatusClassName(renewalStatus);

  function openRenewalModal(): void {
    setRenewalWage(String(staff.wage));
    setRenewalLength("2");
    setRenewalSubmitting(false);
    setRenewalStatus("idle");
    setRenewalError(null);
    setRenewalSuggestedWage(null);
    setRenewalSuggestedYears(null);
    setRenewalSessionStatus("idle");
    setRenewalIsTerminal(false);
    setRenewalCooledOff(false);
    setRenewalFeedback(null);
    setRenewalProjection(null);
    setShowRenewalModal(true);
  }

  function closeRenewalModal(): void {
    if (renewalSubmitting) return;
    setShowRenewalModal(false);
  }

  useEffect(() => {
    setHasConsumedInitialRenewalIntent(false);
  }, [staff.id, startWithRenewalModal]);

  useEffect(() => {
    if (
      !isOwnClub ||
      !startWithRenewalModal ||
      showRenewalModal ||
      hasConsumedInitialRenewalIntent
    ) {
      return;
    }
    setHasConsumedInitialRenewalIntent(true);
    openRenewalModal();
  }, [
    hasConsumedInitialRenewalIntent,
    isOwnClub,
    showRenewalModal,
    startWithRenewalModal,
  ]);

  useEffect(() => {
    if (!showRenewalModal || !isRenewalWageValid) {
      setRenewalProjection(null);
      return;
    }
    let cancelled = false;
    const loadProjection = async (): Promise<void> => {
      try {
        const result = await invoke<RenewalProjectionData>(
          "preview_staff_renewal_financial_impact",
          {
            staffId: staff.id,
            weeklyWage: renewalOfferedWage,
          },
        );
        if (!cancelled) {
          setRenewalProjection(result.projection ?? null);
        }
      } catch {
        if (!cancelled) setRenewalProjection(null);
      }
    };
    loadProjection();
    return () => {
      cancelled = true;
    };
  }, [isRenewalWageValid, staff.id, renewalOfferedWage, showRenewalModal]);

  async function handleRenewalSubmit(): Promise<void> {
    if (renewalSubmitDisabled) return;
    setRenewalSubmitting(true);
    setRenewalStatus("idle");
    setRenewalError(null);
    setRenewalCooledOff(false);
    try {
      const result = await invoke<RenewalResponseData>("propose_staff_renewal", {
        staffId: staff.id,
        weeklyWage: renewalOfferedWage,
        contractYears: renewalOfferedYears,
      });
      onGameUpdate?.(result.game);
      setRenewalStatus(result.outcome);
      setRenewalSuggestedWage(result.suggested_wage);
      setRenewalSuggestedYears(result.suggested_years);
      setRenewalSessionStatus(result.session_status);
      setRenewalIsTerminal(result.is_terminal);
      setRenewalCooledOff(result.cooled_off ?? false);
      setRenewalFeedback(result.feedback ?? null);
      if (result.session_status === "blocked") {
        setRenewalStatus("blocked");
      }
      if (result.outcome === "counter_offer") {
        if (result.suggested_wage !== null) {
          setRenewalWage(String(result.suggested_wage));
        }
        if (result.suggested_years !== null) {
          setRenewalLength(String(result.suggested_years));
        }
      }
    } catch (error) {
      setRenewalStatus("error");
      setRenewalError(String(error));
      setRenewalCooledOff(false);
    } finally {
      setRenewalSubmitting(false);
    }
  }

  async function handleDelegateRenewal(): Promise<void> {
    if (renewalSubmitting) return;
    setRenewalSubmitting(true);
    try {
      const result = await invoke<{ game: GameStateData }>("delegate_renewals", {
        playerIds: [],
        staffIds: [staff.id],
        maxWageIncreasePct: 15,
        maxContractYears: 3,
      });
      onGameUpdate?.(result.game);
      closeRenewalModal();
    } catch {
      /* ignore */
    } finally {
      setRenewalSubmitting(false);
    }
  }

  async function handleRelease(): Promise<void> {
    if (!isOwnClub || releaseLoading) return;
    setReleaseLoading(true);
    try {
      const updated = await releaseStaff(staff.id);
      onGameUpdate?.(updated);
      onClose();
    } catch (e) {
      console.error(e);
    } finally {
      setReleaseLoading(false);
    }
  }

  const displayName = `${staff.first_name} ${staff.last_name}`;

  return (
    <div className="max-w-4xl mx-auto space-y-5">
      <div className="flex flex-wrap items-center gap-3">
        <button
          type="button"
          onClick={onClose}
          className="inline-flex items-center gap-2 text-sm font-heading font-bold uppercase tracking-wider text-primary-600 dark:text-primary-400 hover:opacity-80"
        >
          <ArrowLeft className="w-4 h-4" />
          {t("common.back")}
        </button>
      </div>

      <Card accent="accent">
        <CardBody>
          <div className="flex flex-wrap items-start justify-between gap-4">
            <div>
              <h1 className="font-heading font-bold text-xl uppercase tracking-wide text-gray-900 dark:text-gray-100">
                {displayName}
              </h1>
              <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                {t(`staff.roles.${staff.role}`)} · {t("common.age")} {age} ·{" "}
                {countryName(staff.nationality, i18n.language)}
              </p>
            </div>
            <Badge variant="primary" size="md">
              {ovr} OVR
            </Badge>
          </div>
        </CardBody>
      </Card>

      <Card>
        <CardHeader>{t("playerProfile.contractInfo")}</CardHeader>
        <CardBody>
          <div className="flex flex-col gap-3 text-sm">
            <div className="flex justify-between gap-4">
              <span className="text-gray-500 dark:text-gray-400">
                {t("common.contract")}
              </span>
              <span>
                {staff.contract_end
                  ? t("finances.contractExpiresOn", { date: staff.contract_end })
                  : t("playerProfile.noContract")}
              </span>
            </div>
            <div className="flex justify-between gap-4">
              <span className="text-gray-500 dark:text-gray-400">
                {t("playerProfile.contractRisk")}
              </span>
              <Badge variant={contractRiskLevel === "critical" ? "danger" : contractRiskLevel === "warning" ? "accent" : "neutral"}>
                {contractRiskLabel}
              </Badge>
            </div>
            <div className="flex justify-between gap-4">
              <span className="text-gray-500 dark:text-gray-400">
                {t("playerProfile.weeklyWage")}
              </span>
              <span>{formatPlayerWage(staff.wage, weeklySuffix)}</span>
            </div>
            {typeof staff.morale === "number" && (
              <div className="flex justify-between gap-4">
                <span className="text-gray-500 dark:text-gray-400">
                  {t("common.morale")}
                </span>
                <span>{staff.morale}</span>
              </div>
            )}
            {isOwnClub && staff.team_id && (
              <div className="flex flex-wrap gap-2 pt-2">
                <Button variant="primary" size="sm" onClick={openRenewalModal}>
                  {t("common.renewContract")}
                </Button>
                <Button
                  variant="danger"
                  size="sm"
                  disabled={releaseLoading}
                  onClick={() => void handleRelease()}
                >
                  {t("staff.releaseStaff")}
                </Button>
              </div>
            )}
          </div>
        </CardBody>
      </Card>

      <Card>
        <CardHeader>{t("dashboard.staff")}</CardHeader>
        <CardBody>
          <div className="grid grid-cols-2 gap-3 text-sm">
            {(
              [
                ["coaching", staff.attributes.coaching],
                ["judgingAbility", staff.attributes.judging_ability],
                ["judgingPotential", staff.attributes.judging_potential],
                ["physiotherapy", staff.attributes.physiotherapy],
              ] as const
            ).map(([key, value]) => (
              <div key={key} className="flex justify-between gap-2">
                <span className="text-gray-500 dark:text-gray-400">
                  {t(`staff.attrs.${key}`)}
                </span>
                <span className="font-heading font-bold tabular-nums">{value}</span>
              </div>
            ))}
          </div>
        </CardBody>
      </Card>

      <PlayerProfileRenewalModal
        show={showRenewalModal}
        playerName={displayName}
        t={t}
        weeklySuffix={weeklySuffix}
        renewalWage={renewalWage}
        renewalLength={renewalLength}
        renewalIsTerminal={renewalIsTerminal}
        isRenewalWageValid={isRenewalWageValid}
        renewalViolatesSoftCap={renewalViolatesSoftCap}
        renewalProjection={renewalProjection}
        renewalStatusMessage={renewalStatusMessage}
        renewalStatusClassName={renewalStatusClassName}
        renewalCooledOff={renewalCooledOff}
        renewalFeedback={renewalFeedback}
        renewalSubmitting={renewalSubmitting}
        renewalSubmitDisabled={renewalSubmitDisabled}
        onWageChange={setRenewalWage}
        onLengthChange={setRenewalLength}
        onClose={closeRenewalModal}
        onDelegate={() => void handleDelegateRenewal()}
        onSubmit={() => void handleRenewalSubmit()}
      />
    </div>
  );
}
