module Tasks
  # Cheap pre-filter deciding whether an email is worth an LLM task-extraction call.
  # It has two jobs: cost (most mail carries no action for the reader, so skip the
  # model call) and noise (mail that CANNOT carry a personal ask must never mint
  # tasks, however imperative its wording — a code-review bot's "please fix", a
  # security alert's "change your password" and a marketplace's "leave feedback"
  # are CTAs, not commitments between people).
  #
  # Order: hard vetoes first — outbound mail (an ask in OUR sent mail is the
  # recipient's task, not the reader's), automated/no-reply senders, and the
  # machine triage categories — then an action-request keyword screen covering all
  # four app locales (en/pt/es/fr; the old English-only list silently exempted
  # every Portuguese ask from extraction). The keyword layer leans permissive: a
  # false positive costs one model call; the LLM + confidence floor reject the FYI
  # mail that slips through.
  class ExtractionGate
    # Triage categories that are machine traffic by definition. `updates` is
    # deliberately NOT here — its subject rules ("invoice", "fatura"…) also match
    # real human mail (an accountant's "please pay the invoice").
    MACHINE_CATEGORIES = %w[notifications promotions social].freeze

    KEYWORDS = /\b(?:
      # ---- en ----------------------------------------------------------------
      please|kindly|could\s+you|can\s+you|would\s+you|need\s+you\s+to|
      action\s+(?:required|needed)|to-?do|follow[\s-]?up|let\s+me\s+know|
      get\s+back\s+to\s+(?:me|us)|send\s+(?:me|us|over|back)|forward\s+(?:me|us)|
      review|approv\w*|sign\s+off|sign\s+and\s+return|signature|confirm|complete|
      fill\s+out|submit|provide|respond|reply|reach\s+out|prepare|pay(?:ment)?|
      required|requested|awaiting\s+your|waiting\s+(?:on|for)\s+you|
      your\s+(?:input|feedback|response|approval|sign)|
      # ---- pt ----------------------------------------------------------------
      por\s+favor|agrade[çc](?:o|a|emos|ia)|pod(?:es?|ia[ms]?|eria[ms]?)|
      necessári[oa]|precis(?:o|amos)\s+(?:de|que)|
      envi(?:a|ar|em|o)|reenviar|assin(?:a|ar|atura)|confirm(?:a|ar|em)|
      rever|revis(?:ão|ao|ar)|valid(?:a|ar)|aprov(?:a|ar|ação|acao)|
      preench(?:e|er)|submet(?:e|er)|entreg(?:a|ar)|respond(?:e|er|am)|resposta|
      pag(?:a|ar|amento)|marcar|devolver|remeter|aguard(?:o|amos)|
      # ---- es ----------------------------------------------------------------
      pued(?:es|e)|podría[ns]?|necesit(?:o|amos)|necesari[oa]|
      enví[ae]|firm(?:a|ar|e)|rellen(?:a|ar)|complet(?:a|ar)|apr(?:ueba|obar)|
      respuesta|pag(?:o|ue)|acción\s+requerida|quedamos\s+a\s+la\s+espera|
      # ---- fr ----------------------------------------------------------------
      merci\s+de|veuillez|pourr(?:iez|ais?)[\s-]+(?:vous|tu)|p(?:eux|ouvez)[\s-]+(?:tu|vous)|
      il\s+faut|nous\s+avons\s+besoin|besoin\s+de|envo(?:ye[rz]|ies?)|renvoyer|
      sign(?:er|ez)|vérifi(?:er|ez)|valid(?:er|ez)|approuv(?:er|ez)|
      rempl(?:ir|issez)|répond(?:re|ez)|réponse|transmettre|payer|paiement|
      action\s+requise|dans\s+l'attente
    )\b/xi

    def self.email_allows?(email)
      new.email_allows?(email)
    end

    def self.vetoed?(email)
      new.vetoed?(email)
    end

    def email_allows?(email)
      return false if vetoed?(email)

      # Quote-stripped text: a reply is screened on what the sender just wrote, so
      # a bare "bumping this" doesn't re-open asks already extracted upthread.
      text = [ email.subject, email.try(:ai_summary), Emails::PlainText.of(email.body) ].compact.join(" ")
      text.match?(KEYWORDS)
    end

    # The hard vetoes alone — mail that cannot yield a task for the reader no
    # matter its wording. Split from the keyword screen so cleanup (pruning old
    # machine-mail suggestions) can apply them without re-judging content the LLM
    # already accepted.
    def vetoed?(email)
      junk?(email) ||
        email.try(:outbound?) ||
        MACHINE_CATEGORIES.include?(email.try(:category).to_s) ||
        Emails::Categorizer.machine_sender?(email)
    end

    private

    # Only clear junk; "bulk"/"list" still pass (a real ask — a DocuSign request,
    # an invoice from a human — can ride on list-flavoured transactional mail).
    def junk?(email)
      email.try(:header_precedence).to_s.strip.downcase == "junk"
    end
  end
end
