# frozen_string_literal: true

module Emails
  # The ask in an inbound message, quoted from what the sender actually wrote —
  # no model call. Used by People to say "Asks: “…”" on rows and in the header
  # when Scout's full read (ai_ask) is absent. Returns nil when nothing reads as
  # an ask. Language-aware for en/pt/es/fr cues; the quote keeps its language
  # (and its accents — it is a quote, never transliterated).
  class AskExtractor
    MAX_LENGTH = 110

    def self.call(message) = new(message).call

    def initialize(message)
      @message = message
    end

    def call
      text = Emails::PlainText.of(@message.body.to_s.first(20_000))
      text = @message.summary.to_s if text.blank?
      return nil if text.blank?

      sentences = split_sentences(text)
      pick = last_question(sentences) || last_request(sentences)
      return nil if pick.nil?

      normalize(pick)
    end

    private

    # Sentence ends: terminal punctuation, optionally followed by a closing quote
    # or bracket, then whitespace. (Two fixed-width lookbehind alternatives.)
    SENTENCE_END_RE = /(?<=[.!?…]|[.!?…]["”»)])\s+/

    def split_sentences(text)
      text.split(SENTENCE_END_RE)
          .map(&:strip)
          .reject { |s| s.length < 12 }
    end

    def question?(sentence)
      sentence.match?(/\?["”»)]?\z/) || sentence.include?("¿")
    end

    def request?(sentence)
      sentence.match?(REQUEST_RE)
    end

    def pleasantry?(sentence)
      sentence.match?(PLEASANTRY_RE)
    end

    def last_question(sentences)
      sentences.reject { |s| pleasantry?(s) }.select { |s| question?(s) }.last
    end

    def last_request(sentences)
      sentences.reject { |s| pleasantry?(s) }.select { |s| request?(s) }.last
    end

    # A lead-in before a dash or colon ("Following up on clause 7.2 — could you
    # confirm the cap?") is context, not the ask: keep the tail when it opens like
    # a question or request of its own ("Tuesday or Wednesday?" does not).
    LEAD_IN_RE = /\s[—–-]\s|:\s/
    ASK_OPENER_RE = /\A(?:¿|please|could|can|would|will|shall|do|does|did|is|are|what|which|when|where|who|how|any\ chance|let\ me\ know|send|confirm|kindly|
      por\ favor|podes|pode|poderia|poderias|consegues|consegue|envia|envie|confirma|confirme|quando|qual|onde|como|
      puedes|podrías|podría|puede|envíame|mándame|cuándo|qué|cuál|dónde|cómo|
      merci\ de|peux-tu|pouvez-vous|pourriez-vous|pourrais-tu|envoie|envoyez|quand|quel|quelle|où|comment|est-ce)\b/xi
    WRAPPING_QUOTES_RE = /\A["“«]\s*|\s*["”»]\z/

    # Drop a lead-in and a leading connector ("Also, …"), collapse whitespace,
    # capitalise the first character, and cut long asks at a word boundary.
    def normalize(sentence)
      sentence = strip_lead_in(sentence)
      sentence = sentence.sub(LEADING_CONNECTOR_RE, "").gsub(/\s+/, " ").strip
      return nil if sentence.blank?

      sentence = sentence[0].upcase + sentence[1..]
      truncate(sentence)
    end

    def strip_lead_in(sentence)
      parts = sentence.split(LEAD_IN_RE)
      return sentence if parts.size < 2

      tail = parts.last.to_s.strip.gsub(WRAPPING_QUOTES_RE, "")
      tail.length >= 12 && tail.match?(ASK_OPENER_RE) && (question?(tail) || request?(tail)) ? tail : sentence
    end

    def truncate(str)
      return str if str.length <= MAX_LENGTH

      cut = str[0, MAX_LENGTH].rindex(" ")
      cut ? "#{str[0, cut]}…" : "#{str[0, MAX_LENGTH]}…"
    end

    REQUEST_RE = /\b(
      please|could\ you|can\ you|would\ you|let\ me\ know|send\ (?:me|us|over|it)|confirm|kindly|need\ you\ to|
      por\ favor|podes|pode|poderia|poderias|consegues|consegue|envia(?:-me)?|envie|confirma|confirme|diz-me|diga-me|agradeço\ que|agradecia\ que|
      puedes|podrías|podría|puede|envíame|mándame|avísame|dime|necesito\ que|
      merci\ de|s'il\ (?:te|vous)\ plaît|peux-tu|pouvez-vous|pourriez-vous|pourrais-tu|envoie(?:-moi)?|envoyez(?:-moi)?|confirme[rz]?|dis-moi|dites-moi|j'ai\ besoin\ que
    )\b/xi

    PLEASANTRY_RE = /\A(?:how are you|how's it going|how is it going|hope (?:you|all|this)|i hope|tudo bem|como (?:estás|está|vai)|espero que|cómo (?:estás|está|va)|qué tal|ça va|comment (?:vas|allez)|j'espère)/i

    LEADING_CONNECTOR_RE = /\A(?:also|anyway|so|and|but|além disso|además|aussi|donc)[,:]?\s+/i
  end
end
