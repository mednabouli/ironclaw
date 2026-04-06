# IronClaw Community — Discord Server Setup Guide

## Server Structure

### Categories & Channels

**📢 Information**
- `#announcements` — Releases, breaking changes, security advisories
- `#rules` — Code of conduct and community guidelines
- `#roadmap` — Current milestone progress and upcoming features

**💬 General**
- `#general` — Community chat
- `#introductions` — New member intros
- `#showcase` — Share what you've built with IronClaw

**🔧 Support**
- `#help` — General usage questions
- `#bug-reports` — Report issues (template: OS, version, repro steps)
- `#feature-requests` — Suggest new features

**👨‍💻 Development**
- `#contributors` — Discussion for contributors
- `#providers` — Provider implementation discussion (Ollama, Anthropic, OpenAI, etc.)
- `#channels-dev` — Channel development (REST, Telegram, Discord, CLI)
- `#wasm-plugins` — WASM plugin development
- `#agents` — Agent patterns (ReAct, DAG workflows)

**🤖 Bot Integration**
- `#ironclaw-bot` — Interact with a live IronClaw agent
- `#bot-logs` — Agent response logs and debugging

## Roles

| Role | Color | Permissions |
|------|-------|-------------|
| `@Maintainer` | Red | Admin, manage channels |
| `@Contributor` | Orange | Manage threads, pin messages |
| `@Community` | Green | Default role for verified members |
| `@Bot` | Blue | Bot accounts |

## Bot Setup

To connect IronClaw as a Discord bot:

```toml
# ironclaw.toml
[channels.discord]
enabled = true
token = "BOT_TOKEN_HERE"
guild_id = "YOUR_GUILD_ID"
channel_id = "IRONCLAW_BOT_CHANNEL_ID"
```

```bash
ironclaw start --channels discord
```

## Invite Link Template

```
https://discord.gg/YOUR_INVITE_CODE
```

Once the server is created, add the invite link to:
- `README.md` — badges section
- `CONTRIBUTING.md` — community section
- `index.html` — footer links
