import { memo, useEffect, useState } from "react"
import { BotAvatar } from "bot-avatars"
import { avatarTemplates, templateFor, useAvatarTemplate } from "../lib/avatar-templates"
import { activeFestival, useSeasonalDay } from "../lib/seasonal-avatars"
import { useAppPreferences } from "../lib/app-preferences"
import { SeasonalCostume } from "./seasonal-costume"
import { accountAvatar, type AvatarIdentity } from "../lib/hud"

export const AccountAvatar = memo(function AccountAvatar({ id, identity, size = 48, working = false, animate = false }: { id: string; identity?: AvatarIdentity; size?: number; working?: boolean; animate?: boolean }) {
  const templateIndex = useAvatarTemplate(id)
  const prefs = useAppPreferences()
  useSeasonalDay()
  const festival = prefs.seasonalAvatars ? activeFestival(new Date()) : null
  const [assigned, setAssigned] = useState(identity)
  useEffect(() => {
    let active = true
    if (identity) setAssigned(identity)
    else if (crypto.subtle) void accountAvatar(id).then(value => { if (active) setAssigned(value) })
    return () => { active = false }
  }, [id, identity])
  if (!assigned) return <span className="account-avatar-placeholder" style={{ width: size, height: size }} aria-hidden="true" />
  // Each colour has one consistent shape and wardrobe, shared by both windows.
  const look = templateIndex ?? templateFor(assigned)
  const template = avatarTemplates[look]
  return <span className="costumed-avatar" style={{ width: size, height: size }} aria-hidden="true"><BotAvatar face="mouth" hat={festival ? "none" : look === 0 ? "beanie" : look === 1 ? "beret" : look === 2 ? "crown" : "none"}
    glasses={festival ? "none" : look === 3 ? "round" : look === 4 ? "shades" : "none"} headphones={!festival && look === 5} bowTie={!festival && (look === 1 || look === 3)}
    accessoryColor={look % 2 === 0 ? "#394b68" : "#b8564c"} brightness={1.12} lightFront={42} rim={0.8}
    type={template.kind} color={template.color} seed={template.seed / 0xffffffff} size={size}
    state={working ? "working" : "default"} paused={!animate || !working} interactive={false}
    shading="fabric" saturation={0.95} speed={0.35} turn={0.15} jumpHeight={4} jumpSpin={0} jumpEvery={0}
    furDensity={1.25} furFuzz={0.7} aria-hidden="true" />{festival && <SeasonalCostume festival={festival} variant={look} />}</span>
}, (a, b) => a.id === b.id && a.size === b.size && a.working === b.working && a.animate === b.animate
  && a.identity?.kind === b.identity?.kind && a.identity?.color === b.identity?.color && a.identity?.seed === b.identity?.seed)
