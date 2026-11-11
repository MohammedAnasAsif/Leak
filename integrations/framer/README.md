# Leak news in a Framer site

`LeakStories.tsx` is a Framer code component that shows Leak's live stories, with Leak's generated cover art, in any Framer site you build. It refreshes every minute.

## Add it to your Framer project

1. Make sure Leak's site is live (the **Publish news** workflow has run at least once), so `https://<you>.github.io/<repo>/api/latest.json` opens in a browser.
2. In Framer: **Assets** panel → **Code** → **+** → **New component**. Name it `LeakStories`.
3. Replace everything in the editor with the contents of `LeakStories.tsx` and save.
4. Drag **LeakStories** from Assets onto your page, wherever you want stories to appear.
5. In the right-hand panel, set:
   - **Leak site**: your Leak address, ending in `/`
   - **Section**: Top stories, or one section (World, Politics, Finance…)
   - **Stories**, **Layout** (grid or list), **Columns**, **Gap**
   - **Headline font** and **Body font**: pick your page's fonts so it blends in
   - **Text**, **Muted text**, **Breaking** and **Dividers** colours
6. Use one component per block on your page: for example a 3-column grid of top stories near the top, then a list of Finance stories further down.
7. Publish the Framer site.

Each story opens its Leak story page, with every outlet's coverage, in a new tab.

## What the data contains

The component reads `api/latest.json` (top stories) or `api/sections/<section>.json`. Each story has:

`title`, `dek` (short summary), `path` (story page), `cover` (SVG artwork), `section`, `section_name`, `sources` (number of outlets), `regions`, `first_source`, `created`, `updated`, `breaking`, `developing`.

You can also bind these to your own Framer layouts with Framer's **Fetch** feature instead of this component.

## Licences

The component and Leak's covers are MIT licensed. The Framer template you use is covered by its own licence on Framer, so keep that site inside Framer as its licence allows.
