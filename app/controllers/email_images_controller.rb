class EmailImagesController < ApplicationController
  def show
    account = Current.user.readable_email_accounts.find(params[:email_account_id])

    content_id = params[:cid]
    message_id = params[:nmsgId]
    filename = params[:f]

    return head :bad_request unless content_id.present? && message_id.present?

    # This endpoint proxies untrusted email-attachment bytes — including SVG, which
    # can carry <script> and would otherwise execute on our own origin when fetched
    # directly. Stop the browser from sniffing or running them: nosniff + a
    # locked-down CSP (no script, sandboxed) that still lets the image render.
    response.headers["X-Content-Type-Options"] = "nosniff"
    response.headers["Content-Security-Policy"] = "default-src 'none'; style-src 'unsafe-inline'; sandbox"

    # Look up the EmailMessage to get the folder_id
    email = account.email_messages.find_by!(provider_message_id: message_id)
    folder_id = email.provider_folder_id

    raise "No folder_id for email #{email.id}" unless folder_id.present?

    raw = account.mail_client.download_inline_image(message_id, folder_id, content_id)

    if raw.present? && raw.length > 100
      content_type = mime_type_for(filename) || "image/png"
      expires_in 1.hour, public: false
      send_data raw, type: content_type, disposition: :inline
    else
      # Return a transparent 1x1 PNG so broken image icons don't clutter the UI
      pixel = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="
      send_data Base64.decode64(pixel), type: "image/png", disposition: :inline
    end
  end

  private

  def mime_type_for(filename)
    return nil if filename.blank?
    case File.extname(filename.to_s).downcase
    when ".png"  then "image/png"
    when ".jpg", ".jpeg" then "image/jpeg"
    when ".gif"  then "image/gif"
    when ".bmp"  then "image/bmp"
    when ".webp" then "image/webp"
    when ".svg"  then "image/svg+xml"
    end
  end
end
