import { defineConfig } from 'astro/config';

// Pages serves the landing at /the-last-dev/ and the game build under /the-last-dev/play/.
export default defineConfig({
  site: 'https://wraithyy.github.io',
  base: '/the-last-dev',
});
