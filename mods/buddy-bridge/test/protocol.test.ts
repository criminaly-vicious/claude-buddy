import { describe, expect, test } from 'claude-code/testing'

import {
  bridgeFilePath,
  buildEvent,
  deriveTarget,
  hasClassicHook,
  parseBridgeFile,
  REPORTED_EVENTS,
} from '../hooks/protocol'
import cases from './target-cases.js'

type Case = { tool: string; input: Record<string, unknown>; target: string | null; note?: string }

const SESSION = '11111111-1111-4111-8111-111111111111'
const TOKEN = 'a'.repeat(64)

describe('target rules', () => {
  test('match the shared cases (and so bridge_protocol::target::derive)', async () => {
    const all = cases as Case[]
    expect(all.length > 20).toBe(true)
    for (const c of all) {
      expect({ ...c, got: deriveTarget(c.tool, c.input) ?? null }).toEqual({ ...c, got: c.target })
    }
  })
})

describe('buildEvent', () => {
  test('copies protocol fields only: no prompt, tool input or output', async () => {
    const prompt = buildEvent(
      'UserPromptSubmit',
      { session_id: SESSION, cwd: '/home/me/demo', prompt: 'my secret plan', transcript_path: '/t.jsonl' },
      { at: 5 },
    )
    expect(prompt).toEqual({ v: 1, event: 'UserPromptSubmit', session_id: SESSION, cwd: '/home/me/demo', at: 5 })

    const post = buildEvent(
      'PostToolUse',
      {
        session_id: SESSION,
        cwd: '/home/me/demo',
        tool_name: 'Read',
        tool_input: { file_path: '/home/me/demo/.env' },
        tool_response: { file: { content: 'API_KEY=sk-123' } },
      },
      { at: 6, claudePid: 4242 },
    )
    expect(post).toEqual({
      v: 1,
      event: 'PostToolUse',
      session_id: SESSION,
      cwd: '/home/me/demo',
      at: 6,
      claude_pid: 4242,
      tool_name: 'Read',
    })
    expect(JSON.stringify(post).includes('sk-123')).toBe(false)
  })

  test('derives the target on PreToolUse only', async () => {
    const pre = buildEvent(
      'PreToolUse',
      { tool_name: 'Bash', tool_input: { command: 'TOKEN=x git push' } },
      { sessionId: SESSION, cwd: '/d', at: 1 },
    )
    expect(pre?.target).toBe('git')
    expect(pre?.session_id).toBe(SESSION)
  })

  test('carries the per-event fields', async () => {
    const start = buildEvent('SessionStart', { session_id: SESSION, cwd: '/d', source: 'resume' }, { at: 1 })
    expect(start?.start_source).toBe('resume')
    const note = buildEvent(
      'Notification',
      { session_id: SESSION, cwd: '/d', notification_type: 'permission_prompt', message: 'Claude needs your permission' },
      { at: 1 },
    )
    expect(note?.notification_type).toBe('permission_prompt')
    expect(JSON.stringify(note).includes('permission_prompt"')).toBe(true)
    expect(JSON.stringify(note).includes('needs your permission')).toBe(false)
    const sub = buildEvent('Stop', { session_id: SESSION, cwd: '/d', agent_id: 'a1' }, { at: 1 })
    expect(sub?.agent_id).toBe('a1')
  })

  test('drops what the server would refuse', async () => {
    expect(buildEvent('Stop', { cwd: '/d' }, { at: 1 })).toBe(undefined)
    expect(buildEvent('PreToolUse', { session_id: SESSION, cwd: '/d' }, { at: 1 })).toBe(undefined)
    expect(buildEvent('Notification', { session_id: SESSION, cwd: '/d' }, { at: 1 })).toBe(undefined)
    expect(buildEvent('SessionStart', { session_id: SESSION, cwd: '/d' }, { at: 1 })).toBe(undefined)
  })

  test('cuts long strings to the protocol limits', async () => {
    const body = buildEvent('Stop', { session_id: 's'.repeat(100), cwd: 'c'.repeat(2000) }, { at: 1 })
    expect(body?.session_id.length).toBe(64)
    expect(body?.cwd.length).toBe(1024)
  })

  test('reports the events the tray installer registers', async () => {
    expect([...REPORTED_EVENTS]).toEqual([
      'SessionStart',
      'UserPromptSubmit',
      'PreToolUse',
      'PostToolUse',
      'PostToolUseFailure',
      'Notification',
      'PermissionRequest',
      'Stop',
      'SessionEnd',
    ])
  })
})

describe('bridge.json', () => {
  test('is found where the app writes it', async () => {
    const env = (vars: Record<string, string>) => (name: string) => vars[name]
    expect(bridgeFilePath(true, env({ APPDATA: 'C:\\Users\\me\\AppData\\Roaming' }))).toBe(
      'C:\\Users\\me\\AppData\\Roaming\\claude-buddy\\bridge.json',
    )
    expect(bridgeFilePath(false, env({ HOME: '/home/me' }))).toBe('/home/me/.config/claude-buddy/bridge.json')
    expect(bridgeFilePath(false, env({ HOME: '/home/me', XDG_CONFIG_HOME: '/x' }))).toBe('/x/claude-buddy/bridge.json')
    expect(bridgeFilePath(false, env({ HOME: '/home/me', XDG_CONFIG_HOME: 'relative' }))).toBe(
      '/home/me/.config/claude-buddy/bridge.json',
    )
    expect(bridgeFilePath(true, env({ CLAUDE_BUDDY_BRIDGE_FILE: 'D:\\b.json', APPDATA: 'C:\\x' }))).toBe('D:\\b.json')
    expect(bridgeFilePath(true, env({ APPDATA: '' }))).toBe(undefined)
  })

  test('is used only when its protocol and token are right', async () => {
    const file = (o: Record<string, unknown>) => JSON.stringify({ protocol: 1, port: 50123, token: TOKEN, pid: 1, started_at: 1, ...o })
    expect(parseBridgeFile(file({}))).toEqual({ port: 50123, token: TOKEN })
    expect(parseBridgeFile(file({ protocol: 2 }))).toBe(undefined)
    expect(parseBridgeFile(file({ token: 'short' }))).toBe(undefined)
    expect(parseBridgeFile(file({ token: 'A'.repeat(64) }))).toBe(undefined)
    expect(parseBridgeFile(file({ port: 0 }))).toBe(undefined)
    expect(parseBridgeFile('not json')).toBe(undefined)
    expect(parseBridgeFile('')).toBe(undefined)
  })
})

describe('classic hooks', () => {
  test('are recognised by the buddy-hook file name, in any event', async () => {
    const hooks = (command: string) => ({ Stop: [{ hooks: [{ type: 'command', command, async: true }] }] })
    expect(hasClassicHook(hooks('C:\\Users\\me\\AppData\\Local\\Claude Buddy\\buddy-hook.exe'))).toBe(true)
    expect(hasClassicHook(hooks('/opt/claude-buddy/buddy-hook'))).toBe(true)
    expect(hasClassicHook(hooks('notify-send done'))).toBe(false)
    expect(hasClassicHook(undefined)).toBe(false)
    expect(hasClassicHook({ Stop: 'not a list' })).toBe(false)
  })
})
