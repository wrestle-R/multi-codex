import React from "react"
import ReactDOM from "react-dom/client"
import "@fontsource/bricolage-grotesque/latin-500.css"
import "@fontsource/bricolage-grotesque/latin-600.css"
import "@fontsource/bricolage-grotesque/latin-700.css"
import "@fontsource/outfit/latin-400.css"
import "@fontsource/outfit/latin-500.css"
import "@fontsource/outfit/latin-600.css"
import App from "./App"
import Hud from "./Hud"
import MenuBar from "./MenuBar"

const view = new URLSearchParams(window.location.search).get("view")

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {view === "hud" ? <Hud /> : view === "menu-bar" ? <MenuBar /> : <App />}
  </React.StrictMode>,
)

