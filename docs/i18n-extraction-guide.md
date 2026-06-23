# i18n string-extraction guide (Campbooks)

This is the contract every extraction agent follows so the whole app is consistent.
English is the **source** locale. You externalize hardcoded user-facing strings into
`config/locales/en/<your-zone>.yml` and replace them with `t()`/`l()` calls. You do
**not** translate to other languages (that's a later phase) and you do **not** change
behavior — only move strings.

## The golden rules

1. **Only externalize user-facing UI text.** Buttons, labels, headings, placeholders,
   empty states, table headers, `title=`/`aria-label`, flash messages, tooltips, prose.
2. **Never touch** (leave exactly as-is):
   - LLM prompts / AI instructions (system prompts, extraction prompts, `base_prompt`,
     anything sent to a model). When unsure if a string is a prompt, leave it.
   - Internal symbols/keys, log/debug strings, enum *identifiers*, CSS classes, URLs,
     data values, Stimulus action strings, SVG paths.
   - Brand/product nouns: **Campbooks, Scout, Skim, Zoho Mail, Gmail, Microsoft 365,
     Notion, Slack, Discord**. These are invariant — keep them literal (don't make keys).
   - Strings that become **database records** (e.g. SetupPresets `name:` values that are
     saved as tag/document-type names). Only their UI `description:` text is externalized.
   - Keyboard hint *keys* like `"E"`, `"P"`, `"⌘K"` — but DO externalize their text labels.
3. **Don't change logic or markup structure.** Same DOM, same classes, same conditionals.
4. **Verify Ruby edits**: run `ruby -c <file>` on every `.rb` you change before finishing.

## Where keys live (namespaces)

| Source | How to call | Resolves to | Put English in |
|---|---|---|---|
| ERB view `app/views/foo/bar.html.erb` | `t(".heading")` (leading dot = lazy) | `foo.bar.heading` | your zone yml under `en: foo: bar: heading:` |
| Controller `FooController#create` | `t(".saved")` | `foo.create.saved` | your zone yml under `en: foo: create: saved:` |
| Phlex component `Campbooks::SkimCard` | `t(".keep")` (Base scopes it) | `components.skim_card.keep` | `en: components: skim_card: keep:` |
| Shared chrome (any layer) | `t("shared.actions.save")` (absolute) | `shared.actions.save` | already defined in `shared.yml` — just reference |
| Enum value display | `human_enum(Model, :status, value)` | `activerecord.attributes.<model>.statuses.<value>` | `en/models.yml` (models agent owns this) |

- **Lazy keys** (`t(".x")`) auto-scope by file path in views/controllers and by class in
  components — prefer them. Use an **absolute** key only for shared/cross-zone strings.
- In **Phlex components**, `t(".x")` → `components.<snake_class>.x`; a non-dot key is
  absolute, so reach shared keys with `t("shared.actions.close")`.

## Shared catalog (use these exact keys; don't redefine)

For these common strings, reference the existing `shared.*` keys instead of making new ones:
`shared.actions.{save,save_changes,cancel,delete,remove,edit,add,create,update,close,back,
continue,confirm,done,send,search,connect,disconnect,learn_more,loading,skip_for_now}`,
`shared.nav.{mail,scout,documents,workflows,admin,settings}`. Anything not in this list →
make a zone-local key. (Don't edit `shared.yml`.)

## Pluralization (mandatory — no manual ternaries)

Replace `"#{n} #{n==1 ? 'email' : 'emails'}"`, `pluralize(n, "step")`, `"x".pluralize(n)`
with a `count:` key:

```ruby
t(".emails", count: n)
```
```yaml
en: { components: { skim_card: { emails: { one: "%{count} email", other: "%{count} emails" } } } }
```

## Interpolation & HTML

- Variables: `t(".greeting", name: user.name)` → `greeting: "Hi %{name}"`.
- `data-turbo-confirm`/`title=` with a value: `t(".delete_confirm", name: tag.name)`.
- **Strings containing markup**: keep the markup in the template and interpolate the
  dynamic/emphasized part, or use a `_html`-suffixed key (Rails marks `*_html` keys safe):
  ```erb
  <%= t(".intro_html", link: link_to(t(".providers"), settings_ai_path)) %>
  ```
  Prefer keeping `<strong>`/tags in the template with a `%{placeholder}` for the dynamic bit
  so the locale value stays clean prose.
- **JS-surfaced strings** (TipTap `data-*-placeholder-value`, `data-step-title`, Stimulus
  values): externalize the ERB/Ruby that produces them — pass `t(...)` into the attribute.

## Dates / numbers / currency — never strftime

Use `l(value, format: :name)` with these named formats (already defined):
`date: :section` ("June 2026"), `:full` ("Jun 20, 2026"), `:day_month` ("Jun 20"),
`:numeric` ("20/06/2026"); `time: :thread` (same-day clock), `:clock`, `:at`, `:at_short`.
Currency already localizes through `format_currency`/Money — don't touch it.

## YAML hygiene

- Quote values with `:` or leading special chars. Quote reserved-word keys (`"yes"`, `"no"`,
  `"on"`, `"off"`) — never use them bare.
- Keep keys snake_case and descriptive (`empty_title`, not `t1`).
- Your file starts with `en:` and nests by namespace. Only write YOUR assigned file(s);
  if the file already exists, **read it and merge** — never drop existing keys.

## Output

Return a structured report: files edited, keys added (count), anything you intentionally
skipped (and why), and anything ambiguous a human should review.
