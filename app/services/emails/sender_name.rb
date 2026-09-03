# frozen_string_literal: true

module Emails
  # Turns an RFC 5322 From value — "Ana Silva <ana@x.com>" or a bare
  # "ana.silva@x.com" — into a human display name (and its first name). Shared by
  # the feed cards (Campbooks::Feed::Base) and the bold Time agenda's source labels
  # ("from Ana's email").
  module SenderName
    module_function

    # "Display Name <addr>" → "Display Name"; bare address → humanized local part.
    def from_address(address)
      from = address.to_s.scrub("").delete("\u{FFFD}").strip
      return "" if from.blank?

      if from =~ /\A\s*"?([^"<]+?)"?\s*<.*>\s*\z/
        Regexp.last_match(1).strip
      else
        from.split("@").first.to_s.tr("._", " ").split.map(&:capitalize).join(" ").presence || from
      end
    end

    # Just the first token of the display name — "Ana Silva" → "Ana".
    def first_name(address)
      from_address(address).split(/\s+/).first.to_s
    end
  end
end
