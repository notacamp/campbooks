module Accounts
  # Builds the GDPR data-portability archive (a .zip) for a single user: the
  # structured JSON copy of their account (Accounts::DataExporter) PLUS the actual
  # content that JSON only summarises — their email bodies, email attachments, and
  # the workspace's document files. Scoped strictly to what the user may read
  # (EmailMessage.accessible_to). Mirrors Exports::ZipGenerator's in-memory
  # Zip::OutputStream; large mailboxes build in memory, same as the document export.
  class ArchiveGenerator
    def initialize(user)
      @user = user
    end

    def call
      Zip::OutputStream.write_buffer do |zip|
        write_account_json(zip)
        write_emails(zip)
        write_documents(zip)
      end.string
    end

    private

    def write_account_json(zip)
      zip.put_next_entry("account.json")
      zip.write(Accounts::DataExporter.new(@user).to_json)
    end

    def write_emails(zip)
      EmailMessage.accessible_to(@user).includes(:email_account).with_attached_files.find_each do |email|
        box = sanitize(email.email_account&.email_address || "unknown")

        zip.put_next_entry("emails/#{box}/#{email.id}.json")
        zip.write(JSON.pretty_generate(email_hash(email)))

        email.files.each do |file|
          add_blob(zip, file.blob, "attachments/#{box}/#{email.id}")
        end
      end
    end

    def write_documents(zip)
      @user.workspace&.documents&.with_attached_original_file&.find_each do |document|
        add_blob(zip, document.original_file.blob, "documents", prefix: "#{document.id}-")
      end
    end

    # Stream one ActiveStorage blob into the archive. A blob row can outlive its
    # underlying file (e.g. an earlier storage migration) — skip those rather than
    # abort the whole export, so the user still gets everything that survives.
    def add_blob(zip, blob, folder, prefix: "")
      return unless blob

      data = blob.download
      zip.put_next_entry("#{folder}/#{prefix}#{sanitize(blob.filename.to_s)}")
      zip.write(data)
    rescue ActiveStorage::FileNotFoundError => e
      Rails.logger.warn("[ArchiveGenerator] Skipping missing blob #{blob&.id}: #{e.message}")
    end

    def email_hash(email)
      {
        id: email.id,
        subject: email.subject,
        from: email.from_address,
        to: email.try(:to_address),
        received_at: email.received_at&.iso8601,
        body: email.body.to_s
      }
    end

    # Keep archive paths flat + safe: strip path separators from provider-supplied
    # filenames/addresses so nothing escapes its folder.
    def sanitize(name)
      name.to_s.gsub(%r{[/\\]}, "_").presence || "file"
    end
  end
end
