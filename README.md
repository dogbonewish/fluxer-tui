# Fluxer TUI

TUI for [Fluxer](https://fluxer.app), built with Ratatui.

## Requirements

- Rust toolchain
- A terminal with reasonable size (the layout expects multiple panes)
- Network access for the API, gateway WebSocket, and browser login

## Build and run

```bash
cargo build --release
# binary: target/release/fluxer-tui
cargo run --release
```

## Community
   
   > *fluxer-tui running on a vintage Apple iBook - courtesy of @astromahdi#2602*
   
   ![fluxer-tui on iBook](assets/IMG_7484.jpeg)

## First-time login

If you have no valid saved token then you can easily login via the browser. The TUI will automatically:

1. Print an **8-character login code** (also copied to the clipboard when possible).
2. Opens your browser to complete login (or you can open the printed URL manually).
3. Polls until the browser flow finishes, then saves the token and starts the UI.

You can pass a token once without storing it in config:

```bash
target/release/fluxer-tui --token 'YOUR_TOKEN_HERE'
```

To clear the saved token and exit:

```bash
target/release/fluxer-tui/fluxer-tui --logout
```

Or you can CTRL + L (see below)

## Command-line options

| Option | Description |
|--------|-------------|
| `--token <TOKEN>` | Use this token for this run (still written to config if login succeeds). |
| `--config <PATH>` | Config file path (default: see below). |
| `--api-base-url <URL>` | API base URL (default: `https://api.fluxer.app/v1`). |
| `--logout` | Clear stored token from config and exit. |

## Config file

Default path (unless you pass `--config`): **`…/fluxer-tui/config.toml`** under the OS config directory (on Linux this is usually **`~/.config/fluxer-tui/config.toml`**).

It stores `api_base_url`, `token`, `last_server_id`, and `last_channel_id` so the client can restore your last place.

## Interface overview

The UI has four **focus** areas, cycled with **Tab** / **Shift+Tab** (or **h**/**l** / **Left**/**Right**):

1. **Servers** - guild list and Direct Messages (`@me`).
2. **Channels** - channels for the selected server.
3. **Messages** - message history for the selected channel.
4. **Input** - compose box (text channels only, when you have permission to send).

A status line at the top shows gateway state, errors, and hints.


## Keyboard shortcuts

### Global (not typing in the input box)

| Key | Action |
|-----|--------|
| **Tab** | Next focus (Servers → Channels → Messages → Input → …). |
| **Shift+Tab** | Previous focus. |
| **Left** / **h** | Previous focus. |
| **Right** / **l** | Next focus. |
| **i** | Jump to **Input** (text channel, if you can send). |
| **Enter** | On a **link** channel: open URL in browser. On a **text** channel: jump to **Input**. |
| **Esc** | Clear message selection; focus **Channels**. |
| **↑** / **k** | Move selection / scroll (depends on focus; see below). |
| **↓** / **j** | Move selection / scroll (depends on focus). |
| **PageUp** | Scroll message list up (larger step). |
| **PageDown** | Scroll message list down (larger step). |
| **Ctrl+N** / **Ctrl+P** | Next / previous **text** channel (wraps; works from input too unless a popup is open). |
| **Ctrl+K** | Open **channel picker** (type to filter, **Enter** to jump). |
| **Alt+A** | Jump to the **next channel** (after current) that has **unread** or **mention** badges; wraps. |
| **F1** | **Keybindings** overlay - **↑** / **↓** / **PgUp** / **PgDn** scroll when it does not fit (**Esc** / **Enter** / **q** to close). |
| **Ctrl+H** | Same overlay when focus is **not** the message input (in input, **Ctrl+H** / **Ctrl+Backspace** delete the previous word). |
| **R** | **Refresh** active channel messages and guild metadata used for loads (clears local message cache for the channel and resets fetch/backoff state for the current guild). |
| **Ctrl+C** | Quit. |
| **Ctrl+L** | Log out (clear intent flag) and quit - token is cleared when the process exits cleanly after this. |
| **q** | Quit. |

### When focus is **Servers**

| Key | Action |
|-----|--------|
| **↑** / **k** | Previous server / DM entry. |
| **↓** / **j** | Next server / DM entry. |

### When focus is **Channels**

| Key | Action |
|-----|--------|
| **↑** / **k** | Previous channel. |
| **↓** / **j** | Next channel. |

Changing channels marks read state for the new channel when applicable.

### When focus is **Messages**

| Key | Action |
|-----|--------|
| **↑** / **k** | With **message select mode** on: previous message. Otherwise: scroll up a few lines. |
| **↓** / **j** | With **message select mode** on: next message. Otherwise: scroll down a few lines. |
| **s** | **Select** mode: select the latest message (start of thread for reply / react / forward). |
| **r** | **Reply** to the selected message (only in select mode). Moves focus to **Input** with reply state set. |
| **e** | **React**: pick an emoji (**Enter** sends the reaction via API; **Esc** cancels). |
| **f** | **Forward**: optional note, switch target channel (**Ctrl+K** or list), **Enter** to send (reference type forward). |
| **Ctrl+E** | **Edit** the selected message (your messages only; **Enter** in input to save, **Esc** to cancel). |
| **Ctrl+D** | **Delete** the selected message (yours, or with **Manage Messages**). |
| **[** | Load **older messages** (prepends history; repeat until exhausted). |

Edited messages show **(edited)** in dim italics after the timestamp when the API supplies `edited_timestamp` (including live **MESSAGE_UPDATE** from the gateway).

### When focus is **Input**

| Key | Action |
|-----|--------|
| **Enter** | Send message, save **edit**, or forward with reference only. Long lines **wrap** and the input bar **grows** with the text. |
| **↑** | Leave **Input** and focus **Messages**. |
| **Backspace** | Delete character. |
| **Ctrl+Backspace** / **Ctrl+H** | Delete the previous whitespace-separated word. |
| **Ctrl+U** | Clear the whole input line. |
| **Esc** | If replying/forwarding, cancel; if picking a reaction, cancel; otherwise leave **Input** and focus **Channels**. |
| **:** (colon) | Start **custom emoji** autocomplete (server emojis + unicode picker). |
| **@** | Start **@mention** autocomplete (users/roles in guilds; DMs use recipients). Triggers loading full member list from the API only when needed. |

Plain letters (without **Ctrl**) are inserted into the message, except where autocomplete consumes them.

### @mention autocomplete (while open)

| Key | Action |
|-----|--------|
| **↑** / **↓** | Previous / next suggestion. |
| **Tab** / **Enter** | Insert selected mention. |
| **Esc** | Close autocomplete. |
| **Backspace** | Edit text; filter updates. |
| **Any character** | Type to filter (unless **Ctrl**). |

### Emoji autocomplete (while open)

| Key | Action |
|-----|--------|
| **↑** / **↓** | Previous / next emoji. |
| **Tab** / **Enter** | Insert selected emoji into the message, **or** confirm **reaction** when **e** flow is active. |
| **Esc** | Close autocomplete. |
| **Backspace** | Edit; filter updates. |
| **Any character** | Type to filter (unless **Ctrl**). |

---

## Tips

- **Capital R** is refresh; **lowercase r** in message select mode is reply.
- Message **select mode** is only active after **s** in the **Messages** focus.

## Known issues & TODOs

- **Markdown** parser is still hand-crafted with duct-tape and incomplete.
- **Voice** is view-only; no join/transmit/hear.
- Notification settings dont save, gotta fix but will do later.
- Themeing/syncing with a "rice" will be something i hope to implement next!
- reply highlights will be next update or a patch later on in the night (probably at 1am)
- performance mode doesnt do much for clamshells from the 2000's (so far.)
- "Failed to load guild members: 504 Gateway Timeout Gateway timeout." I believe this happens in servers with members lists that are disabled, so it is PROBABLY intended.
- Open a **feature request** issue for anything you want that is not here yet.

## License

MIT
