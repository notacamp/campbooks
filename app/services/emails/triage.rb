# frozen_string_literal: true

module Emails
  # Orchestrates the triage cost-ladder for a single email and returns a pure
  # Decision. The caller (EmailProcessJob) performs the side effects: persist the
  # category, assign the tag, and only call the expensive Ai::EmailClassifier
  # when the decision says so.
  #
  #   rung 1  rules        — always; coarse category (free)
  #   rung 2  embeddings   — shortlist the nearest workspace tags (~$0)
  #   rung 3  cheap model  — pick the best tag from the shortlist (cheap)
  #   rung 4  full LLM     — only when rungs 1–3 can't resolve it
  #
  # Important / sensitive mail (rules → :important) always takes the full-LLM path
  # so the existing security pre-screen in Ai::EmailClassifier still runs. Cheap
  # rungs that error degrade toward the LLM — ingestion never breaks.
  class Triage
    SHORTLIST_SIZE = 3

    Decision = Data.define(:category, :confidence, :tag, :source) do
      # When true the caller should run Ai::EmailClassifier (the ~2-LLM-call
      # path). Any other source means a cheaper rung already resolved the tagging.
      def needs_llm? = source == :llm
    end

    def initialize(email, rules: nil, embedding: nil, picker: nil)
      @email = email
      @rules = rules
      @embedding = embedding
      @picker = picker
    end

    def call
      rules = categorizer.call

      # Important / sensitive mail: full LLM (and its security pre-screen).
      return escalate(rules) if rules.category == :important

      shortlist = embedding_shortlist
      return escalate(rules) if shortlist.empty?

      # A rare near-duplicate match we can trust without spending a model call.
      top = shortlist.first
      return resolved(rules, top.tag, top.similarity, :embedding) if top.confident?

      # Rung 3: a cheap model picks the best tag from the shortlist.
      picked = cheap_pick(shortlist)
      if picked
        resolved(rules, picked.tag, top.similarity, :cheap_llm)
      else
        escalate(rules)
      end
    end

    private

    def categorizer = @rules || Categorizer.new(@email)

    def embedding_shortlist
      (@embedding || EmbeddingClassifier.new(@email)).shortlist(limit: SHORTLIST_SIZE)
    rescue => e
      log_warning("embedding rung failed: #{e.message}")
      []
    end

    def cheap_pick(shortlist)
      (@picker || LlmTagPicker.new(@email, shortlist)).call
    rescue => e
      log_warning("cheap-model rung failed: #{e.message}")
      nil
    end

    def resolved(rules, tag, confidence, source)
      Decision.new(category: rules.category, confidence: confidence, tag: tag, source: source)
    end

    def escalate(rules)
      Decision.new(category: rules.category, confidence: rules.confidence, tag: nil, source: :llm)
    end

    def log_warning(message)
      Rails.logger.warn("[Emails::Triage] #{message} (email #{email_id})") if defined?(Rails)
    end

    def email_id = @email.respond_to?(:id) ? @email.id : nil
  end
end
