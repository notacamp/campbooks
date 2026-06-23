# frozen_string_literal: true

module Campbooks
  # Renders an email's latest message as real HTML in a sandboxed iframe, so the
  # home-feed preview keeps the sender's formatting (links, lists, tables, inline
  # images) instead of a flattened text excerpt. Quoted reply history is stripped
  # upstream by `email_preview_html` — only what the sender wrote this time shows.
  #
  # The frame is height-capped (~10 lines) with a "Read more" / "Show less" toggle
  # driven by the email_preview controller, which sizes the iframe to its content
  # and reveals the toggle only when the message overflows the cap. With no JS the
  # server-rendered cap still gives a clipped preview with a fade.
  #
  # Security: the sandbox has NO allow-scripts (nothing in the email can run JS),
  # backed by a locked-down CSP; allow-same-origin is granted only so the parent
  # can measure the content height. Links open as a normal new tab. Remote images
  # load for formatting parity with the message detail view.
  class EmailHtmlPreview < Campbooks::Base
    COLLAPSED_HEIGHT = "14rem"

    def initialize(message:, collapsed_height: COLLAPSED_HEIGHT, **attrs)
      @message = message
      @collapsed_height = collapsed_height
      @attrs = attrs
    end

    def view_template
      div(class: class_names("flex flex-col", @attrs.delete(:class)), **@attrs,
          data: { controller: "email-preview", email_preview_collapsed_value: @collapsed_height }) do
        viewport
        toggle_button
      end
    end

    private

    def viewport
      div(
        class: "relative overflow-hidden rounded-xl bg-white",
        style: "height: #{@collapsed_height}; transition: height .2s ease-out",
        data: { email_preview_target: "viewport" }
      ) do
        # Built as a raw tag: Phlex bars the `sandbox` attribute by name, and the
        # srcdoc is escaped by hand for the attribute context below.
        raw safe(iframe_tag)
        # Fade hint that there's more below, shown only while collapsed.
        div(
          class: "pointer-events-none absolute inset-x-0 bottom-0 h-12 bg-gradient-to-t from-white to-transparent",
          data: { email_preview_target: "fade" }
        )
      end
    end

    # The sandboxed preview frame. `srcdoc` is HTML-escaped once so the email's own
    # quotes/markup can't break out of the attribute; the browser unescapes it back
    # to the document source when it parses the frame.
    def iframe_tag
      %(<iframe title="#{CGI.escapeHTML(t('components.email_html_preview.frame_title'))}" ) +
        %(sandbox="allow-same-origin allow-popups allow-popups-to-escape-sandbox" ) +
        %(referrerpolicy="no-referrer" class="block w-full" style="border:0; height: #{@collapsed_height}" ) +
        %(srcdoc="#{CGI.escapeHTML(srcdoc)}" ) +
        %(data-email-preview-target="frame" data-action="load->email-preview#frameLoaded"></iframe>)
    end

    def toggle_button
      button(
        type: "button",
        class: "mt-1.5 hidden self-start rounded-sm text-[12.5px] font-semibold text-muted-foreground underline-offset-2 transition-colors hover:text-foreground hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        aria: { expanded: "false" },
        data: { email_preview_target: "button", action: "email-preview#toggle" }
      ) do
        span(data: { email_preview_target: "more" }) { t("components.clamp_text.more") }
        span(class: "hidden", data: { email_preview_target: "less" }) { t("components.clamp_text.less") }
      end
    end

    # A minimal HTML document around the cleaned email. `base target=_blank` opens
    # links in a new tab; the CSP blocks scripts/objects/frames (belt-and-braces
    # with the sandbox) while allowing inline styles, images, and fonts so the
    # email still looks like itself.
    def srcdoc
      <<~HTML
        <!doctype html><html><head><meta charset="utf-8">
        <base target="_blank">
        <meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src * data:; style-src 'unsafe-inline'; font-src * data:">
        <style>
          html,body{margin:0;padding:0;background:transparent}
          body{padding:16px 18px;display:flow-root;font-family:system-ui,-apple-system,'Segoe UI',sans-serif;font-size:14px;line-height:1.55;color:#1f2937;word-wrap:break-word;overflow-wrap:anywhere}
          img{max-width:100%;height:auto}
          a{color:#b45309}
          table{max-width:100%}
          blockquote{margin:0 0 0 .25rem;padding-left:.75rem;border-left:2px solid #e5e7eb;color:#6b7280}
        </style></head>
        <body>#{email_preview_html(@message)}</body></html>
      HTML
    end
  end
end
