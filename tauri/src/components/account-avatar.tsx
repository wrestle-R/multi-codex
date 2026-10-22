import { useEffect, useState } from "react"
import { BotAvatar } from "bot-avatars"
import { accountAvatar, type AvatarIdentity } from "../lib/hud"

export function AccountAvatar({ id, identity, size = 48, working = false, animate = false }: { id: string; identity?: AvatarIdentity; size?: number; working?: boolean; animate?: boolean }) {
  const [assigned, setAssigned] = useState(identity)
  useEffect(() => {
    let active = true
    if (identity) setAssigned(identity)
    else if (crypto.subtle) void accountAvatar(id).then(value => { if (active) setAssigned(value) })
    return () => { active = false }
  }, [id, identity])
  if (!assigned) return <span className="account-avatar-placeholder" style={{ width: size, height: size }} aria-hidden="true" />
  return <BotAvatar type={assigned.kind} color={assigned.color} seed={assigned.seed / 0xffffffff} size={size}
    state={working ? "working" : "default"} paused={!animate || !working} interactive={false}
    shading="fabric" saturation={0.95} speed={0.35} turn={0.15} jumpHeight={4} jumpSpin={0} jumpEvery={0}
    furDensity={1.25} furFuzz={0.7} aria-hidden="true" />
}
