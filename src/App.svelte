<script>
  import Main from './views/Main.svelte';
  import Capture from './views/Capture.svelte';
  import Hud from './views/Hud.svelte';

  let { label } = $props();
</script>

{#if label === 'capture'}
  <Capture />
{:else if label === 'hud'}
  <Hud />
{:else}
  <Main />
{/if}
