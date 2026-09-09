<script lang="ts">
  import { getLang, setLang, t, type Lang } from "../lib/i18n.svelte";
  import { getTheme, setTheme, type Theme } from "../lib/theme.svelte";
</script>

<div class="flex items-center gap-2">
  <select
    class="field"
    style="width: auto; padding: 0.375rem 0.625rem; font-size: 0.75rem;"
    value={getLang()}
    onchange={(e) => setLang(e.currentTarget.value as Lang)}
    aria-label="Language"
  >
    <option value="es">ES</option>
    <option value="en">EN</option>
  </select>
  <select
    class="field"
    style="width: auto; padding: 0.375rem 0.625rem; font-size: 0.75rem;"
    value={getTheme()}
    onchange={(e) => setTheme(e.currentTarget.value as Theme)}
    aria-label={t("common.theme")}
  >
    <option value="auto">{t("common.auto")}</option>
    <option value="light">{t("common.light")}</option>
    <option value="dark">{t("common.dark")}</option>
  </select>
</div>
