<script lang="ts">
  // Indicateur d'attente (recherche, connexion, reconnexion) : figé si
  // `prefers-reduced-motion`.
  interface Props {
    size?: number;
  }

  let { size = 18 }: Props = $props();
</script>

<svg
  class="kr-spin"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2.6"
  stroke-linecap="round"
  aria-hidden="true"
>
  <circle cx="12" cy="12" r="9" opacity=".3"></circle>
  <path d="M21 12a9 9 0 0 0-9-9"></path>
</svg>

<style>
  svg {
    flex-shrink: 0;
  }
</style>
