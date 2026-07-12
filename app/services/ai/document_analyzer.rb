module Ai
  class DocumentAnalyzer
    CONFIDENCE_THRESHOLD = 0.7
    PURPOSE = "document_analysis"

    # Type names the AI sometimes produces that should never create a DocumentType record.
    JUNK_TYPE_NAMES = %w[null none unknown n/a na].freeze

    BASE_PROMPT = <<~PROMPT
      You are a document analysis assistant specialized in Portuguese business documents.

      Portuguese-specific knowledge:
      - NIF (Número de Identificação Fiscal): 9-digit tax identification number
      - IVA (Imposto sobre o Valor Acrescentado): Portuguese VAT
      - Common IVA rates: 23% (normal), 13% (intermediate), 6% (reduced)
      - Invoice types: Fatura (FT), Fatura-Recibo (FR), Nota de Crédito (NC), Recibo (RC)
      - For amounts, convert to cents (e.g., €123.45 = 12345)
      - Extract the NIF carefully — it must be exactly 9 digits
      - If you cannot determine a field with confidence, set it to null

      You MUST respond with valid JSON only, no other text. Use this exact schema:
      {
        "document_type": "string — pick from the existing types listed below. If none fits, create a NEW English snake_case name",
        "title": "short human-readable title in Portuguese (e.g., 'Fatura EDP — Janeiro 2026', 'Millennium BCP — Pagamento 500€', 'Seguro Auto — Nº 12345'). Keep it under 100 characters",
        "description": "brief description of the document in Portuguese (1-2 sentences)",
        "summary": "a richer, search-optimized summary (3-5 sentences) in the document's language. State the document type, the vendor/counterparty name(s), what the document is for, key amounts, relevant dates, and any identifiers (invoice / receipt / policy / contract numbers). Write names and numbers VERBATIM so the document can be found by search. Be specific and factual — no filler.",
        "confidence": 0.0 to 1.0,
        "type_prompt": "string or null — only if you created a NEW type: describe what it is and provide a suggested extraction_schema",
        "suggested_filename": "string — short descriptive filename in Portuguese using lowercase_underscores",
        "metadata": {
          // type-specific fields from the schema for the type you chose
        }
      }

      CRITICAL RULES — follow exactly:
      1. "document_type" must ALWAYS be in ENGLISH. Portuguese type names like "fatura", "fatura_recibo", "recibo", "nota_de_lancamento", "apolice" are FORBIDDEN. Use the English equivalents: expense_invoice, receipt, bank_statement, insurance_policy.
      2. Use the MOST SPECIFIC existing type that matches. Look at the list of existing types below carefully before deciding.
      3. If and only if no existing type fits, you may create a new English snake_case type name. Be conservative — only create a new type if the document is clearly distinct from all existing types.
      4. "other" is FORBIDDEN unless the file is not a business document at all (e.g., .eml email files, .ics calendar files, .zip archives, inline logos/signatures with no data, or completely unreadable files). If the file IS a business document — even if hard to read — you MUST pick the closest matching type. Use the original filename as a hint when the content is unclear.
      5. For a Portuguese "Fatura-Recibo" (combined invoice+receipt), use "receipt".
      6. For a "Nota de Lançamento" or single bank transaction record, use "bank_journal_entry".
      7. For an "Apolice de Seguro" or insurance policy, use "insurance_policy".
      8. For a "Certidão", "Declaração", or certificate, use "certificate".
      9. For a "Nota de Crédito" (NC) — a credit note that reverses, corrects, or refunds a prior invoice — use "credit_note", NOT "expense_invoice".
      10. Fill metadata fields according to the schema for the type you selected.
      11. BUYER NIF (buyer_nif field): extract the TAX NUMBER of the BUYER / RECIPIENT of the invoice.
          This is the entity that is being invoiced (the one that will pay). Look for labels:
          "NIF", "Contribuinte", "N.º de Contribuinte", "NIF do Adquirente", "NIF do Cliente".
          It is a 9-digit number, possibly prefixed "PT". Do NOT confuse it with the VENDOR'S NIF
          (the seller's tax ID, which is a different party). If only one NIF appears and the document
          is an expense invoice, it is more likely the vendor's NIF — leave buyer_nif null in that case.
      12. currency must always be an ISO-4217 code (EUR, USD, GBP, …). Never output "Euro", "€", "$",
          or any other non-code representation.
      13. NEVER use "null", "none", "unknown", or "n/a" as a document_type value. If no type fits,
          create a descriptive English snake_case name. Only "other" is acceptable as a fallback, and
          only under the conditions in rule 4.
    PROMPT

    def initialize(document)
      @document = document
    end

    def call
      config = Ai::Configuration.for(PURPOSE)

      unless config
        result = { confidence: 0.0, error: "No AI configuration for document_analysis" }
        Rails.logger.error("[Ai::DocumentAnalyzer] #{result[:error]}")
        apply_result(result)
        return result
      end

      result = analyze_with_adapter(config)
      if result[:confidence] < CONFIDENCE_THRESHOLD
        Rails.logger.info("[Ai::DocumentAnalyzer] Low confidence (#{result[:confidence]}), retrying")
        result = analyze_with_adapter(config)
      end

      apply_result(result)
      result
    end

    private

    def system_prompt
      types = DocumentType.where.not(name: %w[other unsupported_format zip_archive])
                          .includes(:rich_text_prompt).order(:name)

      type_list = types.map do |t|
        desc = t.prompt || t.name.humanize
        schema = t.extraction_schema
        fields = schema.is_a?(Hash) ? schema.keys.join(", ") : "any relevant fields"
        "  - #{t.name}: #{desc}
    Fields: #{fields}"
      end.join("

")

      <<~PROMPT
        #{BASE_PROMPT}

        #{direction_hint}

        Existing document types and their extraction fields:
        #{type_list}

        When creating a new type, provide:
        1. type_prompt: describe what this type is and when to use it
        2. Include a suggested extraction_schema in the type_prompt, e.g.:
           "extraction_schema: {\\"field_name\\":{\\"type\\":\\"string\\",\\"description\\":\\"...\\"}}"

        CRITICAL: When filling the metadata object, you MUST use the EXACT field names from the schema for the type you selected. Do NOT invent new field names. For example, if the schema says "vendor_name", use "vendor_name" not "supplier_name". For amounts, use integer cents (e.g., €123.45 = 12345).
        #{Ai::Configuration.user_prompt_suffix(PURPOSE)}
      PROMPT
    end

    def analyze_with_adapter(config)
      parts = build_generic_parts
      return { confidence: 0.0, error: "Unsupported document type" } unless parts

      messages = [ { role: "user", parts: parts } ]

      text = config[:adapter].chat(
        system: system_prompt,
        messages: messages,
        model: config[:model],
        max_tokens: config[:max_tokens],
        temperature: config[:temperature]
      )

      parse_response(text)
    rescue => e
      Rails.logger.error("[Ai::DocumentAnalyzer] Adapter error: #{e.message}")
      { confidence: 0.0, error: e.message }
    end

    def build_generic_parts
      blob = @document.original_file.blob
      file_data = blob.download
      base64_data = Base64.strict_encode64(file_data)
      filename = blob.filename.to_s

      parts = if @document.pdf?
        [
          { type: :document, media_type: "application/pdf", data: base64_data },
          { type: :text, text: "Analyze this Portuguese business document and extract structured data." }
        ]
      elsif @document.image?
        [
          { type: :image, media_type: blob.content_type, data: base64_data },
          { type: :text, text: "Analyze this Portuguese business document image and extract structured data." }
        ]
      elsif (text = office_text(file_data, blob.content_type)).present?
        # Office documents (.docx) carry no image the vision model can read, so we
        # extract their text and send THAT — otherwise the model only ever saw the
        # filename and could extract nothing.
        [
          { type: :text, text: "The attached file \"#{filename}\" is a Word document. Its extracted text content follows:

#{text[0, 50_000]}" },
          { type: :text, text: "Analyze this Portuguese business document and extract structured data." }
        ]
      else
        # For unsupported formats, send a text-only prompt with filename hints
        # so the analyzer can at least classify by name
        [
          { type: :text, text: "The attached file is named \"#{filename}\" and its content could not be read directly. Based on the filename and file type, classify this as accurately as possible. If the file extension indicates a non-document format (.eml email, .ics calendar, .zip archive, .gz compressed, .dwg CAD, etc.), use the \"other\" document type with a helpful title and filename suggestion." }
        ]
      end

      # Bias the model toward how the human has classified similar documents before.
      if (hint = classification_memory.prompt_hint)
        parts << { type: :text, text: hint }
      end

      parts
    end


    def direction_hint
      if @document.sent_email?
        "IMPORTANT: This document was SENT from your mailbox (outbound). " \
          "For invoices or payment-related documents, prefer revenue/outgoing types " \
          "(revenue_invoice, receipt for received payments). " \
          "The counterparty is the recipient: " + (@document.sender_name || "unknown").to_s + "."
      elsif @document.email?
        "This document was RECEIVED in your mailbox (inbound). " \
          "For invoices or payment-related documents, prefer expense/incoming types " \
          "(expense_invoice, receipt for payments made)."
      else
        ""
      end
    end

    def classification_memory
      @classification_memory ||= Documents::ClassificationMemory.new(@document)
    end

    # Extract the body text from a .docx (Office Open XML is a zip whose
    # word/document.xml holds the content). Paragraph breaks become newlines; the
    # remaining tags are stripped. Returns nil for non-docx or on any failure, so
    # the caller falls back to the filename-only prompt.
    def office_text(file_data, content_type)
      return nil unless content_type.to_s.include?("wordprocessingml.document")

      xml = nil
      Zip::File.open_buffer(StringIO.new(file_data)) do |zip|
        xml = zip.find_entry("word/document.xml")&.get_input_stream&.read
      end
      return nil if xml.blank?

      # The zip entry reads as binary (ASCII-8BIT); docx XML is UTF-8.
      xml = xml.force_encoding("UTF-8")
      text = xml.gsub(%r{</w:p>}, "
")
      text = text.gsub(/<[^>]+>/, "") while text.match?(/<[^>]+>/)
      CGI.unescapeHTML(text).gsub(/[ 	]+/, " ").gsub(/
{3,}/, "

").strip.presence
    rescue => e
      Rails.logger.warn("[Ai::DocumentAnalyzer] office_text extraction failed: #{e.message}")
      nil
    end

    def parse_response(text)
      return { confidence: 0.0, error: "Empty response" } if text.blank?

      json_text = text.match(/```json\s*(.*?)\s*```/m)&.captures&.first || text
      data = JSON.parse(json_text)

      {
        document_type: data["document_type"],
        title: data["title"],
        type_prompt: data["type_prompt"],
        description: data["description"],
        summary: data["summary"],
        confidence: data["confidence"] || 0.0,
        suggested_filename: data["suggested_filename"],
        metadata: data["metadata"] || {}
      }
    rescue JSON::ParserError => e
      Rails.logger.error("[Ai::DocumentAnalyzer] JSON parse error: #{e.message}")
      { confidence: 0.0, error: "Failed to parse AI response" }
    end

    def apply_result(result)
      if result[:error]
        # AI couldn't analyze this document (no config, adapter/parse error, …).
        # Record the failure *and the reason* so the human surface can explain why,
        # rather than silently parking it in the review queue.
        @document.update!(ai_status: :failed, ai_error: result[:error])
        return
      end

      type_name = result[:document_type]&.downcase&.strip

      # Guard: AI sometimes returns junk type names despite the prompt instructions.
      # Treat any of these as "other" (no custom type creation) to prevent polluting
      # the workspace's document-type list with garbage entries.
      type_name = "other" if type_name.present? && JUNK_TYPE_NAMES.include?(type_name)

      dt = if type_name.present? && type_name != "other"
        # Scope to the document's workspace: DocumentType names are unique per
        # workspace and the association is `workspace` (there is no `organization`).
        @document.workspace.document_types.find_or_create_by!(name: type_name) do |t|
          t.color = generate_color(type_name)

          # Parse suggested schema from type_prompt if present
          if result[:type_prompt]&.include?("extraction_schema")
            schema_str = result[:type_prompt].match(/extraction_schema:\s*(\{.*\})/m)&.[](1)
            t.extraction_schema = JSON.parse(schema_str) if schema_str
          end
        end
      end

      enum_value = map_to_enum(type_name)

      # Record the learned hint (if any) alongside the extraction so the review UI can
      # explain "matched N approvals from this sender". Reuses the memoized lookup.
      if (hint = classification_memory.suggestion)
        result[:classification_memory] = {
          "type" => hint.type_name, "source" => hint.source.to_s,
          "count" => hint.count, "total" => hint.total
        }
      end

      # Stash which AI provider/region produced this extraction (surfaced on the
      # document page) alongside the extraction data.
      result["_provenance"] = Ai::Provenance.for_purpose("document_analysis")

      # Build a normalized metadata hash from the AI result.
      raw_meta = (result[:metadata] || {}).dup.transform_keys(&:to_s)
      raw_meta["title"] = result[:title] if result[:title].present?

      # Vendor name alias chain — write under "vendor_name" while preserving alias keys.
      raw_meta["vendor_name"] ||=
        raw_meta["insurer_name"]      ||
        raw_meta["counterparty"]      ||
        raw_meta["proposer_name"]     ||
        raw_meta["sender"]            ||
        raw_meta["entity_name"]       ||
        raw_meta["issuing_authority"]

      # Default currency to EUR when any money-ish field is present and currency is absent.
      if raw_meta["currency"].blank? && raw_meta.any? { |k, v| k.end_with?("_cents") && v.present? }
        raw_meta["currency"] = "EUR"
      end

      # Per-field coercion via the document type's schema; keys not in the schema are
      # kept raw (the AI already returns ISO dates / integer cents from its prompt).
      schema = DocumentTypes::Schema.for(dt)
      normalized_meta = raw_meta.each_with_object({}) do |(key, value), h|
        next if value.nil?

        if (field = schema.field(key))
          coerced = field.coerce(value)
          h[key] = coerced unless coerced.nil?
        else
          h[key] = value
        end
      end

      @document.update!(
        document_type: enum_value,
        document_type_id: dt&.id,
        metadata: (@document.metadata || {}).merge(normalized_meta),
        description: result[:description],
        ai_summary: result[:summary],
        ai_confidence_score: result[:confidence],
        ai_extraction_data: result,
        ai_processing_attempts: (@document.ai_processing_attempts || 0) + 1,
        # AI did the heavy lifting; the human now reviews every result regardless of
        # confidence. Confidence is kept only to order the review queue and feed the
        # learning signal. Clear any prior failure reason on a successful re-analysis.
        ai_status: :completed,
        review_status: :pending,
        ai_error: nil
      )
    end

    TYPE_MAPPING = {
      # Portuguese → English
      "fatura" => "expense_invoice",
      "fatura_recibo" => "receipt",
      "recibo" => "receipt",
      "nota_de_lancamento" => "bank_journal_entry",
      "extrato_bancario" => "bank_statement",
      "movimento" => "bank_journal_entry",
      "lancamento" => "bank_journal_entry",
      "nota_de_credito" => "credit_note",
      "apolice_seguro" => "insurance_policy",
      "apolice" => "insurance_policy",
      "certidao" => "certificate",
      "certificado" => "certificate",
      "contrato" => "contract",
      "proposta" => "proposal",
      "documento_veiculo" => "vehicle_document",
      "documento_fiscal" => "tax_document",
      "identificacao" => "identification",
      "orcamento" => "proposal",
      "talon" => "receipt",
      "simulacao" => "insurance_policy",
      # English aliases / close matches
      "invoice" => "expense_invoice",
      "credit_note" => "credit_note",
      "financial_statement" => "bank_statement",
      "bank_document" => "bank_statement",
      "balance_sheet" => "bank_statement",
      "deposit_form" => "bank_statement",
      "deposit_information_form" => "bank_statement",
      "insurance_policy_simulation" => "insurance_policy",
      "policy" => "insurance_policy",
      "architectural_plan" => "correspondence",
      "arquitetonico_plano" => "correspondence",
      "corporate_decision" => "correspondence",
      "corporate_resolution" => "correspondence",
      "communication_to_clients_about_insurance_brokers" => "correspondence",
      "reclamation_request" => "correspondence",
      "cancellation_instructions" => "correspondence",
      "declaration" => "certificate",
      "declaration_of_responsibility" => "certificate",
      "notarial_certificate" => "certificate",
      "terms_and_conditions" => "contract",
      "price_list" => "proposal",
      "event_ticket" => "receipt",
      "bank_journal_entry" => "bank_journal_entry",
      # Files that aren't documents
      "unsupported_format" => "other",
      "zip_archive" => "other"
    }.freeze

    def map_to_enum(type_name)
      key = type_name.to_s.downcase.strip
      enum_value = if key.present? && Document.document_types.keys.include?(key)
        key
      else
        mapped = TYPE_MAPPING[key]
        mapped if mapped && mapped != "other"
      end

      # Filename-based hints can override when the AI is clearly wrong
      filename_hint = guess_from_filename
      return filename_hint if filename_hint != "other" && filename_contradicts?(enum_value)

      enum_value || filename_hint
    end

    def filename_contradicts?(ai_type)
      return false if ai_type.blank? || ai_type == "other"
      hint = guess_from_filename
      hint != "other" && hint != ai_type
    end

    def guess_from_filename
      name = @document.original_file.filename.to_s.downcase
      return "certificate" if name.match?(/certif|declara/)
      return "insurance_policy" if name.match?(/apolice|seguro|insurance/)
      return "contract" if name.match?(/contrato|contract|acordo/)
      return "bank_journal_entry" if name.match?(/lancamento|movimento|millennium|bcp|mov_/)
      return "bank_statement" if name.match?(/extrato|banco|bank|statement/)
      return "expense_invoice" if name.match?(/fatura|factura|invoice(?![^a-z])/)
      return "receipt" if name.match?(/recibo|receipt/)
      return "vehicle_document" if name.match?(/matricula|veiculo|vehicle|livrete/)
      return "proposal" if name.match?(/orcamento|proposta|proposal|budget/)
      return "correspondence" if name.match?(/carta|email|oficio|comunic|\bata\b|acta|reclama/i)
      return "identification" if name.match?(/cc_pass|passaporte|bilhete|identif/)
      return "contract" if name.match?(/cess_quota|estatutos|acordo|constitui/i)
      # DMARC / email feedback reports are XML, not business documents
      return "other" if name.end_with?(".xml") || name.end_with?(".xml.gz")
      "other"
    end

    def parse_date(val)
      return nil if val.blank?
      Date.parse(val)
    rescue
      nil
    end

    def generate_color(name)
      hash = name.bytes.sum
      hue = hash % 360
      "##{hsl_to_hex(hue, 0.65, 0.55)}"
    end

    def hsl_to_hex(h, s, l)
      h = h / 360.0
      c = (1 - (2 * l - 1).abs) * s
      x = c * (1 - ((h * 6) % 2 - 1).abs)
      m = l - c / 2.0
      r, g, b = case (h * 6).floor % 6
      when 0 then [ c, x, 0 ]
      when 1 then [ x, c, 0 ]
      when 2 then [ 0, c, x ]
      when 3 then [ 0, x, c ]
      when 4 then [ x, 0, c ]
      when 5 then [ c, 0, x ]
      end
      [ r, g, b ].map { |v| ((v + m) * 255).round.to_s(16).rjust(2, "0") }.join
    end
  end
end
