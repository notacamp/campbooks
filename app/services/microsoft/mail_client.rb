module Microsoft
  class MailClient
    BASE_URL = "https://graph.microsoft.com/v1.0"

    # Fields hydrated on every message read. internetMessageHeaders carries the
    # bulk/automated signals (List-Unsubscribe / Precedence / Auto-Submitted) that
    # Emails::Categorizer uses; Graph only returns it when explicitly $select-ed.
    # Shared by #list_messages and #list_messages_delta so they can't drift.
    MESSAGE_SELECT = "id,conversationId,subject,from,toRecipients,bodyPreview,receivedDateTime," \
                     "hasAttachments,isRead,parentFolderId,internetMessageId," \
                     "internetMessageHeaders".freeze

    def initialize(email_account)
      @email_account = email_account
      @oauth = Microsoft::OauthClient.new(refresh_token: email_account.refresh_token)
    end

    def list_messages_with_attachments(folder_id: nil, limit: 50)
      list_messages(folder_id: folder_id, limit: limit, has_attachment: true)
    end

    # skip_known is a Google-only optimization (its list returns bare IDs that need
    # a per-message GET to hydrate). Graph's $select already returns full metadata
    # in one call, so there's nothing to skip — the kwarg is accepted and ignored to
    # keep EmailScanJob's call site uniform across providers.
    def list_messages(folder_id: nil, limit: 200, start: nil, has_attachment: nil, received_time_before: nil, skip_known: false)
      url = folder_id ? "#{BASE_URL}/me/mailFolders/#{folder_id}/messages" : "#{BASE_URL}/me/messages"

      response = connection.get(url) do |req|
        req.params["$top"] = [ limit, 1000 ].min
        req.params["$skip"] = start if start&.positive?
        req.params["$orderby"] = "receivedDateTime desc"
        req.params["$select"] = MESSAGE_SELECT
        if has_attachment
          filter = "hasAttachments eq true"
          filter += " and receivedDateTime lt #{received_time_before}" if received_time_before
          req.params["$filter"] = filter
        elsif received_time_before
          req.params["$filter"] = "receivedDateTime lt #{received_time_before}"
        end
      end

      data = JSON.parse(response.body)
      messages = data["value"] || []
      messages.map { |m| normalize_message(m) }
    end

    # Per-folder incremental delta (Graph /messages/delta). Pass delta_link: nil to
    # bootstrap a folder; thereafter pass the stored deltaLink URL. Drains every
    # @odata.nextLink page, then returns the new @odata.deltaLink to persist as the
    # cursor. Returns { messages:, removed_ids:, delta_link: } — removed_ids are
    # messages Graph reports as deleted/moved-out (@removed). Raises
    # Emails::CursorExpired on HTTP 410 (the deltaToken expired → re-bootstrap).
    def list_messages_delta(folder_id:, delta_link: nil)
      messages = []
      removed = []
      next_delta_link = nil
      url = delta_link

      loop do
        response =
          if url
            connection.get(url) # follow nextLink / stored deltaLink (full URL, carries its own params)
          else
            connection.get("#{BASE_URL}/me/mailFolders/#{folder_id}/messages/delta") do |req|
              req.params["$select"] = MESSAGE_SELECT
            end
          end

        raise Emails::CursorExpired, "Graph delta token expired for folder #{folder_id}" if response.status == 410
        unless response.success?
          Rails.logger.error("[Microsoft::MailClient] list_messages_delta failed: #{response.status} #{response.body[0..300]}")
          break
        end

        data = JSON.parse(response.body)
        (data["value"] || []).each do |m|
          m["@removed"] ? (removed << m["id"]) : (messages << normalize_message(m))
        end

        if (next_link = data["@odata.nextLink"])
          url = next_link
          next
        end

        next_delta_link = data["@odata.deltaLink"]
        break
      end

      { messages: messages, removed_ids: removed.compact, delta_link: next_delta_link }
    end

    def get_message_content(message_id, folder_id)
      url = "#{BASE_URL}/me/messages/#{message_id}"
      response = connection.get(url) do |req|
        req.params["$select"] = "body"
      end
      data = JSON.parse(response.body)
      data.dig("body", "content")
    end

    def list_message_attachments(message_id, folder_id)
      url = "#{BASE_URL}/me/messages/#{message_id}/attachments"
      response = connection.get(url)
      data = JSON.parse(response.body)
      attachments = data["value"] || []
      attachments.map { |att| normalize_attachment(att) }
    end

    def download_attachment(message_id, folder_id, attachment_id)
      url = "#{BASE_URL}/me/messages/#{message_id}/attachments/#{attachment_id}/$value"
      response = connection.get(url)
      response.success? ? response.body : nil
    end

    def download_inline_image(message_id, folder_id, content_id)
      attachments = list_message_attachments(message_id, folder_id)
      inline = attachments.find { |a| a["contentId"] == content_id && a["attachmentType"] == "inline" }
      return nil unless inline

      download_attachment(message_id, folder_id, inline["attachmentId"])
    end

    def list_folders
      url = "#{BASE_URL}/me/mailFolders"
      response = connection.get(url)
      data = JSON.parse(response.body)
      folders = data["value"] || []
      folders.map { |f| normalize_folder(f) }
    end

    def inbox_folder_id
      @inbox_folder_id ||= begin
        url = "#{BASE_URL}/me/mailFolders/inbox"
        response = connection.get(url)
        data = JSON.parse(response.body)
        data["id"] || raise("Could not find Inbox folder for #{@email_account.email_address}")
      end
    end

    def drafts_folder_id
      @drafts_folder_id ||= begin
        url = "#{BASE_URL}/me/mailFolders/drafts"
        response = connection.get(url)
        data = JSON.parse(response.body)
        data["id"] || raise("Could not find Drafts folder for #{@email_account.email_address}")
      end
    end

    # Create a real mail folder at the mailbox root. Returns the Graph folder
    # ({ "id", "displayName", ... }). Used by MailFolders::Provisioner.
    def create_folder(name)
      url = "#{BASE_URL}/me/mailFolders"
      response = connection.post(url) do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = { displayName: name }.to_json
      end
      JSON.parse(response.body)
    end

    # Rename a mail folder (PATCH the new displayName). Used by
    # MailFolders::Provisioner when a user renames a custom folder.
    def update_folder(folder_id, name)
      url = "#{BASE_URL}/me/mailFolders/#{folder_id}"
      connection.patch(url) do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = { displayName: name }.to_json
      end
    end

    # Move messages into a folder. NB: Graph reassigns each message a NEW id in
    # the destination folder, so the locally stored provider_message_id goes stale
    # until the next delta sync reconciles it (acceptable: folder filtering keys on
    # provider_folder_id, which the caller updates). Mirrors Zoho/Google's signature.
    def move_to_folder(message_ids, folder_id)
      Array(message_ids).each do |msg_id|
        url = "#{BASE_URL}/me/messages/#{msg_id}/move"
        response = connection.post(url) do |req|
          req.headers["Content-Type"] = "application/json"
          req.body = { destinationId: folder_id }.to_json
        end
        raise "Microsoft move_to_folder failed for #{msg_id}: #{response.status}" unless response.success?
      end
      true
    rescue => e
      Rails.logger.error("[Microsoft::MailClient] move_to_folder failed: #{e.message}")
      raise
    end

    # --- Labels (Outlook categories) ---

    def list_labels
      url = "#{BASE_URL}/me/outlook/masterCategories"
      response = connection.get(url)
      data = JSON.parse(response.body)
      categories = data["value"] || []
      categories.map { |c| { "labelId" => c["id"], "displayName" => c["displayName"], "color" => c["color"] } }
    end

    def create_label(name:, color:)
      url = "#{BASE_URL}/me/outlook/masterCategories"
      response = connection.post(url) do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = { displayName: name, color: preset_color(color) }.to_json
      end
      JSON.parse(response.body)
    end

    def update_label(label_id, name:, color:)
      url = "#{BASE_URL}/me/outlook/masterCategories/#{label_id}"
      response = connection.patch(url) do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = { displayName: name, color: preset_color(color) }.to_json
      end
      JSON.parse(response.body)
    end

    def delete_label(label_id)
      url = "#{BASE_URL}/me/outlook/masterCategories/#{label_id}"
      response = connection.delete(url)
      response.success?
    rescue => e
      Rails.logger.error("[Microsoft::MailClient] delete_label failed: #{e.message}")
      false
    end

    # --- Read/Unread ---

    def mark_read(message_ids)
      Array(message_ids).each do |msg_id|
        url = "#{BASE_URL}/me/messages/#{msg_id}"
        response = connection.patch(url) do |req|
          req.headers["Content-Type"] = "application/json"
          req.body = { isRead: true }.to_json
        end
        raise "Microsoft mark_read failed for #{msg_id}: #{response.status}" unless response.success?
      end
      true
    rescue => e
      Rails.logger.error("[Microsoft::MailClient] mark_read failed: #{e.message}")
      nil
    end

    def mark_unread(message_ids)
      Array(message_ids).each do |msg_id|
        url = "#{BASE_URL}/me/messages/#{msg_id}"
        response = connection.patch(url) do |req|
          req.headers["Content-Type"] = "application/json"
          req.body = { isRead: false }.to_json
        end
        raise "Microsoft mark_unread failed for #{msg_id}: #{response.status}" unless response.success?
      end
      true
    rescue => e
      Rails.logger.error("[Microsoft::MailClient] mark_unread failed: #{e.message}")
      nil
    end

    # --- Drafts API ---

    def save_draft(subject:, body:, to_address: nil, cc_address: nil, in_reply_to_message_id: nil, attachments: [])
      payload = {
        subject: subject,
        body: { content: body, contentType: "html" },
        isDraft: true
      }
      if to_address.present?
        payload[:toRecipients] = to_address.split(",").map { |addr|
          { emailAddress: { address: addr.strip } }
        }
      end
      if cc_address.present?
        payload[:ccRecipients] = cc_address.split(",").map { |addr|
          { emailAddress: { address: addr.strip } }
        }
      end
      # Inline file attachments (Graph accepts them in the draft-create payload up
      # to ~3 MB total; larger would need an upload session).
      if attachments.present?
        payload[:attachments] = attachments.map do |att|
          {
            "@odata.type" => "#microsoft.graph.fileAttachment",
            name: att[:filename],
            contentType: (att[:content_type].presence || "application/octet-stream"),
            contentBytes: Base64.strict_encode64((att[:data] || att[:content]).to_s)
          }
        end
      end

      url = "#{BASE_URL}/me/messages"
      response = connection.post(url) do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = payload.to_json
      end
      data = JSON.parse(response.body)
      { "messageId" => data["id"], "id" => data["id"], "subject" => data["subject"] }
    rescue => e
      Rails.logger.error("[Microsoft::MailClient] save_draft failed: #{e.message}")
      nil
    end

    def update_draft(draft_message_id, subject:, body:)
      url = "#{BASE_URL}/me/messages/#{draft_message_id}"
      response = connection.patch(url) do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = { subject: subject, body: { content: body, contentType: "html" } }.to_json
      end
      JSON.parse(response.body)
    rescue => e
      Rails.logger.error("[Microsoft::MailClient] update_draft failed: #{e.message}")
      nil
    end

    def send_draft(draft_message_id)
      url = "#{BASE_URL}/me/messages/#{draft_message_id}/send"
      response = connection.post(url)
      response.success? ? { "status" => { "code" => 200 } } : nil
    rescue => e
      Rails.logger.error("[Microsoft::MailClient] send_draft failed: #{e.message}")
      nil
    end

    private

    def normalize_message(msg)
      headers = index_headers(msg["internetMessageHeaders"])
      {
        "messageId" => msg["id"],
        "providerThreadId" => msg["conversationId"],
        "folderId" => msg["parentFolderId"],
        "fromAddress" => msg.dig("from", "emailAddress", "address") || "",
        "toAddress" => Array(msg["toRecipients"]).map { |r| r.dig("emailAddress", "address") }.join(", "),
        "subject" => msg["subject"] || "",
        "summary" => msg["bodyPreview"] || "",
        "receivedTime" => msg["receivedDateTime"] ? (DateTime.parse(msg["receivedDateTime"]).to_i * 1000).to_s : nil,
        "hasAttachment" => msg["hasAttachments"] ? "1" : "0",
        "status" => msg["isRead"] ? "1" : "0",
        "header_list_unsubscribe" => headers["list-unsubscribe"],
        "header_precedence" => headers["precedence"],
        "header_auto_submitted" => headers["auto-submitted"],
        "flagid" => nil
      }
    end

    # Graph's internetMessageHeaders is an array of {name:, value:}; index it by
    # down-cased name so header lookups are case-insensitive.
    def index_headers(list)
      Array(list).each_with_object({}) do |h, acc|
        name = h["name"].to_s.downcase
        acc[name] = h["value"] unless name.empty?
      end
    end

    def normalize_attachment(att)
      file_type = att["@odata.type"] == "#microsoft.graph.fileAttachment" ? nil : "item"
      {
        "attachmentId" => att["id"],
        "attachmentName" => att["name"],
        "fileName" => att["name"],
        "mimeType" => att["contentType"],
        "contentId" => att["contentId"],
        "attachmentType" => att["isInline"] ? "inline" : file_type,
        "size" => att["size"]
      }
    end

    def normalize_folder(folder)
      {
        "folderId" => folder["id"],
        "folderName" => folder["displayName"],
        "name" => folder["displayName"],
        "parentFolderId" => folder["parentFolderId"],
        "childFolderCount" => folder["childFolderCount"]
      }
    end

    def preset_color(hex)
      # Microsoft Graph requires preset color names, not hex values
      Microsoft::MailClient::PRESET_COLORS[hex] || "preset0"
    end

    PRESET_COLORS = {
      "#3b82f6" => "preset0",
      "#ef4444" => "preset1",
      "#f59e0b" => "preset2",
      "#10b981" => "preset3",
      "#8b5cf6" => "preset4",
      "#ec4899" => "preset5",
      "#06b6d4" => "preset6",
      "#f97316" => "preset7"
    }.freeze

    def connection
      # Rebuilt per call so @oauth.access_token is re-read each request. Memoizing
      # froze a stale token into the Authorization header, 401-ing mid-scan.
      Faraday.new do |f|
        f.use SystemHealth::FaradayMiddleware, service: "microsoft_mail", expected_statuses: [ 410 ]
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
