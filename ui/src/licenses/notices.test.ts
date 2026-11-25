import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';
import notice from './NotoSans-OFL.txt?raw';

it('ships the complete bundled font copyright and licence without alteration', () => {
  expect(notice).toBe(new TextDecoder().decode(readFileSync('../crates/core/assets/fonts/OFL.txt')));
  expect(notice).toContain('Copyright 2018 The Noto Project Authors');
  expect(notice).toContain('SIL OPEN FONT LICENSE Version 1.1');
});
