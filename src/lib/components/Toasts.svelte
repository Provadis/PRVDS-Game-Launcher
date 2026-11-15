<script lang="ts">
  import { app } from "$lib/state.svelte";
  import Icon from "./Icon.svelte";
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each app.toasts as t (t.id)}
    <div class="toast" class:error={t.kind === "error"}>
      <span>{t.text}</span>
      {#if t.action}
        <button
          class="btn"
          onclick={() => {
            t.action?.run();
            app.dismiss(t.id);
          }}>{t.action.label}</button>
      {/if}
      <button class="close" onclick={() => app.dismiss(t.id)} aria-label="Schließen"><Icon name="close" size={16} /></button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 24px;
    bottom: 24px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    z-index: 100;
    max-width: 480px;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 14px 14px 18px;
    border-radius: 12px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.45);
    font-size: 14px;
  }
  .toast.error {
    border-color: #6b2f2a;
  }
  .toast span {
    flex: 1;
  }
  .close {
    border: 0;
    background: none;
    color: var(--text-muted);
    padding: 4px;
    display: flex;
  }
</style>
