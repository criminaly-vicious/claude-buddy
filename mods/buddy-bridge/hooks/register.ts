// buddy-bridge: Claude Buddy's Claude Code mod (ADR-0013,
// docs/session-detection.md#the-claude-code-mod). It reports each session's
// hook events to the Claude Buddy app, as the classic `buddy-hook` does,
// without an entry in settings.json.
//
// It must never block or fail a session. Every hook hands the event on at once
// (`return next(e)`) and reports it on the side; nothing is awaited on the
// turn's path, and every error is swallowed. Each `.catch` hands the event on
// unchanged should a hook ever fail, so the mod never gates a session.

import type { EngineInterface, Register } from 'claude-code'

import {
  bridgeFilePath,
  buildEvent,
  EVENTS_PATH,
  hasClassicHook,
  MAX_BODY_BYTES,
  parseBridgeFile,
  type EventBody,
} from './protocol'

/** How long one send may take, as `buddy-hook`'s budget. */
const SEND_TIMEOUT_MS = 300
/** How long a report waits for the Claude Code PID before going without it. */
const PID_WAIT_MS = 2_000
/** How long a read of `bridge.json` is trusted before reading it again. */
const BRIDGE_TTL_MS = 5_000

type Bridge = { port: number; token: string }
type Payload = Readonly<Record<string, unknown>>

// Per module load: one per session, and again after a reload.
let quiet: Promise<boolean> | undefined
let claudePid: Promise<number | undefined> | undefined
let bridge: { value: Bridge | undefined; readAt: number } | undefined

export const register: Register = on => {
  quiet = undefined
  claudePid = undefined
  bridge = undefined

  // `classic.PreToolUse` hands the tool call's envelope (`tool`, the call's
  // arguments spread beside it), not a settings hook's stdin: rebuild that.
  on('classic.PreToolUse', ($, e, next) => {
    const { tool, tool_use_id: _id, agentId, ...input } = e as Payload
    report($, 'PreToolUse', {
      tool_name: tool,
      tool_input: input,
      ...(typeof agentId === 'string' ? { agent_id: agentId } : {}),
    })
    return next(e)
  }).catch(($, e, next) => next(e))
  on('classic.SessionStart', ($, e, next) => {
    report($, 'SessionStart', e)
    return next(e)
  }).catch(($, e, next) => next(e))
  on('classic.UserPromptSubmit', ($, e, next) => {
    report($, 'UserPromptSubmit', e)
    return next(e)
  }).catch(($, e, next) => next(e))
  on('classic.PostToolUse', ($, e, next) => {
    report($, 'PostToolUse', e)
    return next(e)
  }).catch(($, e, next) => next(e))
  on('classic.PostToolUseFailure', ($, e, next) => {
    report($, 'PostToolUseFailure', e)
    return next(e)
  }).catch(($, e, next) => next(e))
  on('classic.Notification', ($, e, next) => {
    report($, 'Notification', e)
    return next(e)
  }).catch(($, e, next) => next(e))
  on('classic.PermissionRequest', ($, e, next) => {
    report($, 'PermissionRequest', e)
    return next(e)
  }).catch(($, e, next) => next(e))
  on('classic.Stop', ($, e, next) => {
    report($, 'Stop', e)
    return next(e)
  }).catch(($, e, next) => next(e))
  on('classic.SessionEnd', ($, e, next) => {
    report($, 'SessionEnd', e)
    return next(e)
  }).catch(($, e, next) => next(e))
}

/** Reports one event on the side. Never throws; the hook does not wait. */
function report($: EngineInterface, event: string, payload: Payload): void {
  const at = Date.now()
  reportNow($, event, payload, at).catch(() => {})
}

async function reportNow($: EngineInterface, event: string, payload: Payload, at: number) {
  if (await isQuiet($)) return
  const [sessionId, cwd, pid] = await Promise.all([
    payload.session_id === undefined ? $.session.id() : undefined,
    payload.cwd === undefined ? $.session.cwd() : undefined,
    within($, pidOf($), PID_WAIT_MS),
  ])
  const body = buildEvent(event, payload, { sessionId, cwd, claudePid: pid, at })
  if (body !== undefined) await send($, body, at)
}

/** The classic hooks are installed: they report, and the mod stays quiet. */
function isQuiet($: EngineInterface): Promise<boolean> {
  quiet ??= $.settings.read({ key: 'hooks' }).then(hasClassicHook, () => false)
  return quiet
}

/**
 * The PID of this Claude Code process, for the app's liveness check: the
 * parent of a child it starts, since no `$` call answers it.
 */
function pidOf($: EngineInterface): Promise<number | undefined> {
  claudePid ??= lookUpPid($).catch(() => undefined)
  return claudePid
}

async function lookUpPid($: EngineInterface): Promise<number | undefined> {
  const windows = (await $.env.get('OS')) === 'Windows_NT'
  const argv = windows
    ? [
        'powershell.exe',
        '-NoProfile',
        '-NonInteractive',
        '-Command',
        '(Get-CimInstance Win32_Process -Filter "ProcessId=$PID").ParentProcessId',
      ]
    : ['sh', '-c', 'echo $PPID']
  const { exitCode, stdout } = await $.process.run(argv, { timeoutMs: 10_000 })
  const pid = Number.parseInt(stdout.trim(), 10)
  return exitCode === 0 && pid > 0 ? pid : undefined
}

async function readBridge($: EngineInterface, now: number): Promise<Bridge | undefined> {
  if (bridge !== undefined && now - bridge.readAt < BRIDGE_TTL_MS) return bridge.value
  const env: Record<string, string | undefined> = {
    CLAUDE_BUDDY_BRIDGE_FILE: await $.env.get('CLAUDE_BUDDY_BRIDGE_FILE'),
    APPDATA: await $.env.get('APPDATA'),
    XDG_CONFIG_HOME: await $.env.get('XDG_CONFIG_HOME'),
    HOME: await $.env.get('HOME'),
  }
  const windows = (await $.env.get('OS')) === 'Windows_NT'
  const path = bridgeFilePath(windows, name => env[name])
  const contents = path === undefined ? '' : await $.fs.read(path).catch(() => '')
  const value = parseBridgeFile(contents)
  bridge = { value, readAt: now }
  return value
}

async function send($: EngineInterface, body: EventBody, at: number) {
  const target = await readBridge($, at)
  if (target === undefined) return
  const json = JSON.stringify(body)
  if (json.length > MAX_BODY_BYTES) return
  const sent = await within(
    $,
    $.http.fetch(`http://127.0.0.1:${target.port}${EVENTS_PATH}`, {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${target.token}`,
        'Content-Type': 'application/json',
      },
      body: json,
    }),
    SEND_TIMEOUT_MS,
  )
  // No answer, or a token the app no longer knows (it restarted): read
  // bridge.json again next time.
  if (sent === undefined || sent.status === 401) bridge = undefined
}

/** `promise`'s value, or `undefined` once `ms` have passed. */
function within<T>($: EngineInterface, promise: Promise<T>, ms: number): Promise<T | undefined> {
  return Promise.race([promise, $.clock.sleep(ms).then(() => undefined)])
}
