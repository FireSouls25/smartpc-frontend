// Remote shapes. Preferences mirrors the local stores 1:1 so sync is a
// plain upsert/download of one JSON document per user.
export interface Preferences {
  theme: "light" | "dark" | "auto";
  lang: "es" | "en";
  provider: string;
  model: string;
  gestures: boolean;
}

export interface Profile {
  id: string;
  preferences: Preferences;
  updated_at: string;
}
