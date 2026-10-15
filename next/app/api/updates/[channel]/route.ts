import { createUpdateHandler } from "../../../../lib/update-handler"

export async function GET(_request: Request, { params }: { params: Promise<{ channel: string }> }) {
  return createUpdateHandler()((await params).channel)
}
