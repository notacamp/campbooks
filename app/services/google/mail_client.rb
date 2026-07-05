module Google
  class MailClient
    BASE_URL = "https://gmail.googleapis.com/gmail/v1/users/me"

    def initialize(email_account)
      @email_account = email_account
      @oauth = OauthClient.new(refresh_token: email_account.refresh_token)
      @next_page_token = nil
    end

    # --- Message listing ---

    def list_messages_with_attachments(folder_id: nil, limit: 50)
      list_messages(folder_id: folder_id, limit: limit, has_attachment: true)
    end

    def list_messages(folder_id: nil, limit: 200, start: nil, has_attachment: nil, received_time_before: nil, skip_known: false)
      # Reset page token when starting a fresh listing (start == 0 or nil)
      @next_page_token = nil if start.to_i == 0

      query_parts = []
      query_parts << "has:attachment" if has_attachment
      query_parts << "before:#{Time.at(received_time_before.to_i / 1000).strftime('%Y/%m/%d')}" if received_time_before

      params = { maxResults: limit }
      params[:labelIds] = folder_id if folder_id.present?
      params[:q] = query_parts.join(" ") if query_parts.any?

      if start.to_i > 0 && @next_page_token.nil?
        return []
      end

      params[:pageToken] = @next_page_token if @next_page_token

      response = connection.get("#{BASE_URL}/messages", params)

      unless response.success?
        Rails.logger.error("[Google::MailClient] list_messages failed: #{response.status} #{response.body[0..300]}")
        return []
      end

      data = JSON.parse(response.body)

      if data["error"]
        Rails.logger.error("[Google::MailClient] list_messages API error: #{data['error'].inspect}")
        return []
      end

      messages = data["messages"] || []
      @next_page_token = data["nextPageToken"]

      # Gmail's list endpoint returns only {id, threadId}; hydrating each message's
      # headers is a separate GET per message (~200/page). On the every-minute
      # incremental inbox poll (skip_known) we drop IDs we've already stored before
      # hydrating, so a no-new-mail scan costs one list call instead of ~200 gets.
      # New mail (newest-first, so it's the unseen block at the top) is still
      # hydrated; read/flag reconciliation for existing mail rides the 15-minute
      # full sweep, which passes skip_known: false.
      if skip_known && messages.any?
        ids = messages.map { |m| m["id"] }
        known = @email_account.email_messages.where(provider_message_id: ids).pluck(:provider_message_id).to_set
        messages = messages.reject { |m| known.include?(m["id"]) }
      end

      messages.map { |msg| fetch_message_metadata(msg["id"], msg["threadId"]) }.compact
    end

    # --- Incremental delta (users.history.list) ---

    # Mailbox-wide change feed since `start_history_id`. Gmail's history covers
    # every label in one call, so this replaces walking every folder. Returns
    # { changed_ids:, deleted_ids:, history_id: } — changed_ids are messages that
    # were added or had labels change (re-fetch via #fetch_messages for current
    # state), history_id is the new cursor. Raises Emails::CursorExpired on HTTP
    # 404 (the start id aged out of Gmail's ~1-week history window → full resync).
    def list_history(start_history_id:)
      changed = []
      deleted = []
      latest = start_history_id
      page_token = nil

      loop do
        response = connection.get("#{BASE_URL}/history") do |req|
          req.params["startHistoryId"] = start_history_id
          req.params["maxResults"] = 500
          req.params["pageToken"] = page_token if page_token
        end

        raise Emails::CursorExpired, "Gmail historyId #{start_history_id} expired" if response.status == 404
        unless response.success?
          Rails.logger.error("[Google::MailClient] list_history failed: #{response.status} #{response.body[0..300]}")
          break
        end

        data = JSON.parse(response.body)
        latest = data["historyId"] if data["historyId"].present?

        (data["history"] || []).each do |h|
          (h["messagesAdded"]   || []).each { |m| changed << m.dig("message", "id") }
          (h["labelsAdded"]     || []).each { |m| changed << m.dig("message", "id") }
          (h["labelsRemoved"]   || []).each { |m| changed << m.dig("message", "id") }
          (h["messagesDeleted"] || []).each { |m| deleted << m.dig("message", "id") }
        end

        page_token = data["nextPageToken"]
        break if page_token.blank?
      end

      deleted_set = deleted.compact.to_set
      {
        changed_ids: changed.compact.uniq.reject { |id| deleted_set.include?(id) },
        deleted_ids: deleted_set.to_a,
        history_id: latest
      }
    end

    # The mailbox's current historyId (users.getProfile). Used to baseline the
    # cursor after a full resync so the next incremental pull starts from "now".
    def current_history_id
      response = connection.get("#{BASE_URL}/profile")
      return nil unless response.success?

      JSON.parse(response.body)["historyId"]
    end

    # Current normalized metadata for specific message IDs, for the delta strategy
    # to hydrate changed messages. Drops any that fail to fetch (e.g. deleted
    # between the history read and now).
    def fetch_messages(message_ids)
      Array(message_ids).filter_map { |id| fetch_message_metadata(id, nil) }
    end

    # --- Message content ---

    def get_message_content(message_id, folder_id = nil)
      response = connection.get("#{BASE_URL}/messages/#{message_id}", { format: "full" })
      data = JSON.parse(response.body)
      extract_html_body(data["payload"])
    end

    def list_message_attachments(message_id, folder_id = nil)
      response = connection.get("#{BASE_URL}/messages/#{message_id}", { format: "full" })
      data = JSON.parse(response.body)
      extract_attachment_parts(data["payload"])
    end

    def download_attachment(message_id, folder_id, attachment_id)
      response = connection.get("#{BASE_URL}/messages/#{message_id}/attachments/#{attachment_id}")
      data = JSON.parse(response.body)
      return nil unless data["data"]

      Base64.urlsafe_decode64(data["data"])
    rescue => e
      Rails.logger.error("[Google::MailClient] Attachment download failed: #{e.message}")
      nil
    end

    def download_inline_image(message_id, folder_id, content_id)
      response = connection.get("#{BASE_URL}/messages/#{message_id}", { format: "full" })
      data = JSON.parse(response.body)
      part = find_part_by_content_id(data["payload"], content_id)

      return nil unless part && part.dig("body", "attachmentId")

      raw = download_attachment(message_id, folder_id, part.dig("body", "attachmentId"))
      raw
    rescue => e
      Rails.logger.error("[Google::MailClient] Inline image download failed: #{e.message}")
      nil
    end

    # --- Labels (folders) ---

    FOLDER_SKIP_LABELS = %w[UNREAD STARRED IMPORTANT CHAT SNOOZED].freeze

    FOLDER_NAME_MAP = {
      "INBOX" => "Inbox",
      "SENT"  => "Sent",
      "DRAFT" => "Drafts",
      "SPAM"  => "Spam",
      "TRASH" => "Trash"
    }.freeze

    def list_folders
      labels = list_labels_raw
      labels.reject { |l| FOLDER_SKIP_LABELS.include?(l["id"]) }.map do |label|
        name = FOLDER_NAME_MAP[label["id"]] || label["name"]
        {
          "folderId" => label["id"],
          "folderName" => name,
          "name" => name
        }
      end
    end

    def normalize_folder_name(label_id)
      FOLDER_NAME_MAP[label_id] || label_id
    end

    def inbox_folder_id
      "INBOX"
    end

    def drafts_folder_id
      "DRAFT"
    end

    # --- Labels CRUD ---

    def list_labels
      list_labels_raw
    end

    def create_label(name:, color: nil)
      body = {
        name: name,
        labelListVisibility: "labelShow",
        messageListVisibility: "show"
      }
      body[:color] = hex_to_gmail_color(color) if color

      response = connection.post("#{BASE_URL}/labels") do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = body.to_json
      end
      JSON.parse(response.body)
    end

    def update_label(label_id, name:, color: nil)
      body = { name: name }
      body[:color] = hex_to_gmail_color(color) if color

      response = connection.put("#{BASE_URL}/labels/#{label_id}") do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = body.to_json
      end
      JSON.parse(response.body)
    end

    def delete_label(label_id)
      response = connection.delete("#{BASE_URL}/labels/#{label_id}")
      { "status" => { "code" => response.success? ? 200 : 500 } }
    rescue => e
      { "status" => { "code" => 500, "description" => e.message } }
    end

    # --- Label modification on messages ---

    def apply_labels_to_message(message_id, label_ids)
      response = connection.post("#{BASE_URL}/messages/#{message_id}/modify") do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = { addLabelIds: Array(label_ids).map(&:to_s) }.to_json
      end
      JSON.parse(response.body)
    end

    def remove_labels_from_message(message_id, label_ids)
      response = connection.post("#{BASE_URL}/messages/#{message_id}/modify") do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = { removeLabelIds: Array(label_ids).map(&:to_s) }.to_json
      end
      JSON.parse(response.body)
    end

    def snooze_messages(message_ids)
      Array(message_ids).each do |mid|
        apply_labels_to_message(mid, [ "SNOOZED" ])
        remove_labels_from_message(mid, [ "INBOX" ])
      end
    end

    def unsnooze_messages(message_ids)
      Array(message_ids).each do |mid|
        apply_labels_to_message(mid, [ "INBOX" ])
        remove_labels_from_message(mid, [ "SNOOZED" ])
      end
    end

    def snoozed_folder_id
      "SNOOZED"
    end

    def list_messages_by_label(label_id, limit: 200, start: nil)
      list_messages(folder_id: label_id, limit: limit, start: start)
    end

    # --- Read/unread ---

    def mark_read(message_ids)
      Array(message_ids).each do |mid|
        response = connection.post("#{BASE_URL}/messages/#{mid}/modify") do |req|
          req.headers["Content-Type"] = "application/json"
          req.body = { removeLabelIds: [ "UNREAD" ] }.to_json
        end
        raise "Google mark_read failed for #{mid}: #{response.status}" unless response.success?
      end
      true
    rescue => e
      Rails.logger.error("[Google::MailClient] mark_read failed: #{e.message}")
      nil
    end

    def mark_unread(message_ids)
      Array(message_ids).each do |mid|
        response = connection.post("#{BASE_URL}/messages/#{mid}/modify") do |req|
          req.headers["Content-Type"] = "application/json"
          req.body = { addLabelIds: [ "UNREAD" ] }.to_json
        end
        raise "Google mark_unread failed for #{mid}: #{response.status}" unless response.success?
      end
      true
    rescue => e
      Rails.logger.error("[Google::MailClient] mark_unread failed: #{e.message}")
      nil
    end

    # --- Drafts API ---

    def save_draft(subject:, body:, to_address: nil, cc_address: nil, in_reply_to_message_id: nil, attachments: [])
      raw = build_rfc2822(subject: subject, body: body, to_address: to_address, cc_address: cc_address, in_reply_to: in_reply_to_message_id, attachments: attachments)
      response = connection.post("#{BASE_URL}/drafts") do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = { message: { raw: raw } }.to_json
      end
      data = JSON.parse(response.body)
      { "messageId" => data.dig("message", "id"), "id" => data["id"] }
    rescue => e
      Rails.logger.error("[Google::MailClient] save_draft failed: #{e.message}")
      nil
    end

    def update_draft(draft_message_id, subject:, body:)
      raw = build_rfc2822(subject: subject, body: body)
      response = connection.put("#{BASE_URL}/drafts/#{draft_message_id}") do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = { message: { raw: raw } }.to_json
      end
      data = JSON.parse(response.body)
      { "messageId" => data.dig("message", "id"), "id" => data["id"] }
    rescue => e
      Rails.logger.error("[Google::MailClient] update_draft failed: #{e.message}")
      nil
    end

    def send_draft(draft_message_id)
      response = connection.post("#{BASE_URL}/drafts/send") do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = { id: draft_message_id }.to_json
      end
      response.success? ? { "status" => { "code" => 200 } } : nil
    rescue => e
      Rails.logger.error("[Google::MailClient] send_draft failed: #{e.message}")
      nil
    end

    # Builds the raw RFC 2822 message Gmail's API expects (base64url). With no
    # attachments it's a flat text/html part (as before); with attachments it
    # becomes a multipart/mixed envelope — the HTML body plus one base64 part per
    # file. `attachments` is [{ filename:, content_type:, data: <raw bytes> }].
    def build_rfc2822(subject:, body:, to_address: nil, cc_address: nil, in_reply_to: nil, attachments: [])
      headers = +"From: #{@email_account.email_address}\r\n"
      headers << "To: #{to_address}\r\n" if to_address.present?
      headers << "Cc: #{cc_address}\r\n" if cc_address.present?
      headers << "Subject: #{subject}\r\n"
      headers << "In-Reply-To: #{in_reply_to}\r\n" if in_reply_to.present?
      headers << "MIME-Version: 1.0\r\n"

      if attachments.present?
        boundary = "cb_boundary_#{SecureRandom.hex(16)}"
        headers << "Content-Type: multipart/mixed; boundary=\"#{boundary}\"\r\n\r\n"
        msg = headers
        msg << "--#{boundary}\r\n"
        msg << "Content-Type: text/html; charset=UTF-8\r\n\r\n#{body}\r\n"
        attachments.each do |att|
          name = mime_filename(att[:filename] || att["filename"])
          ctype = (att[:content_type] || att["content_type"]).presence || "application/octet-stream"
          encoded = Base64.strict_encode64((att[:data] || att["data"]).to_s).scan(/.{1,76}/).join("\r\n")
          msg << "--#{boundary}\r\n"
          msg << "Content-Type: #{ctype}; name=\"#{name}\"\r\n"
          msg << "Content-Transfer-Encoding: base64\r\n"
          msg << "Content-Disposition: attachment; filename=\"#{name}\"\r\n\r\n"
          msg << "#{encoded}\r\n"
        end
        msg << "--#{boundary}--"
        Base64.urlsafe_encode64(msg)
      else
        headers << "Content-Type: text/html; charset=UTF-8\r\n\r\n#{body}"
        Base64.urlsafe_encode64(headers)
      end
    end

    # Strip characters that would break the MIME header / allow header injection.
    def mime_filename(name)
      name.to_s.gsub(/["\r\n]/, "").presence || "attachment"
    end

    # --- Pagination ---

    def more_messages?
      @next_page_token.present?
    end

    # --- Folder / label operations used by Tools ---

    def archive_folder_id
      "ARCHIVE"
    end

    def move_to_folder(message_ids, folder_id)
      Array(message_ids).each do |mid|
        case folder_id
        when "INBOX"
          apply_labels_to_message(mid, [ "INBOX" ])
        when "ARCHIVE"
          remove_labels_from_message(mid, [ "INBOX" ])
        else
          apply_labels_to_message(mid, [ folder_id ])
          remove_labels_from_message(mid, [ "INBOX" ])
        end
      end
    end

    def trash_messages(message_ids)
      Array(message_ids).each do |mid|
        apply_labels_to_message(mid, [ "TRASH" ])
        remove_labels_from_message(mid, [ "INBOX" ])
      end
    end

    # --- Forward ---

    def forward_message(message_id, to_address, note: nil)
      response = connection.get("#{BASE_URL}/messages/#{message_id}", { format: "full" })
      data = JSON.parse(response.body)

      headers = parse_headers(data["payload"])
      original_subject = (headers["Subject"] || "").force_encoding("UTF-8")
      original_from = (headers["From"] || "").force_encoding("UTF-8")
      original_date = (headers["Date"] || "").force_encoding("UTF-8")
      original_to = (headers["To"] || @email_account.email_address).force_encoding("UTF-8")

      html_body = extract_html_body(data["payload"])
      html_body = html_body.force_encoding("UTF-8") if html_body
      snippet = ERB::Util.html_escape(data["snippet"] || "")

      forward_subject = original_subject.match?(/^Fwd:/i) ? original_subject : "Fwd: #{original_subject}"

      note_html = note.present? ? "<div>#{ERB::Util.html_escape(note)}</div><br>" : ""

      forward_body = <<~HTML
        #{note_html}<div>---------- Forwarded message ---------</div>
        <div><b>From:</b> #{ERB::Util.html_escape(original_from)}</div>
        <div><b>Date:</b> #{ERB::Util.html_escape(original_date)}</div>
        <div><b>Subject:</b> #{ERB::Util.html_escape(original_subject)}</div>
        <div><b>To:</b> #{ERB::Util.html_escape(original_to)}</div>
        <br>
        #{html_body || "<div>#{snippet}</div>"}
      HTML

      send_message(subject: forward_subject, body: forward_body, to_address: to_address)
    end

    # --- Send ---

    def send_message(subject:, body:, to_address:, cc_address: nil, attachments: [])
      raw = build_rfc2822(subject: subject, body: body, to_address: to_address, cc_address: cc_address, attachments: attachments)
      response = connection.post("#{BASE_URL}/messages/send") do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = { raw: raw }.to_json
      end
      JSON.parse(response.body)
    rescue => e
      Rails.logger.error("[Google::MailClient] send_message failed: #{e.message}")
      nil
    end

    private

    # Fetches metadata for a single message and normalizes to Zoho-compatible hash
    def fetch_message_metadata(message_id, thread_id)
      response = connection.get("#{BASE_URL}/messages/#{message_id}") do |req|
        req.params["format"] = "metadata"
        # The last three are bulk/automated-mail signals consumed by
        # Emails::Categorizer to keep newsletters and machine mail out of "personal".
        req.params["metadataHeaders"] = [ "From", "Subject", "To", "Content-Type",
                                          "List-Unsubscribe", "Precedence", "Auto-Submitted" ]
      end
      data = JSON.parse(response.body)

      headers = parse_headers(data["payload"])
      labels = data["labelIds"] || []
      is_unread = labels.include?("UNREAD")

      folder_id = (%w[INBOX SENT DRAFT TRASH SPAM] & labels).first || labels.first || "INBOX"

      {
        "messageId" => data["id"],
        "providerThreadId" => thread_id || data["threadId"],
        "folderId" => folder_id,
        # Raw Gmail label ids — persisted on the message so triage can read
        # Gmail's category verdicts (EmailMessage#provider_category_hint).
        "providerLabels" => labels,
        "fromAddress" => headers["From"] || headers["from"],
        "toAddress" => headers["To"] || headers["to"],
        "subject" => headers["Subject"] || headers["subject"],
        "summary" => data["snippet"],
        "hasAttachment" => has_attachment?(data["payload"]),
        "receivedTime" => data["internalDate"],
        "status" => is_unread ? "0" : "1",  # "1" = read (matches Zoho convention)
        "header_list_unsubscribe" => headers["List-Unsubscribe"],
        "header_precedence" => headers["Precedence"],
        "header_auto_submitted" => headers["Auto-Submitted"],
        "flagid" => nil
      }
    rescue => e
      Rails.logger.error("[Google::MailClient] Metadata fetch failed for #{message_id}: #{e.message}")
      nil
    end

    def parse_headers(payload)
      return {} unless payload && payload["headers"]

      headers = {}
      payload["headers"].each do |h|
        headers[h["name"]] = h["value"]
      end
      headers
    end

    def has_attachment?(payload)
      return false unless payload

      parts = payload["parts"] || []
      parts.any? { |p| p["filename"].present? && p["filename"] != "" }
    end

    def extract_html_body(payload)
      if payload["mimeType"] == "text/html" && payload.dig("body", "data")
        return Base64.urlsafe_decode64(payload.dig("body", "data"))
      end

      parts = payload["parts"] || []
      html_part = find_part_by_mime(parts, "text/html")
      return nil unless html_part

      data = html_part.dig("body", "data")
      return Base64.urlsafe_decode64(data) if data

      # Check nested multipart
      nested = html_part["parts"] || []
      nested_html = find_part_by_mime(nested, "text/html")
      nested_html ? Base64.urlsafe_decode64(nested_html.dig("body", "data") || "") : nil
    rescue => e
      Rails.logger.error("[Google::MailClient] HTML body extraction failed: #{e.message}")
      nil
    end

    def extract_attachment_parts(payload)
      attachments = []
      collect_parts(payload) do |part|
        if part["filename"].present? && part["filename"] != "" && part.dig("body", "attachmentId")
          attachments << {
            "attachmentId" => part.dig("body", "attachmentId"),
            "attachmentName" => part["filename"],
            "fileName" => part["filename"],
            "mimeType" => part["mimeType"],
            "size" => part.dig("body", "size"),
            "contentId" => part.dig("headers")&.find { |h| h["name"] == "Content-ID" }&.dig("value")&.gsub(/[<>]/, ""),
            "attachmentType" => part.dig("headers")&.any? { |h| h["name"] == "Content-Disposition" && h["value"].include?("inline") } ? "inline" : "attachment"
          }
        end
      end
      attachments
    end

    def find_part_by_mime(parts, mime_type)
      parts.find { |p| p["mimeType"] == mime_type } ||
        parts.flat_map { |p| p["parts"] || [] }.find { |p| p["mimeType"] == mime_type }
    end

    def find_part_by_content_id(payload, content_id)
      return nil unless payload

      found = nil
      collect_parts(payload) do |part|
        cid_header = part.dig("headers")&.find { |h| h["name"]&.downcase == "content-id" }
        if cid_header && cid_header["value"]&.gsub(/[<>]/, "") == content_id
          found = part
        end
      end
      found
    end

    def collect_parts(payload, &block)
      parts = payload["parts"] || []
      parts.each do |part|
        yield part
        nested = part["parts"] || []
        nested.each { |np| yield np }
      end
    end

    def system_label?(label_id)
      %w[INBOX SENT DRAFT TRASH SPAM IMPORTANT STARRED UNREAD CATEGORY_PERSONAL
         CATEGORY_SOCIAL CATEGORY_PROMOTIONS CATEGORY_UPDATES CATEGORY_FORUMS
         CHAT SNOOZED].include?(label_id)
    end

    def list_labels_raw
      response = connection.get("#{BASE_URL}/labels")

      unless response.success?
        Rails.logger.error("[Google::MailClient] list_labels failed: #{response.status} #{response.body[0..300]}")
        return []
      end

      data = JSON.parse(response.body)

      if data["error"]
        Rails.logger.error("[Google::MailClient] list_labels API error: #{data['error'].inspect}")
        return []
      end

      data["labels"] || []
    end

    def hex_to_gmail_color(hex)
      hex = hex.gsub("#", "")
      {
        textColor: hex_to_rgb(hex[0..5]),
        backgroundColor: hex_to_rgb(hex[0..5])
      }
    end

    def hex_to_rgb(hex)
      r = hex[0..1].to_i(16) / 255.0
      g = hex[2..3].to_i(16) / 255.0
      b = hex[4..5].to_i(16) / 255.0
      { red: r, green: g, blue: b }
    end

    def connection
      # Rebuilt per call so @oauth.access_token is re-read each request. Memoizing
      # froze a stale token into the Authorization header, 401-ing mid-scan.
      Faraday.new do |f|
        f.use SystemHealth::FaradayMiddleware, service: "google_mail"
        f.request :url_encoded
        # Bound every call so a slow/hung provider response can't pin a scan open
        # for minutes (see Zoho::MailClient#connection).
        f.options.open_timeout = 10
        f.options.timeout = 30
        f.adapter Faraday.default_adapter
        f.headers["Authorization"] = "Bearer #{@oauth.access_token}"
      end
    end
  end
end
