import type { NextConfig } from "next"
import { withBotId } from "botid/next/config"
const config: NextConfig = { poweredByHeader: false }
export default withBotId(config)
