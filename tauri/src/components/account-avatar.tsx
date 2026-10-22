import { memo, useEffect, useState } from "react"
import { BotAvatar } from "bot-avatars"
import { accountAvatar, type AvatarIdentity } from "../lib/hud"

export const AccountAvatar = memo(function AccountAvatar({ id, identity, size = 48, working = false, animate = false }: { id: string; identity?: AvatarIdentity; size?: number; working?: boolean; animate?: boolean }) {
  const [assigned, setAssigned] = useState(identity)
  useEffect(() => {
    let active = true
    if (identity) setAssigned(identity)
    else if (crypto.subtle) void accountAvatar(id).then(value => { if (active) setAssigned(value) })
    return () => { active = false }
  }, [id, identity])
  if (!assigned) return <span className="account-avatar-placeholder" style={{ width: size, height: size }} aria-hidden="true" />
  // Accessories come from the existing permanent seed; account identities stay intact.
  const look = assigned.seed % 6
  return <BotAvatar face="mouth" hat={look === 0 ? "beanie" : look === 1 ? "beret" : look === 2 ? "crown" : "none"}
    glasses={look === 3 ? "round" : look === 4 ? "shades" : "none"} headphones={look === 5} bowTie={look === 1 || look === 3}
    accessoryColor={look % 2 === 0 ? "#394b68" : "#b8564c"} brightness={1.12} lightFront={42} rim={0.8}
    type={assigned.kind} color={assigned.color} seed={assigned.seed / 0xffffffff} size={size}
    state={working ? "working" : "default"} paused={!animate || !working} interactive={false}
    shading="fabric" saturation={0.95} speed={0.35} turn={0.15} jumpHeight={4} jumpSpin={0} jumpEvery={0}
    furDensity={1.25} furFuzz={0.7} aria-hidden="true" />
}, (a, b) => a.id === b.id && a.size === b.size && a.working === b.working && a.animate === b.animate
  && a.identity?.kind === b.identity?.kind && a.identity?.color === b.identity?.color && a.identity?.seed === b.identity?.seed)
