# Smart PC — Frontend (Svelte 5)

Domain architecture: each business domain owns its screens, API calls and
state. Transversal code (theme, i18n, fetch) lives in `lib/` and `shared/`.

```
src/
  app/            shell: App (boot + restore), Shell (layout), hash router
  domains/
    auth/         LoginPage · RegisterPage · auth.api · auth.store (runes)
    home/         HomePage (assistant mockup) · Placeholder (future domains)
  lib/
    theme.css     THE single theme file (Catppuccin Latte + dark)
    theme.svelte  light/dark/auto manager (data-theme)
    i18n/         es.ts (key contract) · en.ts
    i18n.svelte   reactive t() + persistence
    api.ts        typed fetch wrapper (VITE_API_URL)
  shared/         Logo · LangTheme (transversal UI)
```

## Run (needs the backend for auth)

```bash
npm install
npm run dev      # http://localhost:5173
npm run build
```

Only auth is wired to the API; the main page is a visual mockup to design
the rest of the look and functions.
