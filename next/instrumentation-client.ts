import { initBotId } from "botid/client/core"

initBotId({ protect: [{ path: "/api/issues", method: "POST" }] })
