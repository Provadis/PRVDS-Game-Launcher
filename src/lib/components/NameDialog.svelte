<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import { untrack } from "svelte";
  let {
    title,
    label,
    initial = "",
    confirmLabel = t("Speichern"),
    extra,
    onsubmit,
    onclose,
  }: {
    title: string;
    label: string;
    initial?: string;
    confirmLabel?: string;
    extra?: import("svelte").Snippet;
    onsubmit: (name: string) => void;
    onclose: () => void;
  } = $props();

  let name = $state(untrack(() => initial));

  function submit(e: SubmitEvent) {
    e.preventDefault();
    if (name.trim()) onsubmit(name.trim());
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="nd-title">
  <form onsubmit={submit}>
    <h2 id="nd-title">{title}</h2>
    <label>
      <span>{label}</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input class="input" bind:value={name} autofocus />
    </label>
    {@render extra?.()}
    <div class="row">
      <button class="btn" type="button" onclick={onclose}>{t("Abbrechen")}</button>
      <button class="btn primary" type="submit" disabled={!name.trim()}>{confirmLabel}</button>
    </div>
  </form>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: rgba(5, 6, 8, 0.7);
    display: grid;
    place-items: center;
    padding: 24px;
  }
  .dialog {
    width: min(480px, 100%);
  }
  form {
    padding: 28px;
    border-radius: 20px;
    background: var(--bg-raised);
    border: 1px solid var(--border-strong);
    box-shadow: 0 30px 80px rgba(0, 0, 0, 0.6);
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  h2 {
    font-size: 22px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 14px;
    color: var(--text-2);
  }
  .row {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
</style>
