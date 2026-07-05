module Zoho
  class MailClient
    BASE_URL = Region.mail_api_url.freeze

    def initialize(email_account)
      @email_account = email_account
      @oauth = OauthClient.new(refresh_token: email_account.refresh_token)
    end

    def list_messages_with_attachments(folder_id: nil, limit: 50)
      list_messages(folder_id: folder_id, limit: limit, has_attachment: true)
    end

    # skip_known is a Google-only optimization (its list returns bare IDs that
    # need a per-message GET to hydrate). Zoho's view endpoint already returns full
    # metadata in one call, so there's nothing to skip — the kwarg is accepted and
    # ignored to keep EmailScanJob's call site uniform across providers.
    def list_messages(folder_id: nil, limit: 200, start: nil, has_attachment: nil, received_time_before: nil, skip_known: false)
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/messages/view"

      response = connection.get(url) do |req|
        req.params["folderId"] = folder_id if folder_id
        req.params["limit"] = limit
        req.params["start"] = start if start
        req.params["hasAttachment"] = has_attachment unless has_attachment.nil?
        req.params["receivedTime"] = received_time_before if received_time_before
      end

      data = JSON.parse(response.body)
      # NB: this view endpoint returns Zoho's own metadata only — no transport
      # headers (List-Unsubscribe / Precedence / Auto-Submitted), so Zoho mail
      # carries none of the header-based bulk signals Gmail/Microsoft do. The
      # Categorizer falls back to its sender/subject rules for these accounts.
      data.is_a?(Hash) ? (data["data"] || []) : Array(data)
    end

    def get_message_content(message_id, folder_id)
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/folders/#{folder_id}/messages/#{message_id}/content"

      response = connection.get(url)
      data = JSON.parse(response.body)
      data.is_a?(Hash) ? data["data"]&.dig("content") : nil
    end

    def list_message_attachments(message_id, folder_id)
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/folders/#{folder_id}/messages/#{message_id}/attachmentinfo"

      response = connection.get(url)
      data = JSON.parse(response.body)
      if data.is_a?(Hash)
        inner = data["data"]
        if inner.is_a?(Hash)
          Array(inner["attachments"] || inner["attachmentInfo"] || [])
        elsif inner.is_a?(Array)
          inner
        else
          Array(inner)
        end
      else
        Array(data)
      end
    end

    def download_attachment(message_id, folder_id, attachment_id)
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/folders/#{folder_id}/messages/#{message_id}/attachments/#{attachment_id}"

      response = connection.get(url)
      response.body
    end

    def download_inline_image(message_id, folder_id, content_id)
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/folders/#{folder_id}/messages/#{message_id}/inline"

      response = connection.get(url) do |req|
        req.params["contentId"] = content_id
      end
      response.success? ? response.body : nil
    end

    def list_folders
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/folders"
      response = connection.get(url)
      data = JSON.parse(response.body)

      unless response.success? && data.is_a?(Hash) && data["status"]&.dig("code") == 200
        raise "Failed to list folders: #{response.body[0..300]}"
      end

      data["data"] || []
    end

    def inbox_folder_id
      @inbox_folder_id ||= begin
        inbox = list_folders.find { |f| f["folderName"] == "Inbox" }
        inbox&.dig("folderId") || raise("Could not find Inbox folder for #{@email_account.email_address}")
      end
    end

    def archive_folder_id
      @archive_folder_id ||= begin
        archive = list_folders.find { |f| f["folderName"] == "Archive" }
        archive&.dig("folderId")
      end
    end

    def snoozed_folder_id
      @snoozed_folder_id ||= begin
        snoozed = list_folders.find { |f| f["folderName"] == "Snoozed" }
        snoozed&.dig("folderId")
      end
    end

    def archive_messages(message_ids)
      update_message_status(message_ids, "archiveMails")
    end

    # Create a real mail folder. Returns the folder hash ({ "folderId", "folderName" }).
    # Used by MailFolders::Provisioner when a user adds a custom folder.
    def create_folder(name)
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/folders"
      response = connection.post(url) do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = { folderName: name }.to_json
      end
      data = JSON.parse(response.body)
      data.is_a?(Hash) ? data["data"] : data
    end

    # Rename a mail folder (PUT the new name). Used by MailFolders::Provisioner
    # when a user renames a custom folder.
    def update_folder(folder_id, name)
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/folders/#{folder_id}"
      connection.put(url) do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = { folderName: name }.to_json
      end
    end

    # --- Labels API ---

    def list_labels
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/labels"
      response = connection.get(url)
      data = JSON.parse(response.body)
      data.is_a?(Hash) ? (data["data"] || []) : Array(data)
    end

    def create_label(name:, color:)
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/labels"
      response = connection.post(url) do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = { displayName: name, color: color }.to_json
      end
      data = JSON.parse(response.body)
      data.is_a?(Hash) ? data["data"] : data
    end

    def update_label(label_id, name:, color:)
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/labels/#{label_id}"
      response = connection.put(url) do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = { displayName: name, color: color }.to_json
      end
      JSON.parse(response.body)
    end

    def delete_label(label_id)
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/labels/#{label_id}"
      response = connection.delete(url)
      JSON.parse(response.body)
    rescue => e
      { "status" => { "code" => 500, "description" => e.message } }
    end

    def apply_labels_to_message(message_id, label_ids)
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/updatemessage"
      response = connection.put(url) do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = {
          mode: "applyLabel",
          labelId: Array(label_ids).map(&:to_s),
          messageId: Array(message_id)
        }.to_json
      end
      JSON.parse(response.body)
    end

    def remove_labels_from_message(message_id, label_ids)
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/updatemessage"
      response = connection.put(url) do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = {
          mode: "removeLabel",
          labelId: Array(label_ids).map(&:to_s),
          messageId: Array(message_id)
        }.to_json
      end
      JSON.parse(response.body)
    end

    def list_messages_by_label(label_id, limit: 200, start: nil)
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/messages/view"
      response = connection.get(url) do |req|
        req.params["labelId"] = label_id
        req.params["limit"] = limit
        req.params["start"] = start if start
      end
      data = JSON.parse(response.body)
      data.is_a?(Hash) ? (data["data"] || []) : Array(data)
    end

    # --- Read/Unread API ---

    def mark_read(message_ids)
      update_message_status(message_ids, "markRead")
    end

    def mark_unread(message_ids)
      update_message_status(message_ids, "markUnread")
    end

    def update_message_status(message_ids, mode, extra = {})
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/updatemessage"
      response = connection.put(url) do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = {
          mode: mode,
          messageId: Array(message_ids).map(&:to_s)
        }.merge(extra).to_json
      end
      raise "#{mode} failed with status #{response.status}: #{response.body[0..200]}" unless response.success?
      JSON.parse(response.body)
    rescue => e
      Rails.logger.error("[Zoho::MailClient] #{mode} failed: #{e.message}")
      raise
    end

    def move_to_folder(message_ids, folder_id)
      update_message_status(message_ids, "moveMessage", { destfolderId: folder_id.to_s })
    end

    def trash_messages(message_ids)
      update_message_status(message_ids, "moveToTrash")
    end

    def delete_messages(message_ids)
      update_message_status(message_ids, "delete")
    end

    # --- Drafts API ---

    def drafts_folder_id
      @drafts_folder_id ||= begin
        url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/folders"
        response = connection.get(url)
        data = JSON.parse(response.body)
        folders = data.is_a?(Hash) ? (data["data"] || []) : Array(data)
        drafts = folders.find { |f| f["folderName"] == "Drafts" || f["name"] == "Drafts" }
        drafts&.dig("folderId") || raise("Could not find Drafts folder for #{@email_account.email_address}")
      end
    end

    def save_draft(subject:, body:, to_address: nil, cc_address: nil, in_reply_to_message_id: nil)
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/messages"
      payload = {
        fromAddress: @email_account.email_address,
        toAddress: to_address,
        mode: "draft",
        subject: subject,
        content: body,
        mailFormat: "html"
      }
      payload[:ccAddress] = cc_address if cc_address.present?
      payload[:inReplyTo] = in_reply_to_message_id if in_reply_to_message_id.present?

      response = connection.post(url) do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = payload.to_json
      end

      data = JSON.parse(response.body)
      data.is_a?(Hash) ? data["data"] : data
    rescue => e
      Rails.logger.error("[Zoho::MailClient] save_draft failed: #{e.message}")
      nil
    end

    def update_draft(draft_message_id, subject:, body:)
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/messages/#{draft_message_id}"
      payload = { mode: "draft", subject: subject, content: body, mailFormat: "html" }
      response = connection.put(url) do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = payload.to_json
      end
      JSON.parse(response.body)
    rescue => e
      Rails.logger.error("[Zoho::MailClient] update_draft failed: #{e.message}")
      nil
    end

    def send_message(subject:, body:, to_address:, cc_address: nil, attachments: [])
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/messages"
      payload = {
        fromAddress: @email_account.email_address,
        toAddress: to_address,
        mode: "send",
        subject: subject,
        content: body,
        mailFormat: "html"
      }
      payload[:ccAddress] = cc_address if cc_address.present?

      if attachments.any?
        # Zoho requires a two-step flow: upload each file first, then reference
        # the returned storeName/attachmentPath in the send (inline base64 is
        # rejected with EXTRA_KEY_FOUND_IN_JSON).
        refs = attachments.filter_map { |att| upload_attachment(att[:filename], att[:content_type], att[:data]) }
        payload[:attachments] = refs if refs.any?
      end

      response = connection.post(url) do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = payload.to_json
      end
      result = JSON.parse(response.body)
      Rails.logger.info("[Zoho::MailClient] send_message response: status=#{response.status}, body=#{response.body[0..500]}")
      result
    rescue => e
      Rails.logger.error("[Zoho::MailClient] send_message failed: #{e.message}")
      nil
    end

    # Upload one attachment to Zoho and return the reference the send call needs
    # ({ storeName:, attachmentName:, attachmentPath: }), or nil on failure.
    # Must use uploadType=multipart with a part named "attach" — a raw-body
    # upload returns a ref that the send silently ignores (hasAttachment stays 0).
    def upload_attachment(filename, content_type, data)
      boundary = "----Campbooks#{SecureRandom.hex(12)}"
      ctype = content_type.presence || "application/octet-stream"
      safe_name = filename.to_s.gsub(/["\r\n]/, "")

      body = String.new(encoding: Encoding::ASCII_8BIT)
      body << "--#{boundary}\r\n".b
      body << %(Content-Disposition: form-data; name="attach"; filename="#{safe_name}"\r\n).b
      body << "Content-Type: #{ctype}\r\n\r\n".b
      body << data.to_s.b
      body << "\r\n--#{boundary}--\r\n".b

      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/messages/attachments"
      response = connection.post(url) do |req|
        req.params["uploadType"] = "multipart"
        req.headers["Content-Type"] = "multipart/form-data; boundary=#{boundary}"
        req.body = body
      end

      ref = JSON.parse(response.body)["data"]
      ref = ref.first if ref.is_a?(Array)
      return nil unless ref.is_a?(Hash) && ref["storeName"].present?
      {
        storeName: ref["storeName"],
        attachmentName: ref["attachmentName"].presence || filename,
        attachmentPath: ref["attachmentPath"]
      }
    rescue => e
      Rails.logger.error("[Zoho::MailClient] upload_attachment failed: #{e.message}")
      nil
    end

    def send_draft(draft_message_id)
      url = "#{BASE_URL}/accounts/#{@email_account.provider_account_id}/messages/#{draft_message_id}/send"
      response = connection.post(url)
      result = JSON.parse(response.body)
      Rails.logger.info("[Zoho::MailClient] send_draft response: status=#{response.status}, body=#{response.body[0..500]}")
      result
    rescue => e
      Rails.logger.error("[Zoho::MailClient] send_draft failed: #{e.message}")
      nil
    end

    private

    def connection
      # Rebuilt per call so @oauth.access_token is re-read each request. Memoizing
      # froze a stale token into the Authorization header, 401-ing mid-scan.
      Faraday.new do |f|
        f.use SystemHealth::FaradayMiddleware, service: "zoho_mail"
        f.request :url_encoded
        # Bound every call so a slow/hung provider response can't pin a scan (and
        # the "Syncing your inbox" pill) open for minutes — scans have stalled for
        # hours without these, holding the scanning flag the whole time.
        f.options.open_timeout = 10
        f.options.timeout = 30
        f.adapter Faraday.default_adapter
        f.headers["Authorization"] = "Zoho-oauthtoken #{@oauth.access_token}"
      end
    end
  end
end
