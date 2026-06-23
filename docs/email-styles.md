# Email Styles

All transactional email templates use `MailerStyleHelper` (`app/helpers/mailer_style_helper.rb`) for every
inline style. Never hardcode hex values, font sizes, or padding in a mailer template.

## Colors

Defined in `MailerStyleHelper::COLORS`. Match the app's DESIGN.md calm-blue palette.

| Constant | Value | Role |
|---|---|---|
| `accent` | `#4054c8` | Brand name, buttons, code text |
| `accent_bg` | `#f0eff7` | Code box background |
| `page_bg` | `#f8f7f9` | Page background, stat box background |
| `text_primary` | `#374151` | Body copy |
| `text_heading` | `#1f2937` | Section headings |
| `text_secondary` | `#6b7280` | Expiry, metadata |
| `text_muted` | `#9ca3af` | "Ignore this email" text |
| `footer_text` | `#9a94a8` | Footer |
| `border` | `#e8e6ee` | Footer divider |
| `white` | `#ffffff` | Button text |

## Helper methods

### Typography

| Method | Output |
|---|---|
| `email_text_body(margin: "0 0 20px")` | 15px body paragraph style |
| `email_text_greeting` | 15px greeting with tighter bottom margin |
| `email_text_heading` | 17px semibold heading |
| `email_text_meta` | 13px secondary text (expiry, context) |
| `email_text_muted` | 13px muted text ("ignore this") |

### Components

| Method | Use |
|---|---|
| `email_button(url, label)` | Full button table block — use for CTAs |
| `email_code_box(code)` | Verification code display |
| `email_stat_box { ... }` | Wraps `email_stat_row` calls |
| `email_stat_row(label, value)` | One stat row inside a stat box |

### Custom style

For one-off styles, use `email_text` directly:

```erb
<p style="<%= email_text(font_size: "13px", color: MailerStyleHelper::COLORS[:text_secondary], margin: "0 0 4px") %>">
  Custom text
</p>
```

## Adding a new mailer

1. Add `helper :mailer_style` to the mailer class (inherited from `ApplicationMailer`).
2. Use only the helper methods above for inline styles.
3. If no existing helper covers your element pattern, add a new method to `MailerStyleHelper`
   instead of writing inline styles by hand.
4. Follow the app's copy tone: warm, conversational, human. No system-notification voice.
