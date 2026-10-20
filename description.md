# Markdown Preview

A preview of the markdown file you are editing, in a pane to the right of the editor. It shows the **saved** file: it refreshes when you save, and says when the buffer has changes that are not saved yet.

## What it shows

Headings, emphasis, strong text, inline code, code blocks, quotes, bullet, numbered and task lists, tables, rules and links. Images are shown as a line naming them (a terminal cannot draw them).

## Using it

Turn it on in "Plugins: Manage", then put it in the **side pane** (it needs a screen of 100 columns or more). Open a `.md` or `.markdown` file and save it: the preview follows the file in front.

Turn the **mouse wheel** over the pane to scroll it (3 lines a notch), and drag the border between the editor and the preview to make it wider or narrower ("View: Wider Side Pane" does it from the keyboard). With the keyboard in the pane (`plugins: open markdown` in the palette, or click it): `up`/`down` or `k`/`j`, `pageup`, `pagedown`, `ctrl+u`, `ctrl+d` (half a page), `home`, `end` to read (the header says how far down you are), `tab` and `shift+tab` to move between links, `enter` or a click to follow one, `esc` to go back to the editor.

A link to another file opens it in the editor (and the preview follows it, to the heading when the link names one); `#heading` links jump inside the file; links to the web are shown, not opened.

## What it asks to be allowed to

See the text of the `.md` and `.markdown` files you open, as saved on disk (never other files), and open the files its links lead to. It runs on a thread of its own, because rendering a large document takes processor time.
