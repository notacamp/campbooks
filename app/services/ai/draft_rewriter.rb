# frozen_string_literal: true

module Ai
  # Rewrites the *body the user is actually writing* to a requested tone — the
  # "Shorter" / "Warmer" (and existing "Firmer") buttons in the bold composer's
  # editor footer. Distinct from Compose::ScoutDraft's tone chips, which
  # regenerate Scout's separate ghost draft: this one takes the editor's current
  # HTML and hands back a rewritten HTML body to drop straight back in.
  #
  # Uses the workspace's configured text provider (Ai::Configuration.for_any over
  # the text purposes), honours the user's writing style, and is best-effort:
  # it never raises and returns nil on any failure, so the caller can leave the
  # draft untouched.
  class DraftRewriter
    TONES = %w[shorter warmer firmer].freeze

    MAX_TOKENS = 1500
    MODEL = "claude-sonnet-4-5-20250929" # legacy self-hosted fallback only

    TONE_INSTRUCTIONS = {
      "shorter" => "Make it noticeably shorter and more concise. Cut filler, hedging and " \
                   "repetition, but keep every concrete fact, question, request, name, date and number.",
      "warmer"  => "Make the tone warmer and friendlier — a little more personal, considerate and " \
                   "human — without adding new facts, promises or information.",
      "firmer"  => "Make the tone firmer and more direct — clearer and more assertive while staying " \
                   "professional and polite — without adding new facts, promises or information."
    }.freeze

    # @param body_html [String] the editor's current HTML.
    # @param tone [String] one of TONES.
    # @param style [String, nil] the user's writing-style prompt (User#writing_style_prompt).
    # @return [String, nil] rewritten HTML, or nil when unavailable / on any error.
    def rewrite(body_html, tone:, style: nil)
      tone = tone.to_s
      return nil unless TONES.include?(tone)
      return nil if body_html.to_s.strip.blank?

      raw = generate_text(system_prompt(tone, style), user_message(body_html))
      return nil if raw.blank?

      strip_fences(raw).strip.presence
    rescue => e
      Rails.logger.error("[Ai::DraftRewriter] #{e.class}: #{e.message}")
      nil
    end

    private

    def system_prompt(tone, style)
      <<~PROMPT.strip
        You rewrite the body of an email a person is composing. Apply exactly this change:

        #{TONE_INSTRUCTIONS.fetch(tone)}

        Rules:
        - Preserve the meaning and every concrete fact. Never invent details.
        - Keep the same language as the original.
        - Return valid HTML: keep the paragraph, list and link structure. Do not add a
          subject line, a greeting, or a signature that was not already present.
        - Output ONLY the rewritten HTML body. No explanation, no preamble, no markdown fences.
        #{style.to_s.strip.presence && "\n#{style.strip}"}
      PROMPT
    end

    def user_message(body_html)
      "Rewrite this email body:\n\n#{body_html}"
    end

    # Mirrors Labels::AiClassifier#generate_text: the workspace's configured text
    # provider, or the global Anthropic key on self-hosted.
    def generate_text(system, user)
      config = Ai::Configuration.for_any(AiConfiguration::TEXT_PURPOSES)
      if config
        config[:adapter].chat(
          system: system,
          messages: [ { role: "user", content: user } ],
          model: config[:model],
          max_tokens: MAX_TOKENS,
          temperature: config[:temperature]
        )
      elsif Ai::LegacyFallback.allowed?
        client = Anthropic::Client.new
        response = client.messages.create(
          model: MODEL,
          max_tokens: MAX_TOKENS,
          system: system,
          messages: [ { role: "user", content: user } ],
          thinking: { type: "disabled" }
        )
        response.content.find { |c| c.type.to_s == "text" }&.text
      end
    end

    # Some models wrap output in ```html … ``` despite the instruction; unwrap it.
    def strip_fences(text)
      stripped = text.strip
      return stripped unless stripped.start_with?("```")

      stripped.sub(/\A```[a-zA-Z]*\n?/, "").sub(/```\z/, "")
    end
  end
end
