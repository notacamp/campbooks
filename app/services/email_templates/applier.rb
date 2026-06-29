# frozen_string_literal: true

module EmailTemplates
  # Turns an EmailTemplate + filled variables into ready-to-send content: the
  # Liquid-rendered subject and body, plus a PDF for each attached
  # DocumentTemplate (filled with the SAME variables).
  #
  # Two entry points share one PDF-rendering core:
  #   .call           — composer flow: uploads each PDF as a blob owned by `user`
  #                     (user.outbound_attachments) and returns signed_ids, so they
  #                     pass through EmailComposeController#collected_attachments.
  #   .pdf_attachments — scheduled-send flow: returns { filename:, content_type:,
  #                     data: } hashes streamed straight to Emails::Sender, with no
  #                     persisted blobs (each recurring occurrence regenerates).
  class Applier
    # attachments: [{ signed_id:, filename:, size:, content_type: }]
    Result = Data.define(:subject, :body_html, :attachments)

    def self.call(template:, variables:, user:)
      new(template, variables, user).call
    end

    # Render each attached document template to PDF data (no persistence). A single
    # bad template is skipped rather than failing the whole set.
    def self.pdf_attachments(template:, variables:)
      template.document_templates.filter_map do |dt|
        filled = DocumentTemplates::Filler.call(dt.html_content, variables || {})
        pdf = DocumentTemplates::PdfGenerator.call(filled)
        { filename: "#{dt.name.parameterize.presence || 'document'}.pdf", content_type: "application/pdf", data: pdf }
      rescue => e
        Rails.logger.warn("[EmailTemplates::Applier] PDF for document_template ##{dt.id} failed: #{e.message}")
        nil
      end
    end

    def call
      Result.new(
        subject: @template.rendered_subject(@variables),
        body_html: @template.rendered_body(@variables),
        attachments: self.class.pdf_attachments(template: @template, variables: @variables).map { |att| upload(att) }
      )
    end

    private

    def initialize(template, variables, user)
      @template = template
      @variables = variables || {}
      @user = user
    end

    # Stash a rendered PDF as one of the user's outbound attachments and return the
    # signed-id descriptor the composer injects into the open compose form.
    def upload(att)
      blob = ActiveStorage::Blob.create_and_upload!(
        io: StringIO.new(att[:data]), filename: att[:filename], content_type: att[:content_type]
      )
      @user.outbound_attachments.attach(blob)
      { signed_id: blob.signed_id, filename: blob.filename.to_s, size: blob.byte_size, content_type: blob.content_type }
    end
  end
end
