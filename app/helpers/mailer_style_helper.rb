module MailerStyleHelper
  # Colors mirror the app design system (DESIGN.md): a warm "Ember" signature,
  # near-black "ink" for text/headings, warm-neutral grays. Email-safe hex
  # (mail clients don't support oklch or gradients), derived from the app's
  # OKLCH tokens: accent = Ember oklch(64% 0.21 20); ink = oklch(20% 0.006 60).
  COLORS = {
    accent:        "#f14254",  # Ember — logo mark + the one CTA
    accent_bg:     "#ffeae2",  # soft Ember/peach highlight (code box)
    page_bg:       "#fcf9f7",  # whisper-warm page
    text_primary:  "#3b3734",  # body
    text_heading:  "#181513",  # headings + wordmark (ink)
    text_secondary: "#67625f",
    text_muted:    "#8a8581",
    footer_text:   "#918b86",
    border:        "#e3e1df",
    white:         "#ffffff"
  }.freeze

  # ── Typography ──────────────────────────────────────────────

  def email_text(font_size: "15px", color: COLORS[:text_primary], margin: "0 0 20px")
    "margin:#{margin}; color:#{color}; font-size:#{font_size}; line-height:1.6;"
  end

  def email_text_body(margin: "0 0 20px")
    email_text(margin: margin)
  end

  def email_text_greeting
    email_text(margin: "0 0 12px")
  end

  def email_text_heading
    "margin:0 0 16px; font-size:17px; font-weight:600; color:#{COLORS[:text_heading]};"
  end

  def email_text_meta
    "margin:0 0 6px; color:#{COLORS[:text_secondary]}; font-size:13px;"
  end

  def email_text_muted
    "margin:0; color:#{COLORS[:text_muted]}; font-size:13px;"
  end

  # ── Components ──────────────────────────────────────────────

  def email_button(url, label)
    style = "display:inline-block; padding:10px 24px; background-color:#{COLORS[:text_heading]}; " \
            "color:#{COLORS[:white]}; text-decoration:none; border-radius:8px; " \
            "font-weight:500; font-size:14px;"

    <<~HTML.html_safe
      <table cellpadding="0" cellspacing="0" role="presentation" style="margin:0 0 24px;">
        <tr>
          <td align="center" style="border-radius:8px; background-color:#{COLORS[:text_heading]};">
            <a href="#{url}" style="#{style}">#{label}</a>
          </td>
        </tr>
      </table>
    HTML
  end

  def email_code_box(code)
    <<~HTML.html_safe
      <table cellpadding="0" cellspacing="0" role="presentation" style="margin:0 0 20px;">
        <tr>
          <td style="padding:14px 24px; background-color:#{COLORS[:accent_bg]}; border-radius:8px; font-size:28px; font-weight:700; letter-spacing:5px; color:#{COLORS[:text_heading]}; text-align:center;">
            #{code}
          </td>
        </tr>
      </table>
    HTML
  end

  def email_stat_row(label, value)
    <<~HTML.html_safe
      <tr>
        <td style="padding:4px 0; font-size:13px; color:#{COLORS[:text_secondary]};">#{label}</td>
        <td style="padding:4px 0; padding-left:24px; font-size:20px; font-weight:700; color:#{COLORS[:text_heading]};">#{value}</td>
      </tr>
    HTML
  end

  def email_stat_box(&block)
    <<~HTML.html_safe
      <table cellpadding="0" cellspacing="0" role="presentation" style="margin:0 0 24px; background-color:#{COLORS[:page_bg]}; border-radius:8px;">
        <tr>
          <td style="padding:16px 20px;">
            <table cellpadding="0" cellspacing="0" role="presentation">
              #{capture(&block)}
            </table>
          </td>
        </tr>
      </table>
    HTML
  end
end
