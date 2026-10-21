const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const source = fs.readFileSync('web/app.js', 'utf8');
const apiSource = source.slice(source.indexOf('async function api('), source.indexOf('\nfunction brand('));
const logoutSource = source.split('\n').find(line => line.startsWith('async function logout()'));
const failure = (status, title, detail = title) => ({status, ok: false, json: async () => ({title, detail})});
const success = data => ({status: 200, ok: true, json: async () => data});

function setup(responses) {
  const calls = [], messages = [], state = {csrf: 'old', user: null};
  const context = vm.createContext({Headers, Error, s: state, base: 'https://api.example.test',
    login: message => messages.push(message), note: message => messages.push(message), render: () => {},
    fetch: async (url, options) => {
      calls.push({url, options});
      const response = responses.shift();
      if (response instanceof Error) throw response;
      assert.ok(response, 'Unexpected additional request');
      return response;
    }
  });
  vm.runInContext(apiSource + '\n' + logoutSource, context);
  return {context, calls, state, messages};
}

(async () => {
  let test = setup([failure(403, 'CSRF Validation Failed'), success({csrf_token: 'current'}), success({status: 'totp_required'})]);
  const body = {email: 'synthetic@example.test', password: 'test-only'};
  assert.equal((await test.context.api('/api/v1/auth/login', {method: 'POST', body})).status, 'totp_required');
  assert.equal(test.calls.length, 3);
  assert.equal(test.calls[1].url, 'https://api.example.test/api/v1/auth/csrf');
  assert.equal(test.calls[0].options.headers.get('x-csrf-token'), 'old');
  assert.equal(test.calls[2].options.headers.get('x-csrf-token'), 'current');
  assert.equal(test.calls[2].options.body, test.calls[0].options.body);

  for (const response of [failure(500, 'Temporarily Unavailable'), failure(403, 'Origin Rejected'), new Error('offline')]) {
    test = setup([response]);
    await assert.rejects(test.context.api('/api/v1/account/profile', {method: 'PUT', body: {}}));
    assert.equal(test.calls.length, 1);
  }

  test = setup([failure(403, 'CSRF Validation Failed'), success({csrf_token: 'current'}), failure(403, 'CSRF Validation Failed')]);
  test.state.user = {id: 1};
  await assert.rejects(test.context.api('/api/v1/account/profile', {method: 'PUT', body: {}}), /Inicia sesión de nuevo/);
  assert.equal(test.calls.length, 3);
  assert.equal(test.state.user, null);
  assert.equal(test.messages.length, 1);

  test = setup([failure(403, 'CSRF Validation Failed')]);
  await assert.rejects(test.context.api('/api/v1/auth/csrf'));
  assert.equal(test.calls.length, 1);

  test = setup([failure(500, 'Temporarily Unavailable')]);
  test.state.user = {id: 1};
  await test.context.logout();
  assert.equal(test.state.user.id, 1);
  assert.equal(test.messages[0], 'Temporarily Unavailable');
  console.log('PASS: CSRF refresh, bounded retry, unchanged request body, no network/server/origin retry, reauthentication recovery, and honest logout failure');
})().catch(error => { console.error(error); process.exitCode = 1; });
