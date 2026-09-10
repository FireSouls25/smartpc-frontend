# Supabase — remote sync only

Local-first: the Rust sidecar (`src/native/`) owns auth, control and data.
Supabase keeps **accounts + preferences** so settings follow you across
devices. History, audio and secrets never leave the machine.

Not wired to any UI yet — that happens when multi-device sync is designed.

## Setup

1. Create a project at supabase.com, copy URL + anon key into env:
   `VITE_SUPABASE_URL`, `VITE_SUPABASE_ANON_KEY`.
2. Run this SQL once (Table editor → SQL):

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

## Module

- `config.ts` — env + `isSupabaseConfigured()`
- `client.ts` — lazy `supabase-js` client (throws if unconfigured)
- `types.ts` — `Preferences` mirrors the local stores 1:1
- `sync.ts` — `signUp/signIn/signOut/getSession`,
  `pushPreferences/pullPreferences`
