// Language abstraction: dictionaries in i18n/es|en, reactive current lang.
// Components call t("home.greeting") and re-render on setLang.
import { es, type I18nKey } from "./i18n/es";
import { en } from "./i18n/en";

export type Lang = "es" | "en";
export type { I18nKey };

const dicts = { es, en };
const KEY = "smartpc.lang";

let current = $state<Lang>("es");

export function initLang(): void {
  if (typeof window === "undefined") return;
  const saved = window.localStorage.getItem(KEY);
  current = saved === "en" ? "en" : "es";
  document.documentElement.lang = current;
}

export function getLang(): Lang {
  return current;
}

export function setLang(lang: Lang): void {
  current = lang;
  try {
    window.localStorage.setItem(KEY, lang);
  } catch {
    /* private mode */
  }
  document.documentElement.lang = lang;
}

export function t(key: I18nKey, vars?: Record<string, string>): string {
  const template: string = dicts[current][key] ?? key;
  if (!vars) return template;
  return template.replace(/\{(\w+)\}/g, (m, name: string) => vars[name] ?? m);
}
