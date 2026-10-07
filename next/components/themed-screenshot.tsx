import Image, { type StaticImageData } from "next/image"

export function ThemedScreenshot({ light, dark, alt, sizes, priority = false }: {
  light: StaticImageData
  dark: StaticImageData
  alt: string
  sizes: string
  priority?: boolean
}) {
  return <>
    <Image className="screenshot-light" src={light} alt={alt} sizes={sizes} fetchPriority={priority ? "high" : "auto"} />
    <Image className="screenshot-dark" src={dark} alt={alt} sizes={sizes} fetchPriority={priority ? "high" : "auto"} />
  </>
}
