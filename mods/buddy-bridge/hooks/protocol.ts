// The bridge protocol as the mod speaks it (docs/architecture.md#bridge-protocol).
// A mirror of `crates/bridge-protocol` and `crates/buddy-hook`: same fields,
// limits and `target` rules. The shared cases in `test/target-cases.js` keep
// the two from drifting apart. Pure functions only: no `$` in this file.

export const PROTOCOL_VERSION = 1
export const EVENTS_PATH = '/v1/events'
export const MAX_BODY_BYTES = 16 * 1024
export const BRIDGE_FILE_ENV = 'CLAUDE_BUDDY_BRIDGE_FILE'

const LIMITS = {
  event: 32,
  session_id: 64,
  cwd: 1024,
  tool_name: 128,
  target: 60,
  notification_type: 32,
  start_source: 16,
  agent_id: 64,
} as const

const TOOL_EVENTS = ['PreToolUse', 'PostToolUse', 'PostToolUseFailure', 'PermissionRequest']

/** The events the mod reports: the ones the tray's hook installer registers. */
export const REPORTED_EVENTS = [
  'SessionStart',
  'UserPromptSubmit',
  'PreToolUse',
  'PostToolUse',
  'PostToolUseFailure',
  'Notification',
  'PermissionRequest',
  'Stop',
  'SessionEnd',
] as const

export type EventBody = {
  v: number
  event: string
  session_id: string
  cwd: string
  at: number
  claude_pid?: number
  tool_name?: string
  target?: string
  notification_type?: string
  start_source?: string
  agent_id?: string
}

type Payload = Readonly<Record<string, unknown>>

/** The first `max` Unicode code points of `s` (as Rust's `truncate`). */
export function truncate(s: string, max: number): string {
  let out = ''
  let count = 0
  for (const ch of s) {
    if (count === max) return out
    out += ch
    count++
  }
  return out
}

const text = (payload: Payload, name: string): string | undefined => {
  const value = payload[name]
  return typeof value === 'string' ? value : undefined
}

/**
 * The wire event for one classic hook event, or `undefined` when it cannot be
 * sent. Only protocol fields are copied: prompts, tool inputs and outputs,
 * messages and transcript paths never leave this function, and `tool_input`
 * is used only to derive `target` for PreToolUse.
 */
export function buildEvent(
  event: string,
  payload: Payload,
  context: { sessionId?: string; cwd?: string; claudePid?: number; at: number },
): EventBody | undefined {
  const sessionId = text(payload, 'session_id') ?? context.sessionId
  const cwd = text(payload, 'cwd') ?? context.cwd
  if (!sessionId || !cwd) return undefined
  const toolName = text(payload, 'tool_name')
  const toolInput = payload.tool_input
  const body: EventBody = {
    v: PROTOCOL_VERSION,
    event: truncate(event, LIMITS.event),
    session_id: truncate(sessionId, LIMITS.session_id),
    cwd: truncate(cwd, LIMITS.cwd),
    at: context.at,
  }
  if (context.claudePid !== undefined && Number.isInteger(context.claudePid) && context.claudePid > 0) {
    body.claude_pid = context.claudePid
  }
  if (toolName !== undefined) body.tool_name = truncate(toolName, LIMITS.tool_name)
  if (event === 'PreToolUse' && toolName !== undefined) {
    const target = deriveTarget(toolName, typeof toolInput === 'object' && toolInput !== null ? (toolInput as Payload) : {})
    if (target !== undefined) body.target = target
  }
  const notification = event === 'Notification' ? text(payload, 'notification_type') : undefined
  if (notification !== undefined) body.notification_type = truncate(notification, LIMITS.notification_type)
  const source = event === 'SessionStart' ? text(payload, 'source') : undefined
  if (source !== undefined) body.start_source = truncate(source, LIMITS.start_source)
  const agentId = text(payload, 'agent_id')
  if (agentId !== undefined) body.agent_id = truncate(agentId, LIMITS.agent_id)

  if (TOOL_EVENTS.includes(event) && body.tool_name === undefined) return undefined
  if (event === 'Notification' && body.notification_type === undefined) return undefined
  if (event === 'SessionStart' && body.start_source === undefined) return undefined
  return body
}

/**
 * The `target` for a tool call (docs/architecture.md#target-rules), the same
 * as `bridge_protocol::target::derive`. When in doubt, no target.
 */
export function deriveTarget(tool: string, input: Payload): string | undefined {
  const field = (name: string) => text(input, name)
  let target: string | undefined
  switch (tool) {
    case 'Read':
    case 'Edit':
    case 'MultiEdit':
    case 'Write':
      target = mapDefined(field('file_path'), fileName)
      break
    case 'NotebookEdit':
      target = mapDefined(field('notebook_path'), fileName)
      break
    case 'Grep':
    case 'Glob':
      target = field('pattern')
      break
    case 'WebSearch':
      target = field('query')
      break
    case 'WebFetch':
      target = mapDefined(field('url'), urlHost)
      break
    case 'Bash':
    case 'PowerShell':
      target = mapDefined(field('command'), programName)
      break
    case 'Agent':
    case 'Task':
      target = field('subagent_type')
      break
    default:
      return undefined
  }
  if (target === undefined) return undefined
  const cut = truncate(trimRust(target), LIMITS.target)
  return cut === '' ? undefined : cut
}

const mapDefined = <T>(value: T | undefined, fn: (v: T) => string | undefined) =>
  value === undefined ? undefined : fn(value)

/** Rust's `str::trim`: Unicode White_Space, which JS `trim` also covers. */
const trimRust = (s: string) => s.trim()

/** Last non-empty path segment, accepting `/` and `\`. */
function fileName(path: string): string {
  const parts = path.split(/[\/\\]/)
  for (let i = parts.length - 1; i >= 0; i--) {
    const part = parts[i]
    if (part !== undefined && part !== '') return part
  }
  return ''
}

/** Host of an absolute URL, without user info, port, path or query. */
function urlHost(url: string): string | undefined {
  const scheme = url.indexOf('://')
  if (scheme < 0) return undefined
  const authority = url.slice(scheme + 3).split(/[\/?#]/)[0] ?? ''
  const hostPort = authority.slice(authority.lastIndexOf('@') + 1)
  // An IPv6 literal is bracketed: `[::1]:8080`.
  const host = hostPort.startsWith('[') ? hostPort.slice(1).split(']')[0] : hostPort.split(':')[0]
  return host === undefined || host === '' ? undefined : host
}

/**
 * Program name of a shell command. Leading `NAME=value` assignments are
 * skipped; anything that still looks like an assignment, a variable or a
 * quoted string gives no target, because it may hold a secret.
 */
function programName(command: string): string | undefined {
  const first = command.split(/\s+/).find(token => token !== '' && !isEnvAssignment(token))
  if (first === undefined) return undefined
  if (/[=$"'`]/.test(first) || first.toLowerCase().startsWith('$env:')) return undefined
  return fileName(first)
}

function isEnvAssignment(token: string): boolean {
  const eq = token.indexOf('=')
  return eq > 0 && /^[A-Za-z_][A-Za-z0-9_]*$/.test(token.slice(0, eq))
}

/** `bridge.json`: where the app listens and its token, when usable. */
export function parseBridgeFile(contents: string): { port: number; token: string } | undefined {
  let file: unknown
  try {
    file = JSON.parse(contents)
  } catch {
    return undefined
  }
  if (typeof file !== 'object' || file === null) return undefined
  const { protocol, port, token } = file as Record<string, unknown>
  if (protocol !== PROTOCOL_VERSION) return undefined
  if (typeof port !== 'number' || !Number.isInteger(port) || port < 1 || port > 65535) return undefined
  if (typeof token !== 'string' || !/^[0-9a-f]{64}$/.test(token)) return undefined
  return { port, token }
}

/**
 * Path of `bridge.json`, from environment variables only, as
 * `bridge_protocol::resolve_bridge_file_path`.
 */
export function bridgeFilePath(windows: boolean, env: (name: string) => string | undefined): string | undefined {
  const nonEmpty = (name: string) => {
    const value = env(name)
    return value === undefined || value === '' ? undefined : value
  }
  const explicit = nonEmpty(BRIDGE_FILE_ENV)
  if (explicit !== undefined) return explicit
  if (windows) {
    const appData = nonEmpty('APPDATA')
    return appData === undefined ? undefined : `${appData}\\claude-buddy\\bridge.json`
  }
  const xdg = nonEmpty('XDG_CONFIG_HOME')
  if (xdg !== undefined && xdg.startsWith('/')) return `${xdg}/claude-buddy/bridge.json`
  const home = nonEmpty('HOME')
  return home === undefined ? undefined : `${home}/.config/claude-buddy/bridge.json`
}

/**
 * Whether the classic `buddy-hook` is installed in Claude Code's settings
 * (the `hooks` value `$.settings.read` answers), matched by file name as the
 * tray installer does. The mod stays quiet then, so no event is sent twice.
 */
export function hasClassicHook(hooks: unknown): boolean {
  if (typeof hooks !== 'object' || hooks === null) return false
  for (const groups of Object.values(hooks as Record<string, unknown>)) {
    if (!Array.isArray(groups)) continue
    for (const group of groups) {
      const list = (group as { hooks?: unknown } | null)?.hooks
      if (!Array.isArray(list)) continue
      for (const hook of list) {
        const command = (hook as { command?: unknown } | null)?.command
        if (typeof command !== 'string') continue
        const name = fileName(command).toLowerCase()
        if (name === 'buddy-hook' || name === 'buddy-hook.exe') return true
      }
    }
  }
  return false
}
