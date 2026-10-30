import js from "@eslint/js";
import tseslint from "typescript-eslint";
import svelte from 'eslint-plugin-svelte';
import svelteConfig from './svelte.config.js';

export default tseslint.config(
  { ignores: ["dist", "node_modules"] },
  js.configs.recommended,
  tseslint.configs.recommended,
  ...svelte.configs['flat/recommended'],
  {
    files: ['**/*.svelte'],
    languageOptions: { parserOptions: { parser: tseslint.parser, svelteConfig } },
    // svelte-check validates TypeScript globals/types; JS no-undef misreads DOM types.
    rules: { 'no-undef': 'off' },
  },
);
