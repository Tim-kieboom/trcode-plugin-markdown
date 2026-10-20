# trcode-plugin-markdown

A markdown preview for [TerminalCode](https://github.com/Tim-kieboom/TerminalCode), in a pane
beside the editor. It is a WebAssembly plugin: the editor sends it the saved text of the
markdown file in front (the capability `files.text`), it draws it wrapped to the width of its
pane, and a link to another file opens that file in the editor.

## Installing

In a TerminalCode that has plugin API 1.5 (the next release after 0.8.0) open "Plugins: Manage", choose
Markdown Preview and press `enter`. You are asked what it may do (see the text of `.md` and
`.markdown` files you open, as saved, and open the files its links lead to; it runs on a thread
of its own) and where its view goes: choose the **side pane**, which sits right of the editor.

By hand: build it and put it in the plugins folder of the config folder
(`~/.config/terminalcode/plugins/markdown/` on Linux):

```sh
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --release
dir=~/.config/terminalcode/plugins/markdown
mkdir -p "$dir"
cp manifest.toml description.md "$dir/"
cp target/wasm32-unknown-unknown/release/trcode_plugin_markdown.wasm "$dir/markdown.wasm"
```

## Using it

See [description.md](description.md), which is also what `d` shows in the manager. The files it
gets can be narrowed in `settings.json`: `"plugin_file_types": { "markdown": ["md"] }`.

## How it is made

- `src/render.rs`: markdown (`pulldown-cmark`) to lines of styled text wrapped to a width, in
  the syntax slots of markup (`syntax.text.title`, `syntax.text.strong` and so on), so a theme
  that colors markdown colors the preview. Tables are drawn with box lines and shrink to fit.
- `src/viewer.rs`: what the plugin knows (the document, the width the editor told it, the line
  the reader is on, the links) and how it answers each message.
- `manifest.toml`: `files.text` with `file_types = ["md", "markdown"]`, `editor.open` for links,
  and `thread = "own"`, because parsing a document gets a budget that grows with its size
  (only on a thread of its own).

## Development

`cargo test` runs the renderer and the viewer without WebAssembly. The module built, run by the
editor, is measured and tested from the editor's repository:

```sh
TRCODE_PLUGIN_MARKDOWN_DIR=/path/to/this/repo \
  cargo test -p terminal_code real_markdown -- --ignored --nocapture
```

## Releasing

Set `version` in `manifest.toml` (and `Cargo.toml`), tag `vX.Y.Z` and push the tag. The release
workflow builds the module, attaches it with the manifest and `description.md` to a release, and
prints the entry for [trcode-plugins](https://github.com/Tim-kieboom/trcode-plugins) (or run
`scripts/add_plugin.py` there).

## License

MIT or Apache-2.0, at your option.
