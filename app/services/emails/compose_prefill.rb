# frozen_string_literal: true

module Emails
  # Computes the envelope + quoted block a composer opens with for a given
  # source message and mode. Shared by the Dock (EmailComposeController#create)
  # and the Desk (EmailMessagesController#new with mode/reply_to), so the two
  # surfaces can never drift on reply-all recipients or quoting.
  class ComposePrefill
    Result = Struct.new(:to, :cc, :subject, :quoted_body, keyword_init: true)

    def self.for(message:, mode:)
      new(message, mode.to_s).call
    end

    def initialize(message, mode)
      @message = message
      @mode = mode
    end

    def call
      Result.new(to: to_address, cc: cc_address, subject: subject, quoted_body: quoted_body)
    end

    private

    def to_address
      case @mode
      when "reply" then decode(@message.from_address.to_s)
      when "reply_all"
        recipients = []
        recipients << decode(@message.from_address.to_s) if @message.from_address.present?
        recipients.concat(parse_addresses(@message.to_address))
        recipients.reject! { |a| own_address?(a) }
        recipients.uniq.join(", ")
      else ""
      end
    end

    def cc_address
      return "" unless @mode == "reply_all"

      cc_list = parse_addresses(@message.cc_address || "")
      cc_list.reject! { |a| own_address?(a) }
      cc_list.uniq.join(", ")
    end

    def subject
      case @mode
      when "new_message" then ""
      when "forward"
        subject = decode(@message.subject.to_s)
        subject.match?(/^Fwd:\s*/i) ? subject : "Fwd: #{subject}"
      else
        subject = decode(@message.subject.to_s)
        subject.match?(/^Re:\s*/i) ? subject : "Re: #{subject}"
      end
    end

    def quoted_body
      return "" if @mode == "new_message"

      from = @message.from_address.present? ? decode(@message.from_address) : "Unknown"
      date = @message.received_at&.strftime("%b %d, %Y at %H:%M") || "Unknown date"
      body_html = @message.body.presence || @message.summary.presence || "(no content)"

      if @mode == "forward"
        <<~HTML
          <br><br>
          <p style="font-size: 12px; color: #9ca3af;">
            ---------- Forwarded message ----------<br>
            <b>From:</b> #{ERB::Util.html_escape(from)}<br>
            <b>Date:</b> #{date}<br>
            <b>Subject:</b> #{ERB::Util.html_escape(decode(@message.subject.to_s))}<br>
            <b>To:</b> #{ERB::Util.html_escape(decode(@message.to_address.to_s))}
          </p>
          <br>
          #{body_html}
        HTML
      else
        <<~HTML
          <br><br>
          <blockquote style="border-left: 2px solid #d1d5db; padding-left: 8px; margin-left: 0; color: #6b7280;">
            <p style="font-size: 12px; color: #9ca3af;">
              On #{date}, #{ERB::Util.html_escape(from)} wrote:
            </p>
            #{body_html}
          </blockquote>
        HTML
      end
    end

    # The forwarded originals, as attachment-tray entries.
    def self.forward_attachment_entries(message)
      return [] unless message&.files&.attached?

      message.files.blobs.map do |blob|
        { "signed_id" => blob.signed_id, "filename" => blob.filename.to_s, "byte_size" => blob.byte_size }
      end
    end

    def parse_addresses(str)
      return [] if str.blank?
      str.split(",").map { |a| decode(a.strip) }.select(&:present?)
    end

    # Messages synced from Zoho before the client decoded its HTML-escaped
    # metadata are stored as "&lt;user@example.com&gt;" — decode here so prefill
    # built from those rows matches own_address? and renders real addresses
    # instead of entity soup.
    def decode(str)
      str.include?("&") ? CGI.unescapeHTML(str) : str
    end

    # An address belongs to the receiving account when its email part matches —
    # compare the bare email so "Display Name <addr>" still gets dropped from
    # reply-all, or the user ends up emailing themselves.
    def own_address?(addr)
      bare_email(addr) == bare_email(@message.email_account.email_address)
    end

    def bare_email(addr)
      str = addr.to_s
      (str[/<([^>]+)>/, 1] || str).strip.downcase
    end
  end
end
