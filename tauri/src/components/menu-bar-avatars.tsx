import { invoke } from "@tauri-apps/api/core"
import { useEffect, useRef, useState } from "react"
import { useAppPreferences } from "../lib/app-preferences"
import { activeFestival, useSeasonalDay } from "../lib/seasonal-avatars"
import { AccountAvatar } from "./account-avatar"

const size = 64
const menuBarHeight = 48
const padding = 2

function fitMenuBar(image: HTMLCanvasElement, context: CanvasRenderingContext2D) {
  const { data } = context.getImageData(0, 0, size, size)
  let left = size, top = size, right = 0, bottom = 0
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      if (data[(y * size + x) * 4 + 3] < 32) continue
      left = Math.min(left, x)
      top = Math.min(top, y)
      right = Math.max(right, x + 1)
      bottom = Math.max(bottom, y + 1)
    }
  }
  if (right <= left || bottom <= top) return

  const width = right - left
  const height = bottom - top
  // The native tray fits the image to a fixed height and preserves its aspect ratio.
  const scale = Math.min((menuBarHeight - 2 * padding) / height, 64 / width)
  const output = document.createElement("canvas")
  output.width = Math.ceil(width * scale) + 2 * padding
  output.height = menuBarHeight
  const outputContext = output.getContext("2d")
  if (!outputContext) return
  outputContext.imageSmoothingQuality = "high"
  outputContext.drawImage(image, left, top, width, height,
    (output.width - width * scale) / 2, (output.height - height * scale) / 2, width * scale, height * scale)
  return output.toDataURL("image/png").split(",")[1]
}

async function capture(container: HTMLElement) {
  const source = container.querySelector("canvas")
  if (!source || source.style.opacity !== "1") return
  const image = document.createElement("canvas")
  image.width = image.height = size
  const context = image.getContext("2d")
  if (!context) return

  const { left, top } = container.getBoundingClientRect()
  const bounds = source.getBoundingClientRect()
  context.drawImage(source, bounds.left - left, bounds.top - top, bounds.width, bounds.height)

  const costume = container.querySelector("svg")
  if (costume) {
    const overlaySvg = costume.cloneNode(true) as SVGElement
    overlaySvg.setAttribute("width", String(size))
    overlaySvg.setAttribute("height", String(size))
    const svg = new XMLSerializer().serializeToString(overlaySvg)
    const url = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }))
    try {
      const overlay = new Image()
      overlay.src = url
      await overlay.decode()
      context.drawImage(overlay, 0, 0, size, size)
    } finally {
      URL.revokeObjectURL(url)
    }
  }

  return fitMenuBar(image, context)
}

export function MenuBarAvatars({ ids }: { ids: string[] }) {
  const { seasonalAvatars } = useAppPreferences()
  useSeasonalDay()
  const festival = seasonalAvatars ? activeFestival(new Date()) : null
  const signature = JSON.stringify([...ids].sort())
  return <AvatarSnapshot key={JSON.stringify([signature, festival])} signature={signature} />
}

function AvatarSnapshot({ signature }: { signature: string }) {
  const host = useRef<HTMLDivElement>(null)
  const [captured, setCaptured] = useState(false)
  const [ids] = useState<string[]>(() => JSON.parse(signature))

  useEffect(() => {
    let stopped = false
    let timeout: number | undefined
    let attempts = 0
    const publish = async () => {
      if (!host.current || stopped) return
      const frames = await Promise.all(Array.from(host.current.children, node => capture(node as HTMLElement)))
      if (stopped) return
      if (frames.some(frame => !frame) && ++attempts < 50) {
        timeout = window.setTimeout(() => void publish().catch(error => console.error("Could not publish menu bar avatars", error)), 100)
        return
      }
      const icons = Object.fromEntries(ids.flatMap((id, index) => frames[index] ? [[id, frames[index]]] : []))
      await invoke("save_menu_bar_avatars", { icons })
      if (!stopped) setCaptured(true)
    }
    void publish().catch(error => console.error("Could not publish menu bar avatars", error))
    return () => {
      stopped = true
      window.clearTimeout(timeout)
    }
  }, [ids])

  if (captured) return null
  return <div ref={host} aria-hidden="true" style={{ position: "fixed", left: -size * 2, top: 0, opacity: 0, pointerEvents: "none" }}>
    {ids.map(id => <div key={id} style={{ width: size, height: size }}>
      <AccountAvatar id={id} size={size} />
    </div>)}
  </div>
}
