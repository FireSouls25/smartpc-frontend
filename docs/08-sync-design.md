# 08 — Remote sync design (future, not built)

Supabase was scoped to **accounts + preferences sync** and never wired to any
UI (local auth stays the only login until multi-device sync is designed). To
keep the shipped bundle free of dead weight, the client module
(`src/supabase/`: config/client/sync/types + `@supabase/supabase-js`) was
removed in P2 #14. This file preserves the design so the work isn't lost.

## Principles

- Local-first always: control, history and secrets never leave the machine.
- Supabase keeps **accounts + preferences** so settings follow you across
  devices. History, audio and secrets never leave.
- The open problem is identity linking: the sidecar has its own users table
  (argon2id + JWT), Supabase Auth is a separate system. Syncing with the same
  email/password duplicates credentials across systems — needs a real design
  (e.g. sidecar-issued link tokens), not a quick wire-up.

## Sketched shape (from the removed module)

```ts
interface Preferences {
  theme: "light" | "dark" | "auto";
  lang: "es" | "en";
  provider: string;
  model: string;
  gestures: boolean;
}
```

- `config` — `VITE_SUPABASE_URL` / `VITE_SUPABASE_ANON_KEY` + configured check.
- `client` — lazy `supabase-js` client, throws when unconfigured.
- `sync` — `signUp/signIn/signOut/getSession`,
  `pushPreferences(userId, prefs)` (upsert into `profiles`),
  `pullPreferences(userId)`.

## One-time SQL (when a project is created)

```sql
create table profiles (
  id uuid primary key references auth.users(id) on delete cascade,
  preferences jsonb not null default '{}',
  updated_at timestamptz not null default now()
);
alter table profiles enable row level security;
create policy "own profile" on profiles for all
  using (auth.uid() = id) with check (auth.uid() = id);
```

## Re-entry checklist

1. Design identity linking (no password duplication).
2. `npm i @supabase/supabase-js`, restore the module from git history.
3. Wire push on theme/lang/provider change, pull on login; conflict rule:
   last-write-wins on `updated_at` is fine for a single JSON doc.
