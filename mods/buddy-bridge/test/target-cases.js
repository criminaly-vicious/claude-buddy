// Shared `target` cases (docs/architecture.md#target-rules): the mod's test
// imports this file and `crates/bridge-protocol` reads it, so the TypeScript
// and Rust rules cannot drift apart. Keep it pure JSON after `export default`:
// the Rust test parses whatever follows that.
// `target: null` means no target is sent.
export default [
  {
    "tool": "Read",
    "input": {
      "file_path": "/home/me/src/app.ts"
    },
    "target": "app.ts"
  },
  {
    "tool": "Edit",
    "input": {
      "file_path": "/home/me/src/app.ts"
    },
    "target": "app.ts"
  },
  {
    "tool": "MultiEdit",
    "input": {
      "file_path": "/home/me/src/app.ts"
    },
    "target": "app.ts"
  },
  {
    "tool": "Write",
    "input": {
      "file_path": "/home/me/src/app.ts"
    },
    "target": "app.ts"
  },
  {
    "tool": "Write",
    "input": {
      "file_path": "C:\\Users\\me\\proj\\src\\index.ts"
    },
    "target": "index.ts",
    "note": "Windows path"
  },
  {
    "tool": "Read",
    "input": {
      "file_path": "C:\\Users\\me\\secret\\plan.md"
    },
    "target": "plan.md",
    "note": "no folders"
  },
  {
    "tool": "Read",
    "input": {
      "file_path": "/home/me/secret/plan.md"
    },
    "target": "plan.md",
    "note": "no folders"
  },
  {
    "tool": "Read",
    "input": {
      "file_path": "/home/me/dir/"
    },
    "target": "dir",
    "note": "trailing separator"
  },
  {
    "tool": "NotebookEdit",
    "input": {
      "notebook_path": "/n/analysis.ipynb"
    },
    "target": "analysis.ipynb"
  },
  {
    "tool": "Grep",
    "input": {
      "pattern": "fn main"
    },
    "target": "fn main"
  },
  {
    "tool": "Glob",
    "input": {
      "pattern": "**/*.rs"
    },
    "target": "**/*.rs"
  },
  {
    "tool": "WebSearch",
    "input": {
      "query": "tauri transparent window"
    },
    "target": "tauri transparent window"
  },
  {
    "tool": "WebSearch",
    "input": {
      "query": "  padded  "
    },
    "target": "padded",
    "note": "trimmed"
  },
  {
    "tool": "WebFetch",
    "input": {
      "url": "https://docs.rs/tauri/latest?search=x#y"
    },
    "target": "docs.rs"
  },
  {
    "tool": "WebFetch",
    "input": {
      "url": "http://user:secret@example.com:8080/private"
    },
    "target": "example.com",
    "note": "no user info or port"
  },
  {
    "tool": "WebFetch",
    "input": {
      "url": "http://[::1]:3000/"
    },
    "target": "::1",
    "note": "IPv6 literal"
  },
  {
    "tool": "WebFetch",
    "input": {
      "url": "https://example.com/p?token=abc"
    },
    "target": "example.com",
    "note": "no query"
  },
  {
    "tool": "WebFetch",
    "input": {
      "url": "not a url"
    },
    "target": null
  },
  {
    "tool": "Bash",
    "input": {
      "command": "git push origin main"
    },
    "target": "git"
  },
  {
    "tool": "Bash",
    "input": {
      "command": "/usr/bin/cargo test --workspace"
    },
    "target": "cargo"
  },
  {
    "tool": "Bash",
    "input": {
      "command": "C:\\tools\\node.exe build.js"
    },
    "target": "node.exe"
  },
  {
    "tool": "Bash",
    "input": {
      "command": "RUST_LOG=debug cargo run"
    },
    "target": "cargo"
  },
  {
    "tool": "Bash",
    "input": {
      "command": "A=1 B=2 make"
    },
    "target": "make"
  },
  {
    "tool": "Bash",
    "input": {
      "command": "API_KEY=x curl https://x.dev"
    },
    "target": "curl"
  },
  {
    "tool": "Bash",
    "input": {
      "command": "API_KEY=sk-123"
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "PowerShell",
    "input": {
      "command": "API_KEY=sk-123"
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "Bash",
    "input": {
      "command": "$env:TOKEN='x'; git push"
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "PowerShell",
    "input": {
      "command": "$env:TOKEN='x'; git push"
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "Bash",
    "input": {
      "command": "$TOKEN do-something"
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "PowerShell",
    "input": {
      "command": "$TOKEN do-something"
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "Bash",
    "input": {
      "command": "\"quoted program\" arg"
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "PowerShell",
    "input": {
      "command": "\"quoted program\" arg"
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "Bash",
    "input": {
      "command": "'single' arg"
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "PowerShell",
    "input": {
      "command": "'single' arg"
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "Bash",
    "input": {
      "command": "`backtick`"
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "PowerShell",
    "input": {
      "command": "`backtick`"
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "Bash",
    "input": {
      "command": "--flag=value cmd"
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "PowerShell",
    "input": {
      "command": "--flag=value cmd"
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "Bash",
    "input": {
      "command": ""
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "PowerShell",
    "input": {
      "command": ""
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "Bash",
    "input": {
      "command": "   "
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "PowerShell",
    "input": {
      "command": "   "
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "Bash",
    "input": {
      "command": "\"C:\\Program Files\\git.exe\" status"
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "PowerShell",
    "input": {
      "command": "\"C:\\Program Files\\git.exe\" status"
    },
    "target": null,
    "note": "may hold a secret, or empty"
  },
  {
    "tool": "Agent",
    "input": {
      "subagent_type": "Explore",
      "prompt": "secret plan",
      "description": "d"
    },
    "target": "Explore",
    "note": "never the prompt"
  },
  {
    "tool": "Task",
    "input": {
      "subagent_type": "Explore",
      "prompt": "secret plan"
    },
    "target": "Explore"
  },
  {
    "tool": "TodoWrite",
    "input": {
      "todos": []
    },
    "target": null,
    "note": "no rule"
  },
  {
    "tool": "mcp__github__search_issues",
    "input": {
      "query": "x"
    },
    "target": null,
    "note": "MCP tools have no rule"
  },
  {
    "tool": "SomeFutureTool",
    "input": {
      "file_path": "/x/y"
    },
    "target": null,
    "note": "unknown tool"
  },
  {
    "tool": "Read",
    "input": {},
    "target": null,
    "note": "missing field"
  },
  {
    "tool": "Read",
    "input": {
      "file_path": 42
    },
    "target": null,
    "note": "wrong type"
  },
  {
    "tool": "WebSearch",
    "input": {
      "query": "qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq"
    },
    "target": "qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
    "note": "cut to 60 characters"
  },
  {
    "tool": "WebSearch",
    "input": {
      "query": "ééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééé"
    },
    "target": "éééééééééééééééééééééééééééééééééééééééééééééééééééééééééééé",
    "note": "cut by characters, not bytes"
  }
]
