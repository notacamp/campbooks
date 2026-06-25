# frozen_string_literal: true

module Emails
  # Scans an email body for direct-file links, downloads each one safely, and
  # creates a Document (source: :email) for every link that passes all safety
  # gates — so they flow through the same downstream analysis as real attachments.
  #
  # Safety design (URLs come from attacker-influenced inbound email):
  #   - http/https only, every URL routed through Workflows::UrlGuard (SSRF guard).
  #   - Allowlisted by response Content-Type (primary) and URL extension (fallback).
  #   - Size cap: links whose Content-Length header exceeds MAX_FILE_BYTES are
  #     skipped without downloading; downloaded bodies that exceed the cap are
  #     discarded.
  #   - Per-email link cap: at most MAX_LINKS_PER_EMAIL links are processed.
  #   - Per-link isolation: a failure on one link (blocked, timeout, 4xx/5xx)
  #     never affects the others or the parent job.
  #   - Deduplication: a URL that duplicates an existing attachment (same SHA-256
  #     content hash) is linked rather than re-created. Re-running the job for the
  #     same email skips already-created documents (content_hash guard).
  #
  # Cloud-share resolution (Google Drive / Dropbox / WeTransfer share links) is
  # explicitly out of scope for this iteration.
  class DocumentLinkIngester
    MAX_LINKS_PER_EMAIL = 10
    MAX_FILE_BYTES      = 25 * 1024 * 1024  # 25 MB
    DOWNLOAD_TIMEOUT    = 30                  # seconds

    # Content-Types that represent actionable documents. Images are intentionally
    # excluded: they are almost always tracking pixels or decorative assets.
    ALLOWED_CONTENT_TYPES = %w[
      application/pdf
      application/msword
      application/vnd.openxmlformats-officedocument.wordprocessingml.document
      application/vnd.ms-excel
      application/vnd.openxmlformats-officedocument.spreadsheetml.sheet
      application/vnd.ms-powerpoint
      application/vnd.openxmlformats-officedocument.presentationml.presentation
      text/csv
      text/plain
    ].freeze

    # File extensions used as a fallback when the server returns a generic or
    # absent Content-Type (e.g. application/octet-stream).
    DOCUMENT_EXTENSIONS = %w[
      .pdf .doc .docx .xls .xlsx .ppt .pptx .csv .txt
    ].freeze

    # Cloud-share hostnames whose links look like direct downloads but are not.
    # We skip them gracefully rather than producing garbage documents.
    CLOUD_SHARE_DOMAINS = %w[
      drive.google.com docs.google.com sheets.google.com
      dropbox.com www.dropbox.com
      we.tl wetransfer.com www.wetransfer.com
      1drv.ms onedrive.live.com sharepoint.com
    ].freeze

    def initialize(email)
      @email = email
    end

    # Returns the number of Documents created.
    def call
      return 0 if @email.body.blank?

      links = extract_candidate_links(@email.body)
      return 0 if links.empty?

      count = 0
      links.first(MAX_LINKS_PER_EMAIL).each do |url|
        created = ingest_link(url)
        count += 1 if created
      rescue => e
        Rails.logger.warn("[DocumentLinkIngester] Skipping link #{url.inspect} for email #{@email.id}: #{e.message}")
      end

      Rails.logger.info("[DocumentLinkIngester] Created #{count} document(s) from links for email #{@email.id}")
      count
    end

    private

    # Pull every href/src value that looks like a direct-file URL from the body.
    # We match both href= and src= attributes to cover <a> and embedded objects.
    def extract_candidate_links(body)
      urls = []
      body.scan(/(?:href|src)=["']([^"']+)["']/i) { urls << $1 }
      # Also pick up bare http/https URLs in plain-text bodies.
      body.scan(%r{(https?://[^\s"'<>]+)}) { urls << Regexp.last_match(1) }

      urls
        .map(&:strip)
        .uniq
        .select { |url| url.match?(%r{\Ahttps?://}i) }
        .reject { |url| cloud_share_url?(url) }
        .select { |url| document_url_candidate?(url) }
    end

    def cloud_share_url?(url)
      host = URI.parse(url).host.to_s.downcase
      CLOUD_SHARE_DOMAINS.any? { |d| host == d || host.end_with?(".#{d}") }
    rescue URI::InvalidURIError
      false
    end

    # Cheap pre-filter: skip URLs with no document-like extension unless the URL
    # looks like a raw download endpoint (e.g. query-string-only paths still pass
    # here; Content-Type check is the definitive gate at download time).
    def document_url_candidate?(url)
      path = URI.parse(url).path.to_s.downcase
      ext  = File.extname(path)
      ext.blank? || DOCUMENT_EXTENSIONS.include?(ext)
    rescue URI::InvalidURIError
      false
    end

    # Attempts to download and create a Document for `url`. Returns true if a new
    # Document was created, false/nil if skipped (dedup, size, type, blocked, etc.).
    def ingest_link(url)
      # SSRF guard — raises Workflows::UrlGuard::BlockedError for private/internal URLs.
      Workflows::UrlGuard.validate!(url)

      result = Workflows::HttpClient.call(
        method:  :get,
        url:     url,
        headers: { "User-Agent" => "Campbooks/1.0" },
        timeout: DOWNLOAD_TIMEOUT
      )

      unless result[:ok]
        Rails.logger.info("[DocumentLinkIngester] Download failed for #{url}: #{result[:error] || result[:status]}")
        return false
      end

      content_type = result.dig(:headers, "content-type").to_s.split(";").first.strip.downcase
      body_bytes   = result[:body].to_s

      # Content-Type gate: accept only document types; fall back to extension check
      # when the server returns a generic binary content type.
      unless allowed_content_type?(content_type, url)
        Rails.logger.info("[DocumentLinkIngester] Skipping non-document content type #{content_type.inspect} for #{url}")
        return false
      end

      if body_bytes.bytesize > MAX_FILE_BYTES
        Rails.logger.info("[DocumentLinkIngester] Skipping oversized download (#{body_bytes.bytesize} bytes) for #{url}")
        return false
      end

      if body_bytes.blank?
        Rails.logger.info("[DocumentLinkIngester] Empty body for #{url}")
        return false
      end

      content_hash = Digest::SHA256.hexdigest(body_bytes)

      # Dedup against attachments from the same email account.
      existing = Document.find_by(content_hash: content_hash, email_account_id: @email.email_account_id)
      if existing
        existing.document_email_messages.find_or_create_by!(email_message: @email)
        Rails.logger.info("[DocumentLinkIngester] Deduped link document #{existing.id} for email #{@email.id}")
        return false
      end

      filename     = filename_from(url, content_type)
      resolved_ct  = resolve_content_type(content_type, filename)

      document = Document.new(
        source:          :email,
        ai_status:       :pending,
        review_status:   :pending,
        document_type:   :other,
        workspace:       @email.email_account.workspace,
        email_account:   @email.email_account,
        email_message_id: @email.provider_message_id,
        content_hash:    content_hash,
        sender_name:     @email.from_address
      )
      document.original_file.attach(
        io:           StringIO.new(body_bytes),
        filename:     filename,
        content_type: resolved_ct
      )
      document.save!
      document.document_email_messages.create!(email_message: @email)
      DocumentProcessJob.perform_later(document.id)

      Rails.logger.info("[DocumentLinkIngester] Created document #{document.id} from link for email #{@email.id}")
      true
    end

    def allowed_content_type?(content_type, url)
      return true if ALLOWED_CONTENT_TYPES.include?(content_type)

      # Accept a generic binary type when the URL extension is a known document type.
      if %w[application/octet-stream binary/octet-stream].include?(content_type)
        path = URI.parse(url).path.to_s.downcase rescue ""
        return DOCUMENT_EXTENSIONS.include?(File.extname(path))
      end

      false
    end

    def filename_from(url, content_type)
      path = URI.parse(url).path.to_s rescue ""
      name = File.basename(path).presence || "document"
      # Strip query strings that may have leaked into the basename.
      name = name.split("?").first.presence || "document"
      # Ensure a sensible extension when the server-side path has none.
      if File.extname(name).blank?
        name += extension_for(content_type)
      end
      name
    end

    def extension_for(content_type)
      {
        "application/pdf"          => ".pdf",
        "application/msword"       => ".doc",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => ".docx",
        "application/vnd.ms-excel" => ".xls",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => ".xlsx",
        "application/vnd.ms-powerpoint" => ".ppt",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => ".pptx",
        "text/csv"   => ".csv",
        "text/plain" => ".txt"
      }.fetch(content_type, "")
    end

    def resolve_content_type(content_type, filename)
      return content_type if ALLOWED_CONTENT_TYPES.include?(content_type)
      # Fallback: derive from extension.
      case File.extname(filename).downcase
      when ".pdf"  then "application/pdf"
      when ".doc"  then "application/msword"
      when ".docx" then "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
      when ".xls"  then "application/vnd.ms-excel"
      when ".xlsx" then "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
      when ".ppt"  then "application/vnd.ms-powerpoint"
      when ".pptx" then "application/vnd.openxmlformats-officedocument.presentationml.presentation"
      when ".csv"  then "text/csv"
      when ".txt"  then "text/plain"
      else "application/octet-stream"
      end
    end
  end
end
