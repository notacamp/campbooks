# frozen_string_literal: true

module Emails
  # Deterministic, LLM-free first-pass categorizer.
  #
  # Sorts the inbox firehose using cheap signals (sender, subject, folder). It is
  # the first, free rung of a triage cost-ladder:
  #
  #   1. rules          (this class)        — free, instant
  #   2. embeddings     (EmbeddingService)  — ~$0 per email, also clusters
  #   3. cheap model    (haiku / 4o-mini)   — only the ambiguous residue
  #   4. full analysis  (Ai::EmailAnalyzer) — only mail that "could matter"
  #
  # When the rules are confident (#decisive?) triage stops here; otherwise the
  # orchestrator defers the email to the next rung rather than trusting a weak
  # guess. Most machine notifications and bulk marketing resolve for free and
  # never reach an LLM.
  #
  # Today it reads only fields we already persist on EmailMessage. As we capture
  # richer signals at ingest (List-Unsubscribe / Precedence headers, Gmail
  # category labels) the detectors get sharper without changing this interface —
  # see #header_category, currently a no-op.
  #
  # Kept dependency-free (no ActiveSupport) so it can be unit-tested in isolation.
  class Categorizer
    CATEGORIES = %i[personal important notifications promotions social updates unknown].freeze

    Result = Data.define(:category, :confidence, :reasons) do
      # Bulk mail that is safe to collapse / clear in one swipe.
      def noise? = %i[promotions social].include?(category)

      # True when the rules nailed the bucket confidently enough to act on now.
      # When false, the orchestrator defers the email up the cost ladder
      # (embeddings → cheap completion → full analysis) instead of trusting a
      # weak rules guess.
      def decisive? = confidence >= 0.6 && !%i[personal unknown].include?(category)
    end

    # Sender local-parts that signal an unattended / automated mailbox.
    MACHINE_LOCALPARTS = %w[
      no-reply noreply no_reply donotreply do-not-reply notifications notification
      notify mailer-daemon postmaster bounce bounces builds build ci automated auto
      naoresponder nao-responder naoresponda pesquisas
    ].freeze

    # Local-parts typical of marketing / newsletter senders.
    BULK_LOCALPARTS = %w[
      news newsletter newsletters hello store shop offers promo promotions
      marketing deals team info
    ].freeze

    SOCIAL_DOMAINS = %w[
      facebookmail.com facebook.com twitter.com x.com linkedin.com instagram.com
      tiktok.com reddit.com pinterest.com
    ].freeze

    # Machine / notification senders by domain (CI, VCS, infra).
    NOTIFICATION_DOMAINS = %w[
      github.com circleci.com gitlab.com bitbucket.org atlassian.net sentry.io
      amazonaws.com
    ].freeze

    # Corporate brands that are never a "real person" but span promo / updates /
    # alerts depending on the message. Matched by registrable brand label so every
    # storefront ccTLD (amazon.es, amazon.de, amazon.com.be, amazon.co.uk…) is
    # covered at once. They resolve only AFTER the subject rules have had their
    # say, so a storefront sale still reads as :promotions and a shipping note as
    # :updates — the brand rule just keeps the remainder out of :personal.
    NOTIFICATION_BRANDS = %w[amazon aws].freeze

    # Multi-label public suffixes, so the registrable root of amazon.co.uk is
    # amazon.co.uk (not co.uk) and its brand is amazon (not co).
    COMPOUND_TLDS = %w[
      co.uk com.au com.br com.mx co.jp co.kr co.in com.tr com.sg com.hk co.za
      com.be com.pt com.es com.ar com.co com.tw com.ua co.nz com.my
    ].freeze

    # "no-reply" and its variants, even glued into a longer local-part
    # (aws-no-reply@, noreply-dmarc-support@, account.donotreply@).
    NOREPLY_LOCALPART = /no[._-]?reply|do[._-]?not[._-]?reply|mailer[._-]?daemon/

    PROMO_SUBJECT = /\b\d{1,3}\s?%|\bsale\b|\boff\b|\bdeal\b|\bdesconto|promo|\bnewsletter\b|unsubscribe|gr[aá]tis|\bfree\b/i

    TRANSACTIONAL_SUBJECT = /\border\b|\bshipp|\bdelivery\b|\bencomenda\b|\bpedido\b|\binvoice\b|\bfatura\b|\brecibo\b|\breceipt\b|\btracking\b|a\scaminho/i

    VCS_SUBJECT = %r{\bPR\s?#\d+|\[CircleCI\]|workflow\s+(failed|canceled|succeeded)|\bRe:\s*\[[^\]]+/[^\]]+\]}i

    SECURITY_SUBJECT = /verification\s+code|security\s+alert|\b2fa\b|one-?time|c[oó]digo|password\s+reset|sign-?in\s+(code|link)|new\s+(login|device|sign)/i

    def initialize(email)
      @email = email
    end

    def call
      # Once L0 ingest capture lands, provider-supplied signals win outright.
      header = header_category
      return header if header

      return result(:social, 0.9, [ "social sender: #{from_domain}" ]) if SOCIAL_DOMAINS.include?(from_domain)

      if NOTIFICATION_DOMAINS.include?(from_domain) || subject.match?(VCS_SUBJECT)
        return result(:notifications, 0.92, [ "machine / VCS notification" ])
      end

      if machine_sender?
        # An unattended sender can still carry something the user must see.
        return result(:important, 0.7, [ "automated sender, security-flavoured subject" ]) if subject.match?(SECURITY_SUBJECT)
        return result(:notifications, 0.85, [ "automated sender: #{from_localpart}@" ])
      end

      return result(:promotions, 0.8, [ "marketing subject" ]) if subject.match?(PROMO_SUBJECT)
      return result(:updates, 0.7, [ "transactional subject" ]) if subject.match?(TRANSACTIONAL_SUBJECT)
      return result(:promotions, 0.65, [ "bulk sender: #{from_localpart}@" ]) if bulk_sender?

      return result(:important, 0.6, [ "security-flavoured subject" ]) if subject.match?(SECURITY_SUBJECT)

      # A known corporate brand (Amazon / AWS) is never a real person. Once the
      # subject rules above have passed on it, file the remainder as a machine
      # notification rather than letting it slip through to :personal.
      return result(:notifications, 0.6, [ "corporate sender: #{from_brand}" ]) if NOTIFICATION_BRANDS.include?(from_brand)

      # No machine / bulk / brand / security signal — treat as a human message and
      # hand it to the cheap embedding rung (not the LLM). NB: contact_id is NOT a
      # useful importance signal — the app assigns a contact to virtually every
      # email; a real "VIP" signal should use the contact's relationship_type.
      result(:personal, 0.4, [ "no machine / bulk / security signal" ])
    end

    private

    attr_reader :email

    # Hook for provider-supplied ML verdicts (e.g. Gmail category labels) that
    # should win outright. The RFC bulk/automated headers we now persist
    # (List-Unsubscribe / Precedence / Auto-Submitted) are consumed lower down via
    # #bulk_headers? / #auto_submitted? instead — they say "not a person" without
    # pinning an exact bucket, so they strengthen the existing rules rather than
    # short-circuiting them.
    def header_category = nil

    def result(category, confidence, reasons)
      Result.new(category: category, confidence: confidence, reasons: reasons)
    end

    def subject = email.subject.to_s

    def from_address = email.from_address.to_s.downcase

    # The bare email address even when the header is "Display Name <addr>" — so the
    # localpart/domain rules work on real mail (where From almost always carries a
    # display name). Falls back to the raw value for already-bare addresses.
    def from_email
      from_address[/<([^>]+)>/, 1] || from_address[/[^\s<]+@[^\s>]+/] || from_address
    end

    def from_localpart = from_email[/\A[^@]+/].to_s.split("+").first.to_s

    # The local-part's dash/dot/underscore-separated words, so a compound sender
    # is matched by any of its parts (aws-noreply → "noreply"; store-news →
    # "store"/"news") instead of only as an exact whole-string match.
    def localpart_words = from_localpart.split(/[._+-]/).reject(&:empty?)

    # An unattended / automated mailbox: a whole machine local-part (no-reply@),
    # one of its words (aws-noreply@), or a no-reply phrase glued in anywhere
    # (noreply-dmarc-support@).
    def machine_sender?
      auto_submitted? ||
        MACHINE_LOCALPARTS.include?(from_localpart) ||
        (localpart_words & MACHINE_LOCALPARTS).any? ||
        from_localpart.match?(NOREPLY_LOCALPART)
    end

    # A marketing / newsletter sender, by header signal, whole local-part (store@),
    # or any of its words (store-news@ → "store"/"news").
    def bulk_sender?
      bulk_headers? ||
        BULK_LOCALPARTS.include?(from_localpart) ||
        (localpart_words & BULK_LOCALPARTS).any?
    end

    # --- bulk / automated signals captured from headers at ingest -------------
    # Empty on legacy mail and on providers that don't surface a given header, so
    # they simply contribute nothing there and the local-part rules still apply.

    # List-Unsubscribe (RFC 2369) is the canonical "this is a mailing list" marker;
    # Precedence: bulk/list/junk (RFC 2076) says the same. Either means list
    # traffic — never a 1:1 human.
    def bulk_headers? = list_unsubscribe? || precedence_bulk?

    def list_unsubscribe? = !header(:header_list_unsubscribe).empty?

    def precedence_bulk? = %w[bulk list junk].include?(header(:header_precedence).downcase)

    # Auto-Submitted (RFC 3834): "no" is a human; anything else is machine-generated.
    def auto_submitted?
      value = header(:header_auto_submitted).downcase
      !value.empty? && value != "no"
    end

    def header(name)
      (email.respond_to?(name) ? email.public_send(name) : nil).to_s.strip
    end

    def domain_labels = from_email[/@([^>\s]+)/, 1].to_s.split(".")

    # The registrable domain, matching sub-domained senders by their root
    # (notifications.github.com → github.com) and honouring multi-label TLDs
    # (amazon.co.uk → amazon.co.uk, not co.uk).
    def from_domain
      labels = domain_labels
      take = COMPOUND_TLDS.include?(labels.last(2).join(".")) ? 3 : 2
      labels.length > take ? labels.last(take).join(".") : labels.join(".")
    end

    # The brand label left of the public suffix (amazon.co.uk → "amazon",
    # costalerts.amazonaws.com → "amazonaws").
    def from_brand
      labels = domain_labels
      suffix = COMPOUND_TLDS.include?(labels.last(2).join(".")) ? 2 : 1
      labels[-(suffix + 1)].to_s
    end
  end
end
