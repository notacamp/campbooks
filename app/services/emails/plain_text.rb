module Emails
  # Readable text of an email body for the AI pipelines (task/reminder extraction
  # and their pre-filter gates). A bare `strip_tags` keeps the TEXT of removed
  # tags, so an HTML email leaks its <style>/<head> blocks as content — Outlook
  # "MsoNormal" and Zoho "zm_…parse" CSS rules routinely dominate the first
  # thousands of characters and crowd the real message out of the model's context
  # window. This removes those nodes content-and-all, optionally strips the quoted
  # reply history (so a reply is analysed for what the sender actually wrote, not
  # for asks buried in the quoted thread), and flattens to squished text.
  #
  # Also the single home of the quote-stripping logic shared with the display
  # helpers (EmailMessageHelpers#email_preview_html and friends).
  class PlainText
    # Nodes whose inner text is never message content.
    NOISE_NODES = %w[style script head title meta link].freeze

    # Client-specific containers that hold quoted reply history.
    QUOTE_CONTAINERS = [ ".gmail_quote", ".gmail_extra", ".moz-cite-prefix",
                         "#divRplyFwdMsg", "#appendonsend" ].freeze

    # Opening line of a quoted-reply attribution header, across clients/locales:
    # "On … wrote:", Apple/Gmail; "Le … a écrit", "El … escribió", "Em … escreveu";
    # Outlook/Zoho "From:/De:/Von:" header; "---- Original Message ----".
    ATTRIBUTION_RE = /\A\s*(?:-{2,}\s*)?(?:On\b.{0,200}?\bwrote\b|Le\b.{0,200}?\ba\s+écrit|El\b.{0,200}?\bescribió|Em\b.{0,200}?\bescreveu|(?:From|De|Von):\s|-{2,}\s*Original)/im

    class << self
      # → plain, whitespace-squished text (HTML and plain-text bodies alike).
      def of(raw, strip_quotes: true)
        raw = raw.to_s
        return "" if raw.blank?

        if raw.match?(/<\w+[^>]*>/)
          clean_fragment(raw, strip_quotes: strip_quotes).to_text(encode_special_chars: false).squish
        else
          (strip_quotes ? strip_text_quotes(raw) : raw).squish
        end
      end

      # → Loofah fragment with noise nodes (and optionally the quoted history)
      # removed, ready for a display helper to sanitise and serialise.
      def clean_fragment(raw, strip_quotes: false)
        fragment = Loofah.fragment(raw.to_s)
        strip_quote_nodes(fragment) if strip_quotes
        fragment.css(*NOISE_NODES).each(&:remove)
        fragment
      end

      # Plain-text reply: drop every line that starts with ">" (the quoted history)
      # plus a trailing "On … wrote:" / "_____" attribution left with nothing under it.
      def strip_text_quotes(raw)
        kept = raw.to_s.split(/\r?\n/).reject { |line| line.lstrip.start_with?(">") }.join("\n")
        kept.sub(/\n*^On\b.{0,200}?\bwrote:\s*\z/im, "")
            .sub(/\n*^_{5,}.*\z/m, "")
            .strip
      end

      private

      # Remove quoted reply history from a parsed fragment, keeping only what the
      # sender actually wrote in the latest message. Drops <blockquote> citations
      # and the known client quote containers, then walks back from each removed
      # quote to peel off the attribution header ("From: …/On … wrote:") and blank
      # spacers that introduced it — without touching the real reply before them.
      def strip_quote_nodes(fragment)
        fragment.css(*QUOTE_CONTAINERS).each(&:remove)
        fragment.css("blockquote", '[id*="zmail"]').each do |quote|
          strip_attribution_before(quote)
          quote.remove
        end
        fragment
      end

      # Peel off the contiguous run of attribution header / blank-spacer / <hr>
      # nodes immediately before a quote, stopping at the first real reply content.
      def strip_attribution_before(node)
        sib = node.previous_sibling
        while sib
          text = sib.text.to_s.strip
          break unless text.empty? || sib.name == "hr" || attribution_header?(text)

          prev = sib.previous_sibling
          sib.remove
          sib = prev
        end
      end

      # An attribution line, by its opening words or by the Zoho/Outlook header
      # shape ("Subject:" alongside a From/Sent/Date/To label).
      def attribution_header?(text)
        text.match?(ATTRIBUTION_RE) ||
          (text.match?(/\bSubject:/i) && text.match?(/\b(?:From|Sent|Date|To):/i))
      end
    end
  end
end
