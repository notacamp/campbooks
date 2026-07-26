# frozen_string_literal: true

require "net/imap"
require "net/smtp"
require "digest"
require "erb"

module Imap
  # IMAP/SMTP mail client for password-authenticated email accounts (provider: :imap).
  #
  # Identity scheme: the RFC822 Message-ID header, with surrounding whitespace
  # and one pair of enclosing angle brackets stripped, is the canonical
  # provider_message_id. When the header is absent a SHA-256 "noid-" digest of
  # envelope date/from/to/subject is used. noid- IDs are not server-searchable:
  # resolve_uid returns nil for them and dependent ops (move, mark-read) skip the
  # message silently; a later resync heals the record once the server has it.
  #
  # Folder IDs: the raw IMAP mailbox name (modified-UTF-7, exactly as the server
  # names it) is used as folderId everywhere — store it verbatim and pass it back
  # verbatim to every IMAP command. list_folders canonicalises display names for
  # the app's name-based machinery without touching the folderId.
  #
  # Connection lifecycle: one Net::IMAP per client instance, opened lazily on first
  # use. A single transparent reconnect fires on network-level errors (IOError,
  # EOF, reset, BYE). The sync strategy calls #disconnect in ensure; other callers
  # rely on process teardown.
  #
  # v1 deferrals (matching existing providers' precedent):
  # - Provider-side moves leave provider_folder_id stale until the next resync.
  # - Deleted messages are moved to Trash, never hard-expunged.
  # - Read-state reconciliation is one-way (unread -> read).
  # - CONDSTORE is not used.
  class MailClient
    # Items fetched in each incremental and backfill FETCH call.
    # BODY.PEEK marks messages non-read on the server; the response attr key
    # loses .PEEK — locate it by the "BODY[HEADER.FIELDS" prefix (see normalize_message).
    FETCH_ITEMS = [
      "UID",
      "ENVELOPE",
      "FLAGS",
      "INTERNALDATE",
      "BODYSTRUCTURE",
      "BODY.PEEK[HEADER.FIELDS (MESSAGE-ID LIST-UNSUBSCRIBE PRECEDENCE AUTO-SUBMITTED X-CAMPBOOKS-KIND)]"
    ].freeze

    RECONNECT_ERRORS = [
      IOError, EOFError, Errno::ECONNRESET, Net::IMAP::ByeResponseError
    ].freeze

    # Folder attributes that mark a mailbox as non-selectable or as a virtual
    # aggregate (e.g. Gmail "All Mail" shows as :All in addition to its named folder).
    SKIP_ATTRS = %i[Noselect All Flagged].freeze

    # RFC 6154 special-use attributes -> canonical display names understood by
    # EmailFolder::DEFAULT_ORDER, Emails::Sender::SENT_FOLDER_NAMES, etc.
    SPECIAL_USE_MAP = {
      Sent:    "Sent",
      Drafts:  "Drafts",
      Trash:   "Trash",
      Junk:    "Spam",
      Archive: "Archive"
    }.freeze

    # Case-insensitive match on the last path segment when no special-use attr is set.
    NAME_PATTERN_MAP = [
      [ /\A(sent|sent items|sent mail|sent messages)\z/i, "Sent" ],
      [ /\Adrafts\z/i,                                    "Drafts" ],
      [ /\A(trash|deleted items|deleted messages|bin)\z/i, "Trash" ],
      [ /\A(junk|spam|bulk mail)\z/i,                     "Spam" ],
      [ /\A(archive|archives)\z/i,                        "Archive" ]
    ].freeze

    def initialize(email_account)
      @email_account = email_account
    end

    # --- Connection lifecycle ---

    # Closes the IMAP session. The sync strategy calls this in ensure; direct job
    # callers rely on process teardown instead (acceptable for short-lived workers).
    def disconnect
      @imap_conn&.logout
    rescue StandardError
      nil
    ensure
      begin
        @imap_conn&.disconnect
      rescue StandardError
        nil
      end
      drop_state
    end

    # Connect-time credential check: proves the IMAP login works (STATUS on
    # INBOX exercises auth + mailbox access) and that the SMTP endpoint accepts
    # the same credentials, WITHOUT sending or changing anything. Raises
    # PermanentAuthError / AuthenticationError / HostGuard::BlockedError /
    # network errors for the caller to translate into form feedback.
    def verify!
      SystemHealth.track(service: "imap", operation: "verify", workspace_id: workspace_id) do
        imap.status("INBOX", %w[UIDVALIDITY])
        verify_smtp!
        true
      end
    ensure
      disconnect
    end

    # --- Folder model ---

    def list_folders
      SystemHealth.track(service: "imap", operation: "list_folders", workspace_id: workspace_id) do
        with_reconnect { fetch_folders }
      end
    end

    def inbox_folder_id
      @inbox_folder_id ||= folder_id_by_display("Inbox") ||
        raise("Could not find Inbox folder for #{@email_account.email_address}")
    end

    def archive_folder_id
      @archive_folder_id ||= folder_id_by_display("Archive")
    end

    def snoozed_folder_id
      @snoozed_folder_id ||= folder_id_by_display("Snoozed")
    end

    def drafts_folder_id
      @drafts_folder_id ||= folder_id_by_display("Drafts")
    end

    def create_folder(name)
      SystemHealth.track(service: "imap", operation: "create_folder", workspace_id: workspace_id) do
        with_reconnect do
          encoded = Net::IMAP.encode_utf7(name)
          imap.create(encoded)
          clear_folder_cache
          { "folderId" => encoded, "folderName" => name }
        end
      end
    end

    def update_folder(folder_id, new_name)
      SystemHealth.track(service: "imap", operation: "update_folder", workspace_id: workspace_id) do
        with_reconnect do
          encoded = Net::IMAP.encode_utf7(new_name)
          imap.rename(folder_id, encoded)
          clear_folder_cache
          { "folderId" => encoded, "folderName" => new_name }
        end
      end
    end

    # --- Sync-facing fetch API ---

    def folder_status(folder_id)
      SystemHealth.track(service: "imap", operation: "folder_status", workspace_id: workspace_id) do
        with_reconnect do
          # status() works on an unselected mailbox — no EXAMINE needed.
          result = imap.status(folder_id, %w[UIDVALIDITY UIDNEXT])
          { uidvalidity: result["UIDVALIDITY"], uidnext: result["UIDNEXT"] }
        end
      end
    end

    def fetch_new_messages(folder_id, from_uid:, batch: 500) # rubocop:disable Lint/UnusedMethodArgument
      SystemHealth.track(service: "imap", operation: "fetch_new_messages", workspace_id: workspace_id) do
        with_reconnect do
          select_folder(folder_id, readonly: true)
          data = imap.uid_fetch("#{from_uid}:*", FETCH_ITEMS)
          next [] if data.nil? || data.empty?

          # n:* ALWAYS returns at least the highest-UID message even when nothing
          # new exists (classic IMAP gotcha). The sync strategy's uidnext check is
          # not enough on its own — filter here too.
          data.select { |fd| fd.attr["UID"] >= from_uid }
              .map { |fd| normalize_message(fd, folder_id) }
        end
      end
    end

    def fetch_window(folder_id, since: nil, batch: 500)
      SystemHealth.track(service: "imap", operation: "fetch_window", workspace_id: workspace_id) do
        with_reconnect do
          select_folder(folder_id, readonly: true)
          uids = since ? imap.uid_search([ "SINCE", since.to_date ]) : imap.uid_search([ "ALL" ])
          next [] if uids.nil? || uids.empty?

          # Newest first so incremental backfill shows recent mail earliest.
          uids.sort.reverse.each_slice(batch).flat_map do |slice|
            fetched = imap.uid_fetch(slice, FETCH_ITEMS)
            next [] if fetched.nil?
            fetched.map { |fd| normalize_message(fd, folder_id) }
          end
        end
      end
    end

    # --- Content & attachments ---

    def get_message_content(message_id, folder_id)
      SystemHealth.track(service: "imap", operation: "get_message_content", workspace_id: workspace_id) do
        with_reconnect do
          parsed = raw_mail(message_id, folder_id)
          next nil if parsed.nil?

          html = decode_html_part(parsed)
          next html if html.present?

          text = decode_text_part(parsed)
          next nil if text.nil?

          "<div style=\"white-space: pre-wrap;\">#{ERB::Util.html_escape(text)}</div>"
        end
      end
    end

    def list_message_attachments(message_id, folder_id)
      SystemHealth.track(service: "imap", operation: "list_message_attachments", workspace_id: workspace_id) do
        with_reconnect do
          parsed = raw_mail(message_id, folder_id)
          next [] if parsed.nil?

          # For a non-multipart message, treat the whole message as part index 0
          # when it has attachment qualities (disposition or Content-ID).
          parts = if parsed.multipart?
            parsed.all_parts
          elsif parsed.attachment? || parsed.content_id.present?
            [ parsed ]
          else
            []
          end

          parts.each_with_index.filter_map do |part, idx|
            # Skip multipart container nodes; only leaf parts carry content.
            next if part.content_type&.start_with?("multipart/")

            cid = strip_angle_brackets(part.content_id)
            # A Content-ID alone doesn't make a part inline: Apple Mail stamps one
            # on ordinary PDF attachments, and "inline" here means "not a real
            # document" to the ingester. Only an inline disposition, or a cid'd
            # image (the body-embedded case), counts.
            is_inline = part.content_disposition.to_s.match?(/\Ainline/i) ||
                        (cid.present? && part.mime_type.to_s.start_with?("image/"))
            next unless part.attachment? || cid.present?

            filename = part.filename.presence || "attachment_#{idx}"
            size = begin
              part.decoded.bytesize
            rescue StandardError
              0
            end

            {
              "attachmentId"   => idx.to_s,
              "attachmentName" => filename,
              "fileName"       => filename,
              "mimeType"       => part.mime_type,
              "contentId"      => cid.presence,
              "attachmentType" => is_inline ? "inline" : nil,
              "size"           => size
            }
          end
        end
      end
    end

    def download_attachment(message_id, folder_id, attachment_id)
      SystemHealth.track(service: "imap", operation: "download_attachment", workspace_id: workspace_id) do
        with_reconnect do
          parsed = raw_mail(message_id, folder_id)
          next nil if parsed.nil?

          parts = parsed.multipart? ? parsed.all_parts : [ parsed ]
          part  = parts[attachment_id.to_i]
          next nil if part.nil?

          begin
            part.decoded
          rescue Mail::UnknownEncodingType, Encoding::UndefinedConversionError
            part.body.raw_source
          end
        end
      end
    end

    def download_inline_image(message_id, folder_id, content_id)
      SystemHealth.track(service: "imap", operation: "download_inline_image", workspace_id: workspace_id) do
        with_reconnect do
          parsed = raw_mail(message_id, folder_id)
          next nil if parsed.nil?

          stripped = strip_angle_brackets(content_id)
          parts    = parsed.multipart? ? parsed.all_parts : [ parsed ]
          part     = parts.find do |p|
            pcid = strip_angle_brackets(p.content_id)
            pcid == stripped || pcid == content_id.to_s
          end
          next nil if part.nil?

          part.decoded
        end
      end
    end

    # --- Ops ---

    def mark_read(ids)
      SystemHealth.track(service: "imap", operation: "mark_read", workspace_id: workspace_id) do
        apply_flag_change(ids, "+FLAGS", [ :Seen ])
        true
      end
    end

    def mark_unread(ids)
      SystemHealth.track(service: "imap", operation: "mark_unread", workspace_id: workspace_id) do
        apply_flag_change(ids, "-FLAGS", [ :Seen ])
        true
      end
    end

    def move_to_folder(ids, folder_id)
      SystemHealth.track(service: "imap", operation: "move_to_folder", workspace_id: workspace_id) do
        with_reconnect { perform_move(ids, folder_id) }
        true
      end
    end

    def trash_messages(ids)
      SystemHealth.track(service: "imap", operation: "trash_messages", workspace_id: workspace_id) do
        dest = ensure_trash_folder_id
        with_reconnect { perform_move(ids, dest) }
        true
      end
    end

    # Never hard-expunges user mail; moves to Trash for recoverability.
    def delete_messages(ids)
      SystemHealth.track(service: "imap", operation: "delete_messages", workspace_id: workspace_id) do
        dest = ensure_trash_folder_id
        with_reconnect { perform_move(ids, dest) }
        true
      end
    end

    def archive_messages(ids)
      SystemHealth.track(service: "imap", operation: "archive_messages", workspace_id: workspace_id) do
        return false unless archive_folder_id

        with_reconnect { perform_move(ids, archive_folder_id) }
        true
      end
    end

    # --- Outbound ---

    def send_message(subject:, body:, to_address:, cc_address: nil, attachments: [])
      SystemHealth.track(service: "smtp", operation: "user_send", workspace_id: workspace_id) do
        Imap::HostGuard.validate!(@email_account.smtp_host)

        mail = build_outbound_mail(
          subject: subject, body: body, to_address: to_address,
          cc_address: cc_address, attachments: attachments
        )
        deliver_smtp(mail)
        append_to_folder(mail.to_s, sent_folder_id, [ :Seen ]) if sent_folder_id

        { "messageId" => strip_angle_brackets(mail.message_id) }
      end
    end

    def save_draft(subject:, body:, to_address: nil, cc_address: nil,
                  in_reply_to_message_id: nil, attachments: [])
      SystemHealth.track(service: "imap", operation: "save_draft", workspace_id: workspace_id) do
        ensure_drafts_folder

        mail = build_outbound_mail(
          subject: subject, body: body, to_address: to_address,
          cc_address: cc_address, attachments: attachments,
          in_reply_to_message_id: in_reply_to_message_id
        )
        with_reconnect { imap.append(drafts_folder_id, mail.to_s, [ :Draft, :Seen ]) }

        { "messageId" => strip_angle_brackets(mail.message_id) }
      end
    end

    def update_draft(draft_message_id, subject:, body:)
      SystemHealth.track(service: "imap", operation: "update_draft", workspace_id: workspace_id) do
        ensure_drafts_folder

        with_reconnect do
          # Resolve the old UID before appending so we have a stable target to delete.
          old_uid = resolve_uid(drafts_folder_id, draft_message_id)

          replacement = build_outbound_mail(subject: subject, body: body)
          # Carry the same Message-ID so the draft_message_id remains stable.
          replacement.message_id = "<#{draft_message_id}>"

          imap.append(drafts_folder_id, replacement.to_s, [ :Draft, :Seen ])

          expunge_draft(old_uid, draft_message_id) if old_uid
        end

        { "messageId" => draft_message_id }
      end
    end

    def send_draft(draft_message_id)
      SystemHealth.track(service: "smtp", operation: "user_send", workspace_id: workspace_id) do
        Imap::HostGuard.validate!(@email_account.smtp_host)
        ensure_drafts_folder

        raw = with_reconnect { fetch_raw_by_id(drafts_folder_id, draft_message_id) }
        if raw.nil?
          # Claiming success for a draft the server no longer has would silently
          # swallow the user's reply — surface it as a failure instead.
          Rails.logger.warn("[Imap::MailClient] send_draft: draft #{draft_message_id} not found in Drafts for #{@email_account.email_address}")
          next nil
        end

        mail = Mail.read_from_string(raw)
        deliver_smtp(mail)
        append_to_folder(mail.to_s, sent_folder_id, [ :Seen ]) if sent_folder_id

        with_reconnect do
          uid = resolve_uid(drafts_folder_id, draft_message_id)
          expunge_draft(uid, draft_message_id) if uid
        end

        { "messageId" => draft_message_id }
      end
    end

    def forward_message(message_id, to_address, note: nil)
      SystemHealth.track(service: "smtp", operation: "user_send", workspace_id: workspace_id) do
        Imap::HostGuard.validate!(@email_account.smtp_host)

        orig_folder = @email_account.email_messages
          .find_by(provider_message_id: message_id)&.provider_folder_id
        orig = orig_folder ? with_reconnect { raw_mail(message_id, orig_folder) } : nil

        orig_html  = orig ? decode_html_part(orig) : nil
        orig_text  = orig ? decode_text_part(orig) : nil
        body_parts = [ note.presence, "<hr>" ]
        body_parts << if orig_html.present?
          orig_html
        elsif orig_text.present?
          "<div style=\"white-space: pre-wrap;\">#{ERB::Util.html_escape(orig_text)}</div>"
        end
        forward_body = body_parts.compact.join("\n")
        fwd_subject  = "Fwd: #{decode_mime(orig&.subject)}"

        fwd_atts = orig ? collect_attachments_for_forward(orig) : []

        mail = build_outbound_mail(
          subject: fwd_subject, body: forward_body,
          to_address: to_address, attachments: fwd_atts
        )
        deliver_smtp(mail)
        append_to_folder(mail.to_s, sent_folder_id, [ :Seen ]) if sent_folder_id

        { "messageId" => strip_angle_brackets(mail.message_id) }
      end
    end

    private

    # --- Connection ---

    def imap
      @imap_conn ||= open_imap_connection
    end

    def open_imap_connection
      Imap::HostGuard.validate!(@email_account.imap_host)

      conn = case @email_account.imap_security
      when "ssl"
        Net::IMAP.new(@email_account.imap_host, port: @email_account.imap_port,
                      ssl: true, open_timeout: 15)
      when "starttls"
        c = Net::IMAP.new(@email_account.imap_host, port: @email_account.imap_port,
                          open_timeout: 15)
        c.starttls
        c
      else
        Net::IMAP.new(@email_account.imap_host, port: @email_account.imap_port,
                      open_timeout: 15)
      end

      begin
        authenticate!(conn)
      rescue StandardError
        # Auth failed before the connection was memoized — close the raw socket
        # here or nothing ever will (disconnect only knows about @imap_conn).
        begin
          conn.disconnect
        rescue StandardError
          nil
        end
        raise
      end
      conn
    end

    def authenticate!(conn)
      username = @email_account.imap_login_username
      password = @email_account.imap_password

      plain_capable = begin
        conn.auth_capable?("PLAIN")
      rescue NoMethodError
        # Older net-imap: fall back to the CAPABILITY command.
        Array(conn.capability).include?("AUTH=PLAIN")
      end

      login_disabled = begin
        conn.capable?("LOGINDISABLED")
      rescue NoMethodError
        Array(conn.capability).include?("LOGINDISABLED")
      end

      if plain_capable
        conn.authenticate("PLAIN", username, password)
      elsif !login_disabled
        conn.login(username, password)
      else
        raise AuthenticationError,
              "IMAP server has disabled all plaintext auth and AUTH=PLAIN is unavailable for #{username}"
      end
    rescue Net::IMAP::NoResponseError => e
      code_name = e.response.data.code&.name
      if code_name == "AUTHENTICATIONFAILED"
        # AUTHENTICATIONFAILED means the credentials are permanently wrong —
        # raise PermanentAuthError so the job discards and the account is deactivated.
        # Never include the password in the message.
        raise PermanentAuthError,
              "IMAP authentication failed for #{@email_account.imap_login_username}"
      else
        raise AuthenticationError,
              "IMAP connection rejected: #{code_name || e.class.name}"
      end
    rescue Net::IMAP::BadResponseError => e
      raise AuthenticationError, "IMAP BAD response during auth: #{e.class.name}"
    end

    def select_folder(name, readonly:)
      return if @selected_folder == name && @selected_readonly == readonly

      if readonly
        imap.examine(name)
      else
        imap.select(name)
      end
      @selected_folder = name
      @selected_readonly = readonly
    end

    # Transparent single reconnect. Clears all connection + selection state so
    # the next imap call re-opens and the next select_folder re-selects.
    def with_reconnect
      yield
    rescue *RECONNECT_ERRORS
      drop_connection
      yield
    end

    def drop_connection
      @imap_conn&.logout
    rescue StandardError
      nil
    ensure
      begin
        @imap_conn&.disconnect
      rescue StandardError
        nil
      end
      @imap_conn = nil
      @selected_folder = nil
      @selected_readonly = nil
    end

    # Clears all state reset on a clean disconnect (does not clear caches that
    # remain valid, like uid_cache or raw_mail_cache).
    def drop_state
      @imap_conn      = nil
      @selected_folder  = nil
      @selected_readonly = nil
    end

    # --- Folder helpers ---

    def fetch_folders
      @folders_cache ||= begin
        mailboxes = imap.list("", "*") || []
        mailboxes.filter_map do |mbox|
          attrs = Array(mbox.attr)
          next if (attrs & SKIP_ATTRS).any?

          display = canonicalize_folder_name(mbox.name, attrs, mbox.delim)
          { "folderId" => mbox.name, "folderName" => display }
        end
      end
    end

    def folders_by_display
      @folders_by_display ||= fetch_folders.each_with_object({}) do |f, h|
        # First occurrence wins; some servers advertise INBOX with multiple casings.
        h[f["folderName"]] ||= f
      end
    end

    def folder_id_by_display(display_name)
      folders_by_display[display_name]&.dig("folderId")
    end

    def canonicalize_folder_name(raw_name, attrs, delim)
      # INBOX is case-insensitive in IMAP; always display as "Inbox".
      return "Inbox" if raw_name.casecmp?("INBOX")

      # RFC 6154 special-use attributes take priority over name matching.
      SPECIAL_USE_MAP.each { |attr, display| return display if attrs.include?(attr) }

      # Fall back to the last path segment for name-based detection.
      last_seg = if delim && raw_name.include?(delim)
        Net::IMAP.decode_utf7(raw_name.split(delim).last.to_s)
      else
        Net::IMAP.decode_utf7(raw_name)
      end

      NAME_PATTERN_MAP.each { |(pat, display)| return display if last_seg.match?(pat) }

      Net::IMAP.decode_utf7(raw_name)
    end

    def sent_folder_id
      @sent_folder_id ||= folder_id_by_display("Sent")
    end

    def trash_folder_id
      @trash_folder_id ||= folder_id_by_display("Trash")
    end

    def ensure_trash_folder_id
      return trash_folder_id if trash_folder_id

      with_reconnect { imap.create("Trash") }
      clear_folder_cache
      @trash_folder_id = "Trash"
    end

    def ensure_drafts_folder
      return if drafts_folder_id

      with_reconnect { imap.create("Drafts") }
      clear_folder_cache
      @drafts_folder_id = "Drafts"
    end

    def clear_folder_cache
      @folders_cache      = nil
      @folders_by_display = nil
      @inbox_folder_id    = nil
      @archive_folder_id  = nil
      @snoozed_folder_id  = nil
      @drafts_folder_id   = nil
      @sent_folder_id     = nil
      @trash_folder_id    = nil
    end

    # --- Message identity ---

    def normalize_message_id(raw)
      return nil if raw.nil?

      s = raw.strip
      # Strip exactly one enclosing <> pair if both are present.
      (s.start_with?("<") && s.end_with?(">")) ? s[1..-2] : s
    end

    def resolve_uid(folder_id, message_id)
      # noid- IDs are built from envelope data, not a Message-ID header;
      # they cannot be found via HEADER search.
      return nil if message_id.to_s.start_with?("noid-")

      @uid_cache ||= {}
      key = [ folder_id, message_id ]
      return @uid_cache[key] if @uid_cache.key?(key)

      # Cap the cache to ~100 entries to bound memory on long-lived instances.
      @uid_cache.shift if @uid_cache.size >= 100

      # If the folder is already selected in any mode (read-write from a caller
      # like apply_flag_change), keep that mode — uid_search works in both SELECT
      # and EXAMINE state and we must not silently downgrade write access to
      # read-only mid-operation.
      select_folder(folder_id, readonly: true) unless @selected_folder == folder_id
      results = imap.uid_search([ "HEADER", "Message-ID", message_id ])
      @uid_cache[key] = results&.last
    end

    # --- Normalization ---

    def normalize_message(fetch_data, folder_id)
      attrs   = fetch_data.attr
      env     = attrs["ENVELOPE"]
      flags   = attrs["FLAGS"] || []
      indate  = attrs["INTERNALDATE"]

      # BODY.PEEK[HEADER.FIELDS ...] loses .PEEK in the response key; locate it
      # by prefix so the exact field-list order does not matter.
      hdr_key = attrs.keys.find { |k| k.start_with?("BODY[HEADER.FIELDS") }
      headers = hdr_key ? parse_message_headers(attrs[hdr_key].to_s) : {}

      raw_mid = headers["message-id"]
      mid = if raw_mid.present?
        normalize_message_id(raw_mid)
      else
        build_noid(env)
      end

      {
        "messageId"               => mid,
        "folderId"                => folder_id,
        "fromAddress"             => format_addresses(env&.from),
        "toAddress"               => format_addresses(env&.to),
        "subject"                 => decode_mime(env&.subject),
        "summary"                 => nil,
        # hasAttachment MUST be a string; Zoho's convention — boolean drift
        # caused a real ingestion bug on Gmail (GitHub issue #316).
        "hasAttachment"           => attachment_in_structure?(attrs["BODYSTRUCTURE"]) ? "1" : "0",
        # to_s first: net-imap has historically returned INTERNALDATE as a String
        # but newer parsers may hand back a Time; accept both.
        "receivedTime"            => indate ? Time.parse(indate.to_s).to_i * 1000 : nil,
        "status"                  => flags.include?(:Seen) ? "1" : "0",
        "providerThreadId"        => nil,
        "header_list_unsubscribe" => headers["list-unsubscribe"],
        "header_precedence"       => headers["precedence"],
        "header_auto_submitted"   => headers["auto-submitted"],
        "header_campbooks_kind"   => headers["x-campbooks-kind"]
        # No "providerLabels" key — its presence triggers label-snapshot churn
        # in MessageUpserter#reconcile even when the value is empty.
      }
    end

    def build_noid(env)
      "noid-" + Digest::SHA256.hexdigest(
        [
          env&.date.to_s,
          format_address(env&.from&.first),
          format_address(env&.to&.first),
          decode_mime(env&.subject)
        ].join("\x1f")
      )[0, 40]
    end

    def parse_message_headers(raw)
      # Unfold RFC 5322 continuation lines (line starting with whitespace).
      unfolded = raw.to_s.gsub(/\r?\n[ \t]+/, " ")
      headers  = {}
      # chomp: true strips CR LF from each line so that `\z` in the regex anchors
      # correctly (without it, the trailing \r before \n defeats `.*\z`).
      unfolded.each_line(chomp: true) do |line|
        next unless (m = line.match(/\A([\w-]+):\s*(.*)\z/))

        # First occurrence wins for duplicate header names.
        headers[m[1].downcase] ||= m[2].strip
      end
      headers
    end

    def decode_mime(str)
      return nil if str.nil?

      # Mail::Encodings.value_decode may return the same frozen string when no
      # encoding is present (e.g. plain ASCII). .dup unfreezes it so that
      # force_encoding succeeds in environments with frozen string literals.
      Mail::Encodings.value_decode(str.to_s).dup.force_encoding("UTF-8").scrub
    end

    def format_addresses(addrs)
      return nil if addrs.nil? || addrs.empty?

      addrs.filter_map { |a| format_address(a) }.join(", ").presence
    end

    def format_address(addr)
      # Group-syntax entries have a nil mailbox or host — skip them.
      return nil if addr.nil? || addr.mailbox.nil? || addr.host.nil?

      email = "#{addr.mailbox}@#{addr.host}"
      addr.name.present? ? "#{decode_mime(addr.name)} <#{email}>" : email
    end

    def attachment_in_structure?(structure)
      return false if structure.nil?

      if structure.respond_to?(:parts) && structure.parts
        structure.parts.any? { |p| attachment_in_structure?(p) }
      else
        media    = structure.media_type.to_s.upcase
        sub      = structure.subtype.to_s.upcase
        params   = structure.param || {}
        filename = params["NAME"]
        dsp      = structure.disposition
        dsp_type = dsp&.dsp_type&.upcase
        dsp_file = dsp&.param&.dig("FILENAME")
        cid      = structure.content_id

        # Inline images are embedded in the HTML body (referenced via cid:) and
        # are NOT real attachments for the hasAttachment flag.
        return false if cid.present? && dsp_type != "ATTACHMENT"

        # Plain text/html parts without a filename or ATTACHMENT disposition are
        # the message body, not attachments.
        body_part = %w[TEXT].include?(media) && %w[PLAIN HTML].include?(sub) &&
                    filename.nil? && dsp_file.nil? && dsp_type != "ATTACHMENT"
        return false if body_part

        dsp_type == "ATTACHMENT" || filename.present? || dsp_file.present?
      end
    end

    # --- Raw mail / LRU cache ---

    # Fetches and parses the raw RFC822 message, with a 4-entry LRU cache keyed
    # by [folder_id, message_id]. Must be called from within a with_reconnect block.
    def raw_mail(message_id, folder_id)
      key = [ folder_id, message_id ]
      @raw_mail_cache ||= {}

      if @raw_mail_cache.key?(key)
        # LRU: delete + re-insert moves the entry to the most-recently-used end.
        val = @raw_mail_cache.delete(key)
        @raw_mail_cache[key] = val
        return val
      end

      uid = resolve_uid(folder_id, message_id)
      return nil if uid.nil?

      select_folder(folder_id, readonly: true)
      data = imap.uid_fetch(uid, "BODY.PEEK[]")
      return nil if data.nil? || data.empty?

      raw = data.first.attr["BODY[]"]
      return nil if raw.nil?

      parsed = Mail.read_from_string(raw)
      @raw_mail_cache.shift if @raw_mail_cache.size >= 4
      @raw_mail_cache[key] = parsed
      parsed
    end

    # Fetches the raw RFC822 string without caching (used for send_draft).
    def fetch_raw_by_id(folder_id, message_id)
      uid = resolve_uid(folder_id, message_id)
      return nil if uid.nil?

      select_folder(folder_id, readonly: true)
      data = imap.uid_fetch(uid, "BODY.PEEK[]")
      return nil if data.nil? || data.empty?

      data.first.attr["BODY[]"]
    end

    def decode_html_part(parsed)
      if parsed.multipart?
        part = parsed.html_part
        return nil if part.nil?
        part.decoded
      elsif parsed.mime_type == "text/html"
        parsed.decoded
      end
    rescue Mail::UnknownEncodingType, Encoding::UndefinedConversionError
      src = parsed.multipart? ? parsed.html_part : parsed
      src&.body&.raw_source&.force_encoding("UTF-8")&.scrub
    end

    def decode_text_part(parsed)
      if parsed.multipart?
        part = parsed.text_part
        return nil if part.nil?
        part.decoded
      elsif parsed.mime_type == "text/plain"
        parsed.decoded
      end
    rescue Mail::UnknownEncodingType, Encoding::UndefinedConversionError
      src = parsed.multipart? ? parsed.text_part : parsed
      src&.body&.raw_source&.force_encoding("UTF-8")&.scrub
    end

    # --- Ops internals ---

    def apply_flag_change(ids, op, flags)
      by_folder = @email_account.email_messages
        .where(provider_message_id: ids)
        .pluck(:provider_message_id, :provider_folder_id)
        .group_by(&:last)

      by_folder.each do |folder_id, pairs|
        msg_ids = pairs.map(&:first)
        with_reconnect do
          select_folder(folder_id, readonly: false)
          uids = msg_ids.filter_map { |mid| resolve_uid(folder_id, mid) }
          next if uids.empty?

          imap.uid_store(uids, op, flags)
        end
      end
    end

    def perform_move(ids, dest_folder_id)
      by_src = @email_account.email_messages
        .where(provider_message_id: ids)
        .pluck(:provider_message_id, :provider_folder_id)
        .group_by(&:last)

      # Check capabilities once; uid_move is preferred, copy+delete otherwise.
      move_cap   = imap_capable?("MOVE")
      uidplus_cap = imap_capable?("UIDPLUS")

      by_src.each do |src_folder, pairs|
        src_ids = pairs.map(&:first)
        select_folder(src_folder, readonly: false)
        uids = src_ids.filter_map { |mid| resolve_uid(src_folder, mid) }
        next if uids.empty?

        if move_cap
          imap.uid_move(uids, dest_folder_id)
        else
          imap.uid_copy(uids, dest_folder_id)
          imap.uid_store(uids, "+FLAGS", [ :Deleted ])
          # UIDPLUS lets us expunge only the UIDs we just flagged; plain EXPUNGE
          # flushes all \Deleted in the currently selected mailbox.
          if uidplus_cap
            imap.uid_expunge(uids)
          else
            imap.expunge
          end
        end
      end
    end

    def imap_capable?(cap)
      imap.capable?(cap)
    rescue NoMethodError
      # Older net-imap without #capable? — send a CAPABILITY command.
      Array(imap.capability).include?(cap)
    end

    # Deletes one draft by UID. UIDPLUS lets us expunge just that UID; plain
    # EXPUNGE flushes every \Deleted message in Drafts, which could take other
    # drafts the user flagged elsewhere with it. The uid cache entry must go too:
    # update_draft re-appends the SAME Message-ID, so a cached (folder, id) -> old
    # expunged UID would make a follow-up send_draft fetch nothing and no-op.
    def expunge_draft(uid, draft_message_id)
      select_folder(drafts_folder_id, readonly: false)
      imap.uid_store(uid, "+FLAGS", [ :Deleted ])
      if imap_capable?("UIDPLUS")
        imap.uid_expunge(uid)
      else
        imap.expunge
      end
      @uid_cache&.delete([ drafts_folder_id, draft_message_id ])
    end

    # --- SMTP helpers ---

    def build_outbound_mail(subject:, body:, to_address: nil, cc_address: nil,
                            attachments: [], in_reply_to_message_id: nil)
      mail = Mail.new
      mail.from    = @email_account.email_address
      mail.to      = to_address if to_address.present?
      mail.cc      = cc_address if cc_address.present?
      mail.subject = subject

      domain = @email_account.email_address.to_s.split("@").last.presence || "localhost"
      mail.message_id = "<#{SecureRandom.uuid}@#{domain}>"

      if in_reply_to_message_id.present?
        irt = in_reply_to_message_id.to_s
        irt = "<#{irt}>" unless irt.start_with?("<")
        mail.in_reply_to = irt
      end

      # Capture body in a differently-named local so it is unambiguous inside
      # the html_part block (which is instance_eval'd on the part object, where
      # `body` would otherwise resolve to Mail::Part#body).
      html_src = body

      if attachments.present?
        mail.html_part do
          content_type "text/html; charset=UTF-8"
          self.body = html_src
        end
        attachments.each do |att|
          filename = (att[:filename] || att["filename"]).to_s
          content  = att[:data] || att["data"]
          ctype    = (att[:content_type] || att["content_type"]).presence || "application/octet-stream"
          safe     = filename.gsub(/["\r\n]/, "").presence || "attachment"
          mail.attachments[safe] = { content: content.to_s, mime_type: ctype }
        end
      else
        mail.content_type = "text/html; charset=UTF-8"
        mail.body = html_src
      end

      mail
    end

    # Opens and closes an authenticated SMTP session without sending anything.
    # Same auth fallback as deliver_smtp so a LOGIN-only server verifies too.
    def verify_smtp!(auth_method: :plain)
      Imap::HostGuard.validate!(@email_account.smtp_host)

      smtp = Net::SMTP.new(@email_account.smtp_host, @email_account.smtp_port)
      case @email_account.smtp_security
      when "ssl"      then smtp.enable_tls
      when "starttls" then smtp.enable_starttls
      end
      smtp.open_timeout = 15
      helo = @email_account.email_address.to_s.split("@").last.presence || "localhost"

      smtp.start(helo, @email_account.imap_login_username, @email_account.imap_password, auth_method) { true }
    rescue Net::SMTPAuthenticationError
      raise PermanentAuthError, "SMTP authentication failed for #{@email_account.imap_login_username}" if auth_method == :login

      verify_smtp!(auth_method: :login)
    end

    def deliver_smtp(mail, auth_method: :plain)
      # A fresh session per attempt: after a failed AUTH the Net::SMTP instance
      # may hold a half-open socket, and restarting it would leak that socket.
      smtp = Net::SMTP.new(@email_account.smtp_host, @email_account.smtp_port)
      case @email_account.smtp_security
      when "ssl"      then smtp.enable_tls
      when "starttls" then smtp.enable_starttls
      end
      smtp.open_timeout = 15

      recipients = Array(mail.destinations).compact
      helo = @email_account.email_address.to_s.split("@").last.presence || "localhost"

      smtp.start(helo, @email_account.imap_login_username, @email_account.imap_password, auth_method) do |s|
        s.send_message(mail.to_s, @email_account.email_address, recipients)
      end
    rescue Net::SMTPAuthenticationError
      # Some servers reject AUTH PLAIN; retry once with LOGIN before giving up.
      raise PermanentAuthError, "SMTP authentication failed for #{@email_account.imap_login_username}" if auth_method == :login

      deliver_smtp(mail, auth_method: :login)
    end

    def append_to_folder(raw_message, folder_id, flags = [])
      with_reconnect { imap.append(folder_id, raw_message, flags) }
    rescue StandardError
      # APPEND failure is non-fatal: the message is already sent. Skip silently.
      nil
    end

    def strip_angle_brackets(str)
      return nil if str.nil?

      str.to_s.strip.delete_prefix("<").delete_suffix(">")
    end

    def collect_attachments_for_forward(mail_obj)
      return [] unless mail_obj.multipart?

      mail_obj.all_parts.filter_map do |part|
        next if part.content_type&.start_with?("multipart/")
        next unless part.attachment?

        decoded = begin
          part.decoded
        rescue StandardError
          part.body.raw_source
        end

        {
          filename:     part.filename.presence || "attachment",
          content_type: part.mime_type || "application/octet-stream",
          data:         decoded
        }
      end
    end

    def workspace_id
      @email_account.workspace_id
    end
  end
end
