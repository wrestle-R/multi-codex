import type { FestivalId } from "../lib/seasonal-avatars"

// Vector clothing stays crisp at Orbiter sizes and adds no image downloads.
export function SeasonalCostume({ festival, variant }: { festival: FestivalId; variant: number }) {
  const v = variant % 3
  const alternate = variant >= 3
  const gold = "#edc76c", cream = "#fff0cf"
  const sparkle = <g fill={gold}><path d="m16 42 2-5 2 5 5 2-5 2-2 5-2-5-5-2Z" /><path d="m82 37 1.5-4 1.5 4 4 1.5-4 1.5-1.5 4-1.5-4-4-1.5Z" /><circle cx="84" cy="63" r="1.7" /></g>
  const scarf = (color: string) => <g><path d="M28 74Q50 83 72 74L70 84Q50 92 30 84Z" fill={color} /><path d="m59 80 10 2-3 18-12-3Z" fill={color} /><path d="m57 94 9 3M32 80q18 8 36 0" fill="none" stroke={cream} strokeWidth="1.5" /></g>
  const vest = (color: string) => <g><path d="M29 73 40 77 50 83 60 77 71 73 76 94Q50 103 24 94Z" fill={color} stroke={gold} strokeWidth="1.4" /><path d="m40 77 10 10 10-10M50 87v11M30 85l8 8M70 85l-8 8" fill="none" stroke={gold} strokeWidth="1.5" />{[87,92,97].map(y => <circle key={y} cx="50" cy={y} r="1" fill={gold} />)}</g>
  let clothing
  if (festival === "halloween") clothing = v === 0 ? <>
    <path d="m28 73-10 23q12-8 22 1l10-13 10 13q10-9 22-1L72 73l-22 7Z" fill="#332045" stroke="#8e6baa" strokeWidth="1.2" />
    <path d="m29 31 13-26 7 9 7-7 15 26Z" fill="#352041" stroke="#b19ac7" strokeWidth="1.2" /><path d="m33 25 33 2 3 6-39-1Z" fill="#dc7e32" /><rect x="47" y="25" width="9" height="6" rx="1" fill="none" stroke={gold} strokeWidth="2" /><ellipse cx="50" cy="33" rx="32" ry="5" fill="#493052" />{sparkle}
  </> : v === 1 ? <>
    <path d="M27 64 15 87l13 12 22-15 22 15 13-12-12-23-12 18H39Z" fill="#3a1728" stroke="#ca6674" strokeWidth="1.4" /><path d="m27 64 12 18 11 2-18-7ZM73 64 61 82l-11 2 18-7Z" fill="#be3d56" /><path d="m43 68 3 6 2-6m5 0 2 6 3-6" fill="#fff3e1" /><path d="m12 30 6 5 4-4 4 8-8-2-5 2Z" fill="#a576bf" /><circle cx="50" cy="84" r="3" fill={gold} />
  </> : <>
    <path d="m28 30 45 7-2 9-44-7Zm0 10 20 5-2 8-20-5ZM58 47l15 3-1 9-15-3ZM28 69l44 6-3 10-43-7Zm0 12 41 6-3 10-39-7Z" fill="#e9debf" stroke="#b2a58a" strokeWidth="1" /><path d="m31 34 33 6M31 74l33 6M32 86l30 6" stroke="#fff6dc" strokeWidth="2" /><path d="m69 84 12 6-2 7-14-6Z" fill="#e9debf" />
  </>
  else if (festival === "christmas") clothing = <>
    {scarf(v === 1 ? "#b23e3d" : "#28745a")}
    {v === 0 ? <><path d="M25 33Q35 1 60 12l16 14-16-3-1 12Z" fill="#c94545" stroke="#e99182" /><path d="M24 30Q50 24 70 31l-1 8q-22-5-46 0Z" fill={cream} /><circle cx="76" cy="26" r="6" fill={cream} /><path d="m36 18 7-3" stroke="#ef8b82" strokeWidth="2" /></> : v === 1 ? <g stroke="#ae7950" strokeWidth="4" strokeLinecap="round" fill="none"><path d="m33 34-7-20-7-5m9 12 6-9m34 22 7-20 7-5m-9 12-6-9" /><path d="m27 29-11-3 6 12m51-9 11-3-6 12" stroke="#ca9b70" /></g> : <><path d="M24 33 43 5l9 14 16 1-3 15Z" fill="#247757" stroke="#78b88d" /><path d="m24 33 8 9 8-9 8 9 8-9 9 9 4-10" fill="#d34f4a" /><circle cx="43" cy="7" r="4" fill={gold} /></>}
    {sparkle}
  </>
  else if (festival === "easter") clothing = <>
    <g transform={v === 1 ? "rotate(-8 50 40)" : undefined}><ellipse cx="37" cy="20" rx="7" ry="19" fill="#f5e8da" stroke="#c9b7ce" /><ellipse cx="63" cy="20" rx="7" ry="19" fill="#f5e8da" stroke="#c9b7ce" /><ellipse cx="37" cy="21" rx="3" ry="13" fill="#e7a6be" /><ellipse cx="63" cy="21" rx="3" ry="13" fill="#e7a6be" /></g>{scarf(["#b08bbd", "#7baf8a", "#dbac5d"][v])}<ellipse cx="81" cy="85" rx="7" ry="10" fill="#b3cfa1" stroke="#729977" /><path d="m75 82 12 5m-12-1 11 5" stroke="#f9e2a2" strokeWidth="2" />
  </>
  else if (festival === "holi") clothing = <>
    {scarf(["#b9437f", "#de7a3e", "#3c9eb2"][v])}<path d="M26 31q24-8 48 0l-1 8q-23-8-46 0Z" fill="#e7b350" stroke={cream} /><g opacity=".85"><path d="m25 49 8-3 5 5-7 5-8-1Z" fill="#e25c9b" /><path d="m65 45 10 5-2 8-11-3Z" fill="#36b2c3" /><circle cx="71" cy="66" r="4" fill="#e6bd42" /><circle cx="22" cy="64" r="3" fill="#ae69d3" /><circle cx="18" cy="43" r="2" fill="#ee8749" /></g>
  </>
  else if (festival === "diwali" || festival === "navratri" || festival === "onam" || festival === "pongal") {
    const ivory = festival === "onam" || festival === "pongal"
    clothing = <>
      {vest(ivory ? ["#eee1c2", "#c69b49", "#648568"][v] : ["#893759", "#246b69", "#46457f"][v])}
      <path d="M28 32Q32 14 50 16q18-2 23 16Z" fill={ivory ? "#e7d7b2" : ["#b84b6d", "#368480", "#645d9d"][v]} stroke={gold} strokeWidth="1.4" /><path d="m32 29 30-9m-25 12 29-10m-20 9 23-7" fill="none" stroke={gold} strokeWidth="1.1" />
      <circle cx="50" cy="28" r="3" fill={gold} /><circle cx="50" cy="28" r="1.4" fill="#e7f2d8" />
      {v === 1 ? <g fill="#e8af42">{[28,35,42,58,65,72].map((x,i) => <circle key={x} cx={x} cy={77 + Math.sin(i)*3} r="3" />)}</g> : <path d="m61 80 10 3-5 15-9-3Z" fill={gold} />}
      {festival === "diwali" ? <><path d="M75 86h18q-2 10-9 10t-9-10Z" fill="#b9673f" stroke={gold} /><path d="M84 86q-7-6 0-13 6 8 0 13Z" fill="#f4cd62" /><path d="m84 82 0-4" stroke={cream} /></> : sparkle}
    </>
  } else if (festival === "eid" || festival === "eid-adha") clothing = <>
    {vest(["#246b5d", "#404f82", "#78516c"][v])}<path d="M31 33v-9q19-10 38 0v9Z" fill={v === 1 ? cream : "#337c66"} stroke={gold} strokeWidth="1.3" /><path d="M31 29h38m-32-5 4 4 4-5 5 5 5-5 4 5 4-4" stroke={gold} fill="none" strokeWidth="1" /><path d="M82 35a8 8 0 1 1-6-14 8 8 0 0 0 6 14Z" fill={gold} /><path d="M82 68h8l3 13-7 5-7-5Z" fill="#bd9653" stroke={cream} /><path d="M86 62v6m-3 4v8m6-8v8" stroke={cream} />
  </>
  else if (festival === "lunar") clothing = <>
    {vest(["#a8313f", "#b2603d", "#2d7061"][v])}<path d="M29 32q2-18 21-18t21 18Z" fill="#a7373e" stroke={gold} /><path d="M27 31h46v6H27Z" fill="#542c31" stroke={gold} /><circle cx="50" cy="24" r="3" fill={gold} /><path d="M84 42v7m0 20v8" stroke={gold} /><ellipse cx="84" cy="59" rx="9" ry="11" fill="#bf4440" stroke={gold} /><path d="M84 49v20m-7-17h14m-14 14h14" stroke={gold} fill="none" />{sparkle}
  </>
  else if (festival === "valentine") clothing = <>
    {scarf(["#b74e6a", "#a9678b", "#d68183"][v])}<path d="M26 34q1-15 23-16 22 0 25 16Z" fill="#a84c6d" stroke="#e5a0bb" /><path d="M14 38c-9-8 1-14 4-7 4-7 13 0 4 7l-4 4Z" fill="#d87793" /><path d="M75 56c-7-6 1-11 3-6 3-5 10 0 3 6l-3 3Z" fill="#d87793" />
  </>
  else clothing = <>
    {vest(["#3b4d70", "#826540", "#67547a"][v])}<path d="m31 33 9-30 24 30Z" fill={["#586c92", "#c59a4b", "#9977a6"][v]} stroke={gold} /><path d="m37 19 14 3m-17 5 23 3" stroke={gold} strokeWidth="2" />{sparkle}<g stroke="#e5b8ac" strokeWidth="2"><path d="m18 65-4 5m67 17 5 3m-12-72 4-4" /></g>
  </>
  return <svg className="seasonal-costume" viewBox="0 0 100 100" aria-hidden="true">{clothing}{alternate && <g fill="none" stroke={gold} strokeWidth="1.5"><path d="M32 84q18 12 36 0M35 90q15 9 30 0" /><path d="m24 68 4-4 4 4-4 4ZM68 68l4-4 4 4-4 4Z" />{variant === 4 && <path d="m34 33 3-9 5 7 7-10 8 10 6-7 4 9Z" fill="#b76d45" />}{variant === 5 && <path d="M31 78q-10 6-10 16m48-16q10 6 10 16" strokeWidth="4" />}</g>}</svg>
}
