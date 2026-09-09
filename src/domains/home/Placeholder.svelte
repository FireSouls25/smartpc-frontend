<script lang="ts">
  import { t } from "../../lib/i18n.svelte";

  let { domain }: { domain: string } = $props();
</script>

<div class="card text-center">
  <p class="text-lg font-bold">{domain}</p>
  <p class="chip mt-3">{t("common.soon")}</p>
  <p class="muted mt-2 text-sm">{t("common.soonDesc")}</p>
</div>
