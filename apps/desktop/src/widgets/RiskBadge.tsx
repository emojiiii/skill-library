import { riskLabel, riskTone } from "../utils/risk";
import { Pill } from "./Pill";

export function RiskBadge({ level }: { level?: string | null }) {
  const value = level ?? "low";
  return (
    <Pill tone={(riskTone[value] ?? "default") as never}>
      {riskLabel[value] ?? value}
    </Pill>
  );
}
