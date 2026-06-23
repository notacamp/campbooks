module Documents
  class TitleGenerator
    PURPOSE = "document_analysis"  # reuse existing AI config

    SYSTEM_PROMPT = <<~PROMPT
      You are a document title assistant. Given metadata about a Portuguese business document, produce a short, descriptive title in Portuguese.

      Rules:
      - Return ONLY the title, no quotes, no extra text, no explanation.
      - Keep it under 100 characters.
      - Include the key entity (company/person name) and what the document is (invoice, receipt, bank entry, contract, etc.).
      - If a date or reference number is available, include it.
      - Examples of good titles:
        "Fatura EDP Comercial — Jan 2026 — €124,50"
        "Millennium BCP — Pagamento transferência — 15/03/2026"
        "Seguro Ageas — Apólice 12345 — 2026"
        "Consilcar — Orçamento matrícula 34-TX-21"
        "IMT — Certidão de matrícula — AA-123-BB"
        "Nota de lançamento — Comissão bancária — Mai 2023"
      - NOT acceptable: just an email address, just a company name, or a raw filename.
      - If the existing title is just a company name or email, you MUST generate a proper one.
    PROMPT

    def initialize(document)
      @document = document
    end

    def call
      return nil unless needs_title?

      config = Ai::Configuration.for(PURPOSE)
      return nil unless config

      title = generate_title(config)
      return nil if title.blank?

      metadata = (@document.metadata.presence || {}).merge("title" => title)
      @document.update_columns(metadata: metadata)
      title
    rescue => e
      Rails.logger.warn("[Documents::TitleGenerator] Failed for doc #{@document.id}: #{e.message}")
      nil
    end

    private

    def needs_title?
      current = @document.metadata&.dig("title") || @document.entity_display_name
      return true if current.blank?
      return true if current.match?(/@/)
      return true if current.match?(/^document_\d+/)  # generic fallback
      return true if current.match?(/\.(pdf|jpe?g|png|docx?)$/i)  # raw filename
      return true if current.split(/\s+/).size <= 1  # single word
      # Has at least one digit AND a document keyword = probably good
      has_digit = current.match?(/\d/)
      has_keyword = current.match?(/[Ff]atura|[Rr]ecibo|[Ii]nvoice|[Rr]eceipt|[Ss]eguro|[Pp]agamento|[Tt]ransfer|[Ee]xtrato|[Cc]ertid|[Oo]rçamento|[Dd]eclara|[Nn]ota/)
      return false if has_digit && has_keyword
      true
    end

    def generate_title(config)
      text = config[:adapter].chat(
        system: SYSTEM_PROMPT,
        messages: [ { role: "user", parts: [ { type: :text, text: build_context } ] } ],
        model: config[:model],
        max_tokens: 80,
        temperature: 0.3
      )
      title = text.to_s.strip
                    .gsub(/\A["']|["']\z/, "")
                    .gsub(/\n+/, " ")
                    .truncate(100)
      title.present? ? title : nil
    end

    def build_context
      parts = []
      parts << "Document type: #{@document.classification&.name || @document.document_type}"
      parts << "Current title/name: #{@document.metadata&.dig('title') || @document.entity_display_name || @document.original_file.filename}"
      parts << "Vendor/emitter: #{@document.vendor_name}" if @document.vendor_name.present?
      parts << "Client: #{@document.client_name}" if @document.client_name.present?
      parts << "Bank: #{@document.bank_name}" if @document.bank_name.present?
      parts << "Date: #{@document.document_date}" if @document.document_date.present?
      parts << "Amount: #{format_amount}" if @document.amount_cents.present?
      parts << "Invoice number: #{@document.invoice_number}" if @document.invoice_number.present?
      parts << "Receipt number: #{@document.receipt_number}" if @document.receipt_number.present?
      parts << "Description: #{@document.description}" if @document.description.present?
      parts << "Account: #{@document.account_number}" if @document.account_number.present?

      parts.join("\n")
    end

    def format_amount
      return nil unless @document.amount_cents
      euros = @document.amount_cents.abs / 100.0
      formatted = format("%.2f", euros).sub(".", ",")
      sign = @document.amount_cents.negative? ? "-" : ""
      "#{sign}€#{formatted}"
    end
  end
end
