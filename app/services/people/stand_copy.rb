# frozen_string_literal: true

module People
  # Scout's words for a standing: the one-clause row line and the header note.
  # Composed at render time so every locale reads its own copy.
  module StandCopy
    module_function

    # Row line. nil when there is nothing to say (the row then shows the snippet).
    def line(result)
      case result.detail_kind
      when :ask_ai        then I18n.t("people.stand.asks_for", ask: result.detail)
      when :ask_quote     then I18n.t("people.stand.asks_quote", quote: result.detail)
      when :reason        then I18n.t("people.stand.waiting_for", what: result.detail)
      when :silence       then I18n.t("people.stand.no_answer_since", date: fmt_date(result.detail))
      when :prompt        then sentence_case(result.detail)
      when :you_wrote_last then I18n.t("people.stand.you_wrote_last", date: fmt_date(result.detail))
      when :money         then money_line(result)
      end
    end

    # Reply/forward prefixes belong to the row, not to a sentence about the thread.
    SUBJECT_PREFIX_RE = /\A(?:(?:re|fwd?|fw|aw|sv|tr|enc)\s*:\s*)+/i

    # Header note. nil when the standing has nothing to say (caller shows no_standing).
    def note(result, name:, date: nil)
      verb = result.verb
      dk   = result.detail_kind
      subj = result.subject.to_s.sub(SUBJECT_PREFIX_RE, "").strip.truncate(60)
      wait = result.wait_days.to_i

      if verb
        case verb
        when :reply
          if dk == :ask_ai
            I18n.t("people.conversation.stand.reply_ask", name: name, ask: result.detail,
                   subject: subj, count: wait)
          elsif dk == :ask_quote
            I18n.t("people.conversation.stand.reply_quote", name: name, quote: result.detail,
                   subject: subj, count: wait)
          else
            I18n.t("people.conversation.stand.reply_plain", name: name, subject: subj, count: wait)
          end
        when :nudge
          base = I18n.t("people.conversation.stand.nudge", name: name, subject: subj, count: wait)
          if dk == :reason && result.detail.present?
            base + " " + I18n.t("people.conversation.stand.nudge_reason", what: result.detail)
          else
            base
          end
        when :decide
          if dk == :prompt && result.detail.present?
            I18n.t("people.conversation.stand.decide", name: name, subject: subj, prompt: result.detail)
          elsif dk == :ask_ai
            I18n.t("people.conversation.stand.reply_ask", name: name, ask: result.detail,
                   subject: subj, count: wait)
          elsif dk == :ask_quote
            I18n.t("people.conversation.stand.reply_quote", name: name, quote: result.detail,
                   subject: subj, count: wait)
          else
            I18n.t("people.conversation.stand.reply_plain", name: name, subject: subj, count: wait)
          end
        when :pay
          money_note(result, name: name, key: "pay")
        when :chase
          money_note(result, name: name, key: "chase")
        end
      else
        # No verb: nothing is overdue, but the latest exchange may still say something.
        # `date` is when their latest message arrived (the caller passes it when known).
        case dk
        when :you_wrote_last
          I18n.t("people.conversation.stand.you_wrote_last", date: fmt_date(result.detail))
        when :ask_ai
          if date
            I18n.t("people.conversation.stand.fresh_ask", name: name, ask: result.detail, date: fmt_date(date))
          else
            I18n.t("people.conversation.stand.fresh_ask_undated", name: name, ask: result.detail)
          end
        when :ask_quote
          if date
            I18n.t("people.conversation.stand.fresh_quote", name: name, quote: result.detail, date: fmt_date(date))
          else
            I18n.t("people.conversation.stand.fresh_quote_undated", name: name, quote: result.detail)
          end
        end
      end
    end

    # An ISO date/time string, a Date or a Time → the short day_month form
    # ("Aug 30"); anything unparseable comes back as-is rather than blowing up a row.
    def fmt_date(value)
      I18n.l(value.to_date, format: :day_month)
    rescue ArgumentError, TypeError, NoMethodError
      value.to_s
    end

    def sentence_case(s)
      return s if s.blank?

      s[0].upcase + s[1..]
    end

    def money_line(result)
      m = result.money || {}
      cents    = m["amount_cents"]
      currency = m["currency"] || "EUR"
      due_date = m["due_date"]
      days     = m["days_late"].to_i

      amount = cents.present? ? ::Money.new(cents, currency).format : nil
      date   = due_date.present? ? fmt_date(due_date) : nil

      if result.verb == :chase
        I18n.t("people.stand.money_owed_to_you", amount: amount, date: date, count: days)
      else
        I18n.t("people.stand.money_late", amount: amount, date: date, count: days)
      end
    end

    def money_note(result, name:, key:)
      m = result.money || {}
      cents    = m["amount_cents"]
      currency = m["currency"] || "EUR"
      due_date = m["due_date"]
      days     = m["days_late"].to_i
      ref_num  = m["reference"].presence

      amount = cents.present? ? ::Money.new(cents, currency).format : nil
      date   = due_date.present? ? fmt_date(due_date) : nil
      ref    = if ref_num
        I18n.t("people.standing.invoice_ref", number: ref_num)
      else
        I18n.t("people.standing.an_invoice")
      end

      I18n.t("people.conversation.stand.#{key}", name: name, reference: ref,
             amount: amount, date: date, count: days)
    end
  end
end
