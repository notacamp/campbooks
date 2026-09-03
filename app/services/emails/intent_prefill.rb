# frozen_string_literal: true

module Emails
  # Infers the envelope a *new* message should open with from the context the
  # user came from — the "compose from intent" surface (bold layout). Two
  # inputs, either or both:
  #
  # - `to:`   an explicit address (People's "Reply to <name>", an org page).
  # - `intent:` free text ("write to Sofia about the Q3 deck") — a known contact
  #   name in it resolves the recipient, and the trailing "about …" clause (or,
  #   when the intent asks to reply/answer/follow up, the latest thread with that
  #   contact) becomes the subject.
  #
  # Everything it fills is marked *inferred*: the composer shows a muted
  # "· inferred" suffix that clears the moment the user edits the field. The
  # service only claims what it can prove — an ambiguous name match yields no
  # recipient rather than a guess (mirrors docs/messaging.md's honesty rule).
  #
  # Sibling to Emails::ComposePrefill (which handles reply/forward from a source
  # message); this one handles new messages started from context.
  class IntentPrefill
    Result = Struct.new(:to, :subject, :to_inferred, :subject_inferred, :contact, :reply_source,
                        keyword_init: true) do
      def to_inferred? = to_inferred ? true : false
      def subject_inferred? = subject_inferred ? true : false
      def any? = to.to_s.present? || subject.to_s.present?
    end

    # The intent asks to continue an existing conversation → prefer "Re: <thread>".
    REPLY_CUES = /\b(?:repl(?:y|ies|ied)|answer(?:ing)?|respond(?:ing)?|follow[ -]?up|get back|circle back|chase)\b/i

    # Words that tend to precede a recipient's name; used only to widen the
    # candidate-name net before matching against the contact list.
    NAME_CUES = %w[to for reply answer respond email message write ask tell remind ping contact dm].freeze

    # Common words never worth treating as a name, even when capitalized (e.g. a
    # sentence's first word) — keeps the candidate query cheap and precise.
    STOPWORDS = %w[
      the a an and or but about for to from with of on in at re fwd please can could you your
      i we they he she it this that them him her about ask tell write reply answer respond email
      message send hi hey hello thanks thank regarding
    ].to_set.freeze

    EMAIL_RE = /\A[^@\s]+@[^@\s]+\.[^@\s]+\z/

    def self.for(user:, intent: nil, to: nil)
      new(user: user, intent: intent, to: to).call
    end

    def initialize(user:, intent:, to:)
      @user = user
      @intent = intent.to_s.strip
      @to = to.to_s.strip
    end

    def call
      contact = resolved_contact
      to_value = to_address(contact)
      reply = reply_source_for(contact)
      subject, subject_inferred = subject_for(reply)

      Result.new(
        to: to_value,
        subject: subject,
        to_inferred: to_value.present?,
        subject_inferred: subject_inferred,
        contact: contact,
        reply_source: reply
      )
    end

    private

    # ── recipient ────────────────────────────────────────────────
    def resolved_contact
      return contact_by_email(@to) if explicit_to?
      contact_from_intent
    end

    def explicit_to? = @to.present? && @to.match?(EMAIL_RE)

    def contact_by_email(email)
      workspace_contacts.where("lower(email) = ?", email.downcase).first
    end

    # Match contacts whose name carries an intent name-token as a whole word,
    # then break ties by starred-first, most-recent-next. A genuine tie (two
    # equally-ranked matches) is ambiguous → no recipient.
    def contact_from_intent
      candidates = contact_candidates
      return nil if candidates.empty?
      return candidates.first if candidates.one?

      ranked = candidates.sort_by { |c| rank_key(c) }
      best, second = ranked[0], ranked[1]
      return nil if rank_key(best) == rank_key(second)

      best
    end

    def rank_key(contact)
      [ contact.starred? ? 0 : 1, -(contact.last_email_at&.to_i || 0) ]
    end

    def contact_candidates
      tokens = name_tokens
      return [] if tokens.empty?

      # Postgres word-boundary regexes (\m…\M), case-insensitive (~*), matched
      # against any token in one query so the scan stays bounded to real hits.
      regexes = tokens.map { |t| "\\m#{Regexp.escape(t)}\\M" }
      workspace_contacts
        .where.not(name: [ nil, "" ])
        .where.not(email: [ nil, "" ])
        .where("name ~* ANY (ARRAY[?]::text[])", regexes)
        .to_a
    end

    def workspace_contacts
      @user.workspace.contacts
    end

    # Candidate name words: every capitalized word plus every word following a
    # directional cue, downcased and stripped of trailing punctuation, minus the
    # stopword list.
    def name_tokens
      return [] if @intent.blank?

      words = @intent.scan(/\p{L}[\p{L}'.\-]*/)
      capitalized = words.select { |w| w.match?(/\A\p{Lu}/) }
      cued = []
      words.each_with_index do |word, i|
        cued << words[i + 1] if NAME_CUES.include?(word.downcase) && words[i + 1]
      end

      (capitalized + cued)
        .map { |w| w.downcase.gsub(/[.'\-]+\z/, "") }
        .uniq
        .reject { |w| w.length < 2 || STOPWORDS.include?(w) }
    end

    def to_address(contact)
      if contact
        name = contact.display_name.to_s.tr(",", " ").squish
        name.present? && name.casecmp?(contact.email) == false ? "#{name} <#{contact.email}>" : contact.email
      elsif explicit_to?
        @to
      else
        ""
      end
    end

    # ── subject ──────────────────────────────────────────────────
    def reply_source_for(contact)
      return nil unless contact
      return nil unless @intent.match?(REPLY_CUES)

      EmailMessage.accessible_to(@user)
                  .where(contact_id: contact.id)
                  .order(received_at: :desc)
                  .first
    end

    def subject_for(reply)
      clause = about_clause
      if clause.present?
        [ capitalize_first(clause), true ]
      elsif reply&.subject.to_s.present?
        subject = decode(reply.subject)
        [ subject.match?(/\Are:\s*/i) ? subject : "Re: #{subject}", true ]
      else
        [ "", false ]
      end
    end

    # The trailing "about …" clause of the intent, sans terminal punctuation.
    def about_clause
      match = @intent.match(/\babout\s+(.+?)[.?!]*\z/i)
      match && match[1].strip.presence
    end

    def capitalize_first(text)
      text.sub(/\A(\p{L})/) { ::Regexp.last_match(1).upcase }
    end

    # Zoho stored some metadata HTML-escaped; decode so an inferred "Re:" subject
    # reads as text, not entities (mirrors Emails::ComposePrefill#decode).
    def decode(str)
      str.to_s.include?("&") ? CGI.unescapeHTML(str.to_s) : str.to_s
    end
  end
end
