import cargo from '../../Cargo.toml?raw';

// Cargo.toml [workspace.package] is the one source of the game version; the site only reads it.
const match = cargo.match(/^version = "(.+)"$/m);
if (!match) throw new Error('No workspace version in Cargo.toml');
export const VERSION = match[1];

export const BASE = import.meta.env.BASE_URL.replace(/\/?$/, '/');
export const REPO = 'https://github.com/wraithyy/the-last-dev';
