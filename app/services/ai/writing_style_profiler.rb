module Ai
  # Derives a short "how this person writes" profile from the user's own sent
  # mail, so Scout can draft replies in their voice. Stored on
  # user.writing_style_learned and augmented/overridden by the manually written
  # user.writing_style (see User#writing_style_prompt). Runs in the background
  # via WritingStyleProfileJob.
  class WritingStyleProfiler
    PURPOSE = "draft_reply"
    SAMPLE_SIZE = 20
    MAX_CHARS_PER_MESSAGE = 800
    MAX_TOTAL_CHARS = 8_000

    def self.call(user)
      samples = sent_samples(user)
      return nil if samples.empty?

      text = chat(system_prompt, user_message(samples))
      return nil if text.blank?

      user.update!(writing_style_learned: text.strip, writing_style_updated_at: Time.current)
      text.strip
    rescue => e
      Rails.logger.error("[Ai::WritingStyleProfiler] #{e.class}: #{e.message}")
      nil
    end

    # The user's recent sent messages (sent? == from_address matches the account),
    # de-HTML'd and bounded so the prompt stays small.
    def self.sent_samples(user)
      accounts = user.sendable_email_accounts.to_a
      addresses = accounts.filter_map { |a| a.email_address.to_s.downcase.presence }
      return [] if accounts.empty? || addresses.empty?

      messages = EmailMessage
        .where(email_account_id: accounts.map(&:id))
        .where("LOWER(from_address) IN (?)", addresses)
        .where.not(body: [ nil, "" ])
        .order(received_at: :desc)
        .limit(SAMPLE_SIZE)

      samples = []
      total = 0
      messages.each do |m|
        body = clean_body(m.body)
        next if body.blank?
        break if total + body.length > MAX_TOTAL_CHARS

        samples << body
        total += body.length
      end
      samples
    end

    def self.clean_body(html)
      text = ActionController::Base.helpers.strip_tags(html.to_s)
      CGI.unescapeHTML(text).gsub(/\s+/, " ").strip[0, MAX_CHARS_PER_MESSAGE]
    end

    def self.system_prompt
      <<~PROMPT
        You analyze how a person writes emails so an assistant can later draft replies in their voice.
        From the writing samples, produce a SHORT profile (under 120 words) capturing:
        - typical greeting and sign-off
        - formality and warmth
        - sentence / paragraph length and structure
        - recurring phrases, emoji, or punctuation habits
        - the language(s) they write in
        Write it as direct guidance ("Greets with…", "Signs off…", "Tends to…"). Do not quote whole emails or invent traits that aren't in the samples. Output the profile text only — no preamble.
      PROMPT
    end

    def self.user_message(samples)
      "Writing samples (most recent first):\n\n" + samples.join("\n\n---\n\n")
    end

    def self.chat(system, user_message)
      config = Ai::Configuration.for(PURPOSE)
      return nil unless config

      config[:adapter].chat(
        system: system,
        messages: [ { role: "user", content: user_message } ],
        model: config[:model],
        max_tokens: [ config[:max_tokens].to_i, 500 ].min.clamp(200, 500),
        temperature: 0.3
      )
    rescue => e
      Rails.logger.error("[Ai::WritingStyleProfiler] adapter error: #{e.message}")
      nil
    end
  end
end
