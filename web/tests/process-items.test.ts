import { test } from 'node:test';
import assert from 'node:assert/strict';
import { processItems } from '../src/process-items.js';

test('Scout numbered items and legacy JSON render as text without throwing', () => {
  assert.deepEqual(processItems('1. Dos equipos\n2. Cable, modelo A'), ['Dos equipos','Cable, modelo A']);
  assert.deepEqual(processItems('["Dos equipos","<script>alert(1)</script>"]'), ['Dos equipos','<script>alert(1)</script>']);
  assert.deepEqual(processItems(''), []);
  assert.deepEqual(processItems('[dato incompleto'), ['[dato incompleto']);
});
