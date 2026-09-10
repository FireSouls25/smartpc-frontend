// Remote account (for sync) + preferences push/pull.
// Not wired to any UI yet: local auth (sidecar) stays the only login until
// multi-device sync is designed.
import { supabase } from "./client";
import type { Preferences } from "./types";

export async function signUp(email: string, password: string) {
  const { data, error } = await supabase().auth.signUp({ email, password });
  if (error) throw error;
  return data;
}

export async function signIn(email: string, password: string) {
  const { data, error } = await supabase().auth.signInWithPassword({
    email,
    password,
  });
  if (error) throw error;
  return data;
}

export async function signOut() {
  const { error } = await supabase().auth.signOut();
  if (error) throw error;
}

export async function getSession() {
  const { data, error } = await supabase().auth.getSession();
  if (error) throw error;
  return data.session;
}

export async function pushPreferences(
  userId: string,
  prefs: Preferences,
): Promise<void> {
  const { error } = await supabase().from("profiles").upsert({
    id: userId,
    preferences: prefs,
    updated_at: new Date().toISOString(),
  });
  if (error) throw error;
}

export async function pullPreferences(
  userId: string,
): Promise<Preferences | null> {
  const { data, error } = await supabase()
    .from("profiles")
    .select("preferences")
    .eq("id", userId)
    .maybeSingle();
  if (error) throw error;
  return (data?.preferences as Preferences) ?? null;
}
