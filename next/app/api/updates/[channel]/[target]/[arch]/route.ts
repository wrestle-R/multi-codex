import { createUpdateHandler } from "../../../../../../lib/update-handler"

export async function GET(_request: Request, { params }: { params: Promise<{ channel: string; target: string; arch: string }> }) {
  const { channel, target, arch } = await params
  return createUpdateHandler()(channel, `${target}-${arch}`)
}
