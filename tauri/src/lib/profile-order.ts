import type { Profile } from "./types"

// Known individual plans follow their price order. Other or unknown tiers keep
// their existing relative order above Free, without guessing a custom price.
const planOrder: Record<string, number> = { pro: 4, plus: 3, go: 2, free: 0 }
export function orderProfilesByPlan(profiles: Profile[]): Profile[] {
  return [...profiles].sort((left, right) =>
    (planOrder[right.accountTier?.trim().toLowerCase() ?? ""] ?? 1)
    - (planOrder[left.accountTier?.trim().toLowerCase() ?? ""] ?? 1),
  )
}
