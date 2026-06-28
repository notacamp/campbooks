module DocumentTemplates
  # Fills a template's Liquid body with variables, renders it to a PDF, and
  # optionally emails that PDF as an attachment via the shared Emails::Sender.
  #
  # Returns a Result and never raises: a render failure (e.g. Chromium missing)
  # or a send failure both come back as `ok: false` with an error message, so the
  # controller can show a clean flash instead of a 500. With a blank to_address
  # it just renders the PDF (preview mode).
  class Sender
    Result = Data.define(:ok, :pdf, :email_message, :error) do
      def self.success(pdf:, email_message: nil)
        new(ok: true, pdf: pdf, email_message: email_message, error: nil)
      end

      def self.failure(error)
        new(ok: false, pdf: nil, email_message: nil, error: error)
      end
    end

    def self.call(template:, variables: {}, to_address: nil, subject: nil, body: nil,
                  user: nil, email_account_id: nil)
      new(template: template, variables: variables, to_address: to_address, subject: subject,
          body: body, user: user, email_account_id: email_account_id).call
    end

    def initialize(template:, variables: {}, to_address: nil, subject: nil, body: nil,
                   user: nil, email_account_id: nil)
      @template = template
      @variables = variables || {}
      @to_address = to_address
      @subject = subject
      @body = body
      @user = user
      @email_account_id = email_account_id
    end

    def call
      filled = Filler.call(@template.html_content, @variables)
      pdf = PdfGenerator.call(filled)

      return Result.success(pdf: pdf) if @to_address.blank?

      sent = deliver(pdf)
      if sent.ok?
        Result.success(pdf: pdf, email_message: sent.email_message)
      else
        Result.failure(sent.error_code)
      end
    rescue PdfGenerator::PdfGenerationError => e
      Result.failure(e.message)
    end

    private

    def deliver(pdf)
      blob = ActiveStorage::Blob.create_and_upload!(
        io: StringIO.new(pdf),
        filename: "#{@template.name.parameterize}.pdf",
        content_type: "application/pdf"
      )

      Emails::Sender.call(
        user: @user,
        to_address: @to_address,
        subject: @subject.presence || @template.name,
        body: @body.presence || "",
        email_account_id: @email_account_id,
        attachments: [ blob ]
      )
    end
  end
end
