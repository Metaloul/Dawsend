<script>
  // Vumètre stéréo à segments, de -60 à 0 dBFS : vert jusqu'à -9, ambre jusqu'à -1, rouge
  // au-dessus. `level` arrive déjà lissé (lib/meters.svelte.js) ; la partie allumée
  // s'arrête toujours au bord d'un segment (pas de 4 px).
  let { level = [0, 0], dim = false, vertical = false } = $props();
  let size = $state(0);

  const PITCH = 4;
  const lit = (v) => {
    const f = v > 0 ? Math.min(Math.max((20 * Math.log10(v) + 60) / 60, 0), 1) : 0;
    return Math.round((f * size) / PITCH) * PITCH;
  };
</script>

{#if vertical}
  <div class="meter vertical" class:dim aria-hidden="true" bind:clientHeight={size}>
    {#each level as v}
      <div class="bar"><div class="fill" style="--f: {lit(v)}px"></div></div>
    {/each}
  </div>
{:else}
  <div class="meter" class:dim aria-hidden="true" bind:clientWidth={size}>
    {#each level as v}
      <div class="bar"><div class="fill" style="--f: {lit(v)}px"></div></div>
    {/each}
  </div>
{/if}

<style>
  .meter {
    display: grid;
    gap: 3px;
    width: 100%;
  }
  .bar {
    position: relative;
    height: 5px;
    background: var(--unlit);
    mask: repeating-linear-gradient(90deg, #000 0 3px, transparent 3px 4px);
  }
  .fill {
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, var(--ok) 0 85%, var(--warn) 85% 98.4%, var(--bad) 98.4%);
    clip-path: inset(0 calc(100% - var(--f)) 0 0);
  }
  .dim .fill {
    background: var(--dim);
  }

  .vertical {
    grid-auto-flow: column;
    width: auto;
    height: 100%;
  }
  .vertical .bar {
    width: 6px;
    height: 100%;
    mask: repeating-linear-gradient(0deg, #000 0 3px, transparent 3px 4px);
  }
  .vertical .fill {
    background: linear-gradient(0deg, var(--ok) 0 85%, var(--warn) 85% 98.4%, var(--bad) 98.4%);
    clip-path: inset(calc(100% - var(--f)) 0 0 0);
  }
  .vertical.dim .fill {
    background: var(--dim);
  }
</style>
