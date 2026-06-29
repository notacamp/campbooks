module Documents
  # Heuristic, bilingual (EN/PT) parser that turns a free-text Files search query
  # into structured hints — a document type, an invoice/receipt number, and a
  # counterparty name — so search can target the common journeys without an LLM:
  #
  #   "invoice FT 2024/123"            → type: expense_invoice, number: "FT 2024/123"
  #   "the contract with company Acme" → type: contract, counterparty: "Acme"
  #   "all payment receipts to Acme"   → type: receipt, counterparty: "Acme"
  #
  # Every hint is a SOFT signal. Documents::Search uses the type only to pre-narrow
  # the semantic candidate pool, and the counterparty/number only to sharpen the
  # keyword (ILIKE) arm — so a wrong guess never hides a result, it only fails to
  # boost one. When a query mentions two different types (often because a type word
  # sits inside a company name, e.g. "Seguros"), we treat it as ambiguous and emit
  # no type hint, letting the semantic + keyword arms decide.
  class QueryParser
    Result = Struct.new(:cleaned_query, :document_type, :number, :counterparty, keyword_init: true)

    # Bilingual keyword → Document#document_type enum string. We collect the SET of
    # distinct types a query mentions and only emit a hint when EXACTLY ONE matches.
    # "invoice"/"fatura" maps to expense_invoice (the dominant ingested case); the
    # keyword arm still finds revenue invoices by number, so the soft pin is safe.
    TYPE_PATTERNS = {
      "receipt"            => /\b(?:receipts?|recibos?|tal[ãa]o|talões|fatura[\s-]?recibos?)\b/i,
      "expense_invoice"    => /\b(?:invoices?|faturas?|facturas?|bill)\b/i,
      "credit_note"        => /\b(?:credit\s+notes?|notas?\s+de\s+cr[ée]dito)\b/i,
      "bank_statement"     => /\b(?:bank\s+statements?|extratos?|extractos?)\b/i,
      "bank_journal_entry" => /\b(?:lan[çc]amentos?|movimentos?|nota\s+de\s+lan[çc]amento)\b/i,
      "contract"           => /\b(?:contracts?|contratos?|agreements?|acordos?)\b/i,
      "insurance_policy"   => /\b(?:insurances?|seguros?|ap[óo]lices?|policy|policies)\b/i,
      "certificate"        => /\b(?:certificates?|certificados?|certid[õo]es|certid[ãa]o|declara[çc][õo]es|declara[çc][ãa]o|declarations?)\b/i,
      "tax_document"       => /\b(?:tax|taxes|imposto|impostos|fiscal|irs|irc)\b/i,
      "vehicle_document"   => /\b(?:vehicles?|ve[íi]culos?|matr[íi]culas?|livrete|car)\b/i,
      "identification"     => /\b(?:identifica[çc][ãa]o|passports?|passaportes?|bilhete\s+de\s+identidade|cart[ãa]o\s+de\s+cidad[ãa]o)\b/i,
      "proposal"           => /\b(?:proposals?|propostas?|or[çc]amentos?|quotes?|budget)\b/i,
      "correspondence"     => /\b(?:correspondence|letters?|cartas?|of[íi]cios?)\b/i
    }.freeze

    # Invoice / receipt / document numbers. Ordered most-specific first; first hit
    # wins. Each captures the number to match in group 1 (against the original-case
    # query — codes are upper-case).
    NUMBER_PATTERNS = [
      # Portuguese fiscal codes: FT 2024/123, FR-123/2024, FT20240123 (prefix kept)
      %r{\b((?:FT|FR|NC|RC|FS)\s*[-/]?\s*\d+(?:[-/]\d+)?)\b}i,
      # A number introduced by a document noun: "invoice 12345", "apólice 998877",
      # "nº 2024/001", "ref ABC-123" (the noun is dropped, the number kept)
      %r{\b(?:invoices?|faturas?|facturas?|recibos?|receipts?|ap[óo]lices?|polic(?:y|ies)|contracts?|contratos?|n[ºo]\.?|ref\.?|#)\s*[:#-]?\s*(\d[\w/-]{2,19})\b}i,
      # A bare slashed/dashed number: 2024/123
      %r{\b(\d{4}[-/]\d{1,6})\b}
    ].freeze

    # Conservative counterparty cues — only fire on an explicit signal so we don't
    # mistake an ordinary noun for a company. A quoted name, or a name introduced by
    # "company/empresa/…". Straight single quotes are NOT delimiters (apostrophes).
    QUOTED   = /["“”‘’]([^"“”‘’]{2,60})["“”‘’]/
    LABELLED = /\b(?:company|empresa|fornecedor|cliente|vendor|supplier)\s+(\p{L}[\p{L}\p{N}&.,'\-\s]{1,40})/i

    # Framing phrases that carry no search signal — stripped from the text that gets
    # embedded, so the vector targets the content rather than the request wrapper.
    LEAD_INS = /\A\s*(?:i'?m\s+looking\s+for|looking\s+for|find(?:\s+me)?|show\s+me|search\s+for|get\s+me|where(?:'s|\s+is)|preciso\s+de|procuro(?:\s+por)?|encontrar?|mostra(?:-me)?|onde\s+est[áa])\s+(?:the\s+|an?\s+|os?\s+|as?\s+|um(?:a)?\s+)?/i

    def self.parse(query)
      new(query).parse
    end

    def initialize(query)
      @raw = query.to_s.strip
      @normalized = @raw.downcase
    end

    def parse
      Result.new(
        cleaned_query: clean_query,
        document_type: extract_document_type,
        number: extract_number,
        counterparty: extract_counterparty
      )
    end

    private

    def extract_document_type
      matched = TYPE_PATTERNS.select { |_enum, re| @normalized.match?(re) }.keys
      matched.one? ? matched.first : nil
    end

    def extract_number
      NUMBER_PATTERNS.each do |re|
        m = @raw.match(re)
        return m[1].strip if m
      end
      nil
    end

    def extract_counterparty
      m = @raw.match(QUOTED) || @raw.match(LABELLED)
      m && clean_name(m[1])
    end

    def clean_name(str)
      str.to_s.strip.sub(/[\s,;:.]+\z/, "").presence
    end

    def clean_query
      @raw.sub(LEAD_INS, "").strip.presence || @raw
    end
  end
end
