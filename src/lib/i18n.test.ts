import { describe, expect, test } from "vitest";
import { en } from "./i18n/en";
import { es } from "./i18n/es";
import { t } from "./i18n.svelte";

// es.ts is the key contract (en.ts is typed Record<I18nKey, string>, so this
// is a runtime backstop for dynamic/build-time consumers, not the compiler).
describe("i18n dictionaries", () => {
  test("en covers exactly the es keys", () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(es).sort());
  });

  test("no empty strings on either side", () => {
    for (const [k, v] of Object.entries({ ...es, ...en })) {
      expect(v.trim().length, k).toBeGreaterThan(0);
    }
  });

  test("interpolates {vars}, leaves unknown placeholders", () => {
    // Default lang without initLang is es.
    expect(t("voice.sayHey", { word: "hey" })).toContain("hey");
    expect(t("voice.sayHey", {})).toContain("{word}");
    expect(t("chat.send")).toBe(es["chat.send"]);
  });
});
