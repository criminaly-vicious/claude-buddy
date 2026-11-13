import { expect, mock, test } from 'claude-code/testing'
import type { On } from 'claude-code'

const SESSION = '11111111-1111-4111-8111-111111111111'
const TOKEN = 'b'.repeat(64)
const BRIDGE = JSON.stringify({ protocol: 1, port: 50123, token: TOKEN, pid: 7, started_at: 1 })

type Sent = { url: string; init?: { method?: string; headers?: Record<string, string>; body?: unknown } }

/** The world beneath the mod: the app running, no classic hooks, a PID. */
function world(on: On, options: { hooks?: unknown; bridge?: string | null; fetch?: 'hang' } = {}) {
  mock.env(on, { HOME: '/home/me' })
  const clock = mock.clock(on, { now: 1_000 })
  const sent: Sent[] = []
  on('settings.read', () => ({ value: options.hooks === undefined ? {} : { hooks: options.hooks } }))
  // The engine's own classic chain, beneath the mod.
  on('classic.Stop', () => ({}))
  on('classic.SessionEnd', () => ({}))
  on('fs.read', () => {
    if (options.bridge === null) throw new Error('ENOENT')
    return { value: options.bridge ?? BRIDGE }
  })
  on('process.run', () => ({
    value: { exitCode: 0, stdout: '4242\n', stderr: '', isStdoutTruncated: false, isStderrTruncated: false },
  }))
  on('session.id', () => ({ value: SESSION }))
  on('session.cwd', () => ({ value: '/home/me/demo' }))
  on('tool.call', () => ({ result: {} }))
  on('http.fetch', (_, e) => {
    sent.push(e as Sent)
    if (options.fetch === 'hang') return new Promise(() => {})
    return { value: { status: 202, ok: true, headers: {}, text: '' } }
  })
  return { clock, sent }
}

const body = (s: Sent) => JSON.parse(String(s.init?.body))

test('reports a classic event to the bridge', async ($, on) => {
  const { clock, sent } = world(on)
  await $.classic.Stop({
    session_id: SESSION,
    cwd: '/home/me/demo',
    stop_hook_active: false,
    last_assistant_message: 'a secret answer',
  })
  await clock.settle()

  expect(sent.length).toBe(1)
  expect(sent[0]?.url).toBe('http://127.0.0.1:50123/v1/events')
  expect(sent[0]?.init?.method).toBe('POST')
  expect(sent[0]?.init?.headers?.Authorization).toBe(`Bearer ${TOKEN}`)
  const b = body(sent[0]!)
  expect(b.event).toBe('Stop')
  expect(b.session_id).toBe(SESSION)
  expect(b.cwd).toBe('/home/me/demo')
  expect(b.claude_pid).toBe(4242)
  expect(String(sent[0]?.init?.body).includes('secret')).toBe(false)
})

test('reports a tool call from its envelope, with the target only', async ($, on) => {
  const { clock, sent } = world(on)
  await $.tool.call({ tool: 'Bash', command: 'API_KEY=sk-123 git push origin main' })
  await clock.settle()

  const pre = sent.map(body).find(b => b.event === 'PreToolUse')
  expect(pre?.tool_name).toBe('Bash')
  expect(pre?.target).toBe('git')
  expect(pre?.session_id).toBe(SESSION)
  expect(pre?.cwd).toBe('/home/me/demo')
  expect(sent.some(s => String(s.init?.body).includes('sk-123'))).toBe(false)
})

test('stays quiet when the classic buddy-hook is installed', async ($, on) => {
  const hooks = { Stop: [{ hooks: [{ type: 'command', command: 'C:\\x\\buddy-hook.exe', args: ['Stop'] }] }] }
  const { clock, sent } = world(on, { hooks })
  await $.classic.Stop({ session_id: SESSION, cwd: '/d', stop_hook_active: false })
  await clock.settle()
  expect(sent.length).toBe(0)
})

test('sends nothing when Claude Buddy is not running', async ($, on) => {
  const { clock, sent } = world(on, { bridge: null })
  await $.classic.Stop({ session_id: SESSION, cwd: '/d', stop_hook_active: false })
  await clock.settle()
  expect(sent.length).toBe(0)
})

test('never holds the session: a hanging send does not delay the event', async ($, on) => {
  const { clock, sent } = world(on, { fetch: 'hang' })
  const result = await $.classic.Stop({ session_id: SESSION, cwd: '/d', stop_hook_active: false })
  expect(result.block).toBe(undefined)
  await clock.advance(1_000)
  expect(sent.length).toBe(1)
  // The next event still goes out after the 300 ms give-up.
  await $.classic.SessionEnd({ session_id: SESSION, cwd: '/d', reason: 'other' })
  await clock.settle()
  expect(sent.length).toBe(2)
})
