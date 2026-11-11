/**
 * LeakStories: live news from Leak inside any Framer site.
 *
 * Paste into Framer (Assets → Code → + → New component), then drag it onto
 * the canvas. Every option is in the right-hand panel: which section, how many
 * stories, grid or list, fonts and colours, so it matches your page.
 *
 * Data comes from Leak's public JSON API and refreshes every minute.
 * Covers are Leak's own generated artwork, free to use.
 *
 * @framerSupportedLayoutWidth any-prefer-fixed
 * @framerSupportedLayoutHeight auto
 */
import { addPropertyControls, ControlType } from "framer"
import { useEffect, useState, type CSSProperties } from "react"

type Story = {
    id: string
    title: string
    dek: string
    path: string
    cover: string
    section: string
    section_name: string
    sources: number
    regions: number
    first_source: string
    created: string
    updated: string
    breaking: boolean
    developing: boolean
}

type Props = {
    site: string
    section: string
    count: number
    layout: "grid" | "list"
    columns: number
    gap: number
    showCovers: boolean
    showSummary: boolean
    coverRadius: number
    titleFont: CSSProperties
    bodyFont: CSSProperties
    textColor: string
    mutedColor: string
    breakingColor: string
    dividerColor: string
    refreshSeconds: number
    style?: CSSProperties
}

const SECTIONS = ["all", "world", "politics", "finance", "tech", "science", "health", "climate", "sports", "culture"]
const SECTION_TITLES = ["Top stories", "World", "Politics", "Finance", "Tech", "Science", "Health", "Climate", "Sports", "Culture"]

function timeAgo(iso: string): string {
    const s = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000)
    if (s < 45) return "just now"
    const m = Math.round(s / 60)
    if (m < 60) return `${m} min ago`
    const h = Math.floor(m / 60)
    if (h < 24) return `${h} h ago`
    const d = Math.floor(h / 24)
    return d === 1 ? "yesterday" : `${d} days ago`
}

function base(site: string): string {
    return site.endsWith("/") ? site : site + "/"
}

export default function LeakStories(props: Props) {
    const {
        site, section, count, layout, columns, gap, showCovers, showSummary, coverRadius,
        titleFont, bodyFont, textColor, mutedColor, breakingColor, dividerColor, refreshSeconds, style,
    } = props
    const [stories, setStories] = useState<Story[] | null>(null)
    const [failed, setFailed] = useState(false)
    const [, tick] = useState(0)

    useEffect(() => {
        let alive = true
        const root = base(site)
        const url = section === "all" ? `${root}api/latest.json` : `${root}api/sections/${section}.json`
        const load = () => {
            const bucket = Math.floor(Date.now() / 30000)
            fetch(`${url}?t=${bucket}`)
                .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
                .then((data) => {
                    if (!alive) return
                    setStories(data.stories || [])
                    setFailed(false)
                })
                .catch(() => alive && setFailed(true))
        }
        load()
        const every = Math.max(30, refreshSeconds) * 1000
        const id = setInterval(load, every)
        const clock = setInterval(() => tick((n) => n + 1), 30000)
        return () => {
            alive = false
            clearInterval(id)
            clearInterval(clock)
        }
    }, [site, section, refreshSeconds])

    const wrap: CSSProperties = { width: "100%", color: textColor, ...style }

    if (failed && !stories) {
        return (
            <div style={{ ...wrap, ...bodyFont, color: mutedColor, padding: 16 }}>
                Couldn't load stories. Check the Leak site address in the component settings.
            </div>
        )
    }

    const list = (stories ?? []).slice(0, count)
    const loading = stories === null
    const items = loading ? Array.from({ length: count }, () => null) : list

    const grid: CSSProperties =
        layout === "grid"
            ? { display: "grid", gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`, gap }
            : { display: "flex", flexDirection: "column", gap: 0 }

    return (
        <div style={wrap}>
            <div style={grid}>
                {items.map((s, i) => (
                    <Card
                        key={s ? s.id : i}
                        story={s}
                        root={base(site)}
                        layout={layout}
                        showCover={showCovers}
                        showSummary={showSummary}
                        coverRadius={coverRadius}
                        titleFont={titleFont}
                        bodyFont={bodyFont}
                        mutedColor={mutedColor}
                        breakingColor={breakingColor}
                        dividerColor={dividerColor}
                        gap={gap}
                    />
                ))}
            </div>
            {!loading && list.length === 0 && (
                <div style={{ ...bodyFont, color: mutedColor }}>No stories in this section yet.</div>
            )}
        </div>
    )
}

function Card(p: {
    story: Story | null
    root: string
    layout: "grid" | "list"
    showCover: boolean
    showSummary: boolean
    coverRadius: number
    titleFont: CSSProperties
    bodyFont: CSSProperties
    mutedColor: string
    breakingColor: string
    dividerColor: string
    gap: number
}) {
    const { story: s, root, layout, showCover, showSummary, coverRadius, titleFont, bodyFont, mutedColor, breakingColor, dividerColor, gap } = p
    const isList = layout === "list"
    const placeholder = "rgba(127,127,127,0.15)"

    const cover = showCover && (
        <div
            style={{
                flex: isList ? "0 0 32%" : undefined,
                aspectRatio: "16 / 10",
                borderRadius: coverRadius,
                overflow: "hidden",
                background: placeholder,
            }}
        >
            {s && (
                <img
                    src={root + s.cover}
                    alt=""
                    loading="lazy"
                    style={{ width: "100%", height: "100%", objectFit: "cover", display: "block" }}
                />
            )}
        </div>
    )

    const meta: CSSProperties = { ...bodyFont, color: mutedColor, fontSize: "0.8em", display: "flex", gap: 10, flexWrap: "wrap", alignItems: "center" }

    return (
        <a
            href={s ? root + s.path : undefined}
            target="_blank"
            rel="noopener"
            style={{
                display: "flex",
                flexDirection: isList ? "row" : "column",
                gap: isList ? gap : 12,
                textDecoration: "none",
                color: "inherit",
                padding: isList ? `${gap / 2}px 0` : 0,
                borderBottom: isList ? `1px solid ${dividerColor}` : undefined,
                alignItems: isList ? "center" : "stretch",
            }}
        >
            {cover}
            <div style={{ display: "flex", flexDirection: "column", gap: 6, minWidth: 0, flex: 1 }}>
                <div style={meta}>
                    <span>{s ? s.section_name : " "}</span>
                    {s?.breaking && (
                        <span style={{ color: "#fff", background: breakingColor, borderRadius: 999, padding: "1px 8px" }}>Breaking</span>
                    )}
                    {s?.developing && <span style={{ color: breakingColor }}>Developing</span>}
                </div>
                <div style={{ ...titleFont, margin: 0, background: s ? undefined : placeholder, minHeight: s ? undefined : "2.4em", borderRadius: 4 }}>
                    {s?.title}
                </div>
                {showSummary && s?.dek && (
                    <div
                        style={{
                            ...bodyFont,
                            color: mutedColor,
                            display: "-webkit-box",
                            WebkitLineClamp: 2,
                            WebkitBoxOrient: "vertical",
                            overflow: "hidden",
                        }}
                    >
                        {s.dek}
                    </div>
                )}
                {s && (
                    <div style={meta}>
                        <span>
                            {s.sources} outlet{s.sources === 1 ? "" : "s"}
                        </span>
                        <span>{timeAgo(s.updated)}</span>
                    </div>
                )}
            </div>
        </a>
    )
}

LeakStories.defaultProps = {
    site: "https://mohammedanasasif.github.io/Leak/",
    section: "all",
    count: 6,
    layout: "grid",
    columns: 3,
    gap: 24,
    showCovers: true,
    showSummary: true,
    coverRadius: 16,
    textColor: "#111111",
    mutedColor: "#6B6B6B",
    breakingColor: "#D7262E",
    dividerColor: "rgba(0,0,0,0.12)",
    refreshSeconds: 60,
}

addPropertyControls(LeakStories, {
    site: { type: ControlType.String, title: "Leak site" },
    section: { type: ControlType.Enum, title: "Section", options: SECTIONS, optionTitles: SECTION_TITLES },
    count: { type: ControlType.Number, title: "Stories", min: 1, max: 30, step: 1, displayStepper: true },
    layout: { type: ControlType.Enum, title: "Layout", options: ["grid", "list"], optionTitles: ["Grid", "List"], displaySegmentedControl: true },
    columns: { type: ControlType.Number, title: "Columns", min: 1, max: 4, step: 1, displayStepper: true, hidden: (p: Props) => p.layout !== "grid" },
    gap: { type: ControlType.Number, title: "Gap", min: 0, max: 64, unit: "px" },
    showCovers: { type: ControlType.Boolean, title: "Covers" },
    showSummary: { type: ControlType.Boolean, title: "Summary" },
    coverRadius: { type: ControlType.Number, title: "Cover radius", min: 0, max: 40, unit: "px", hidden: (p: Props) => !p.showCovers },
    titleFont: {
        type: ControlType.Font,
        title: "Headline font",
        controls: "extended",
        defaultFontType: "serif",
        defaultValue: { fontSize: 22, lineHeight: 1.2, fontWeight: 600 },
    },
    bodyFont: {
        type: ControlType.Font,
        title: "Body font",
        controls: "extended",
        defaultFontType: "sans-serif",
        defaultValue: { fontSize: 15, lineHeight: 1.5 },
    },
    textColor: { type: ControlType.Color, title: "Text" },
    mutedColor: { type: ControlType.Color, title: "Muted text" },
    breakingColor: { type: ControlType.Color, title: "Breaking" },
    dividerColor: { type: ControlType.Color, title: "Dividers" },
    refreshSeconds: { type: ControlType.Number, title: "Refresh (s)", min: 30, max: 600, step: 30 },
})
