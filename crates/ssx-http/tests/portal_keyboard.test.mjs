import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';

// Execute the production browser asset through its DOM event boundary. This
// regression owns shortcut dispatch only; real layout/browser evidence is T03.
function fixture() {
  const listeners = new Map();
  const opened = [];
  const document = { documentElement: { dataset: { defaultTheme: 'dark' } }, body: { dataset: {} } };
  const element = (kind) => ({
    kind, isContentEditable: false, classList: { toggle() {} },
    matches(selector) { return selector.split(',').map((part) => part.trim()).includes(kind); },
    setAttribute() {}, addEventListener() {},
    focus() { document.activeElement = this; },
  });
  const input = element('input');
  const link = element('a');
  const card = { classList: { toggle() {} }, scrollIntoView() {}, querySelector() { return link; } };
  const button = element('button');
  document.querySelector = (selector) => selector === '[data-theme-toggle]' ? button : selector === '.search-input' ? input : null;
  document.querySelectorAll = (selector) => selector === '[data-result-card]' ? [card] : [];
  document.addEventListener = (name, callback) => listeners.set(name, callback);
  const window = { localStorage: { getItem() { return null; }, setItem() {} }, matchMedia() { return { matches: false }; }, addEventListener() {}, open(...args) { opened.push(args); } };
  vm.runInNewContext(fs.readFileSync(new URL('../src/portal/behavior.js', import.meta.url), 'utf8'), { document, window });
  function key(name, target = document.activeElement ?? element('body')) {
    let prevented = false;
    listeners.get('keydown')({ key: name, target, preventDefault() { prevented = true; } });
    return prevented;
  }
  return { document, element, input, link, opened, key };
}

test('j/k navigation leaves Enter to the currently focused native control', () => {
  const f = fixture();
  assert.equal(f.key('j'), true);
  assert.equal(f.document.activeElement, f.link);
  assert.equal(f.key('Enter', f.link), false, 'the focused result link keeps native activation');
  for (const kind of ['summary', 'button', 'a', 'input', 'select']) {
    const control = f.element(kind);
    control.focus();
    assert.equal(f.key('Enter', control), false, `Enter was intercepted on ${kind}`);
    assert.equal(f.document.activeElement, control);
  }
  assert.deepEqual(f.opened, []);
});

test('slash does not steal focus while typing in another form control', () => {
  const f = fixture();
  const dateInput = f.element('input');
  dateInput.focus();
  assert.equal(f.key('/', dateInput), false);
  assert.equal(f.document.activeElement, dateInput);
  assert.equal(f.key('/', f.element('body')), true);
  assert.equal(f.document.activeElement, f.input);
});
