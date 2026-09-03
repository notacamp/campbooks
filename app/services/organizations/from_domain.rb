# frozen_string_literal: true

module Organizations
  # Links a SERVICE contact to an organization by its email domain — the Rethink
  # "services belong to organizations" rule. A service (billing@cloudhost.example)
  # rarely carries the AI-analyzed Person#organization string that
  # Organizations::Backfill relies on for people, so we derive the org from the
  # registrable domain instead: find-or-create Organization(domain:) and give the
  # service's Person a membership.
  #
  # Deliberately narrow so it never invents an org from a personal mailbox:
  #   * free-mail / webmail domains are skipped (gmail, outlook, proton, sapo…);
  #   * the workspace's own account domains are skipped (your own mail isn't a vendor);
  #   * a contact whose Person already belongs to an org is left alone.
  # Idempotent and fail-safe: any error is swallowed (it rides the mail pipeline).
  # Persons keep today's name-based linkage — this does NOT touch Organizations::Backfill.
  module FromDomain
    # Registrable domains that are personal mailboxes, never a vendor/brand.
    WEBMAIL_DOMAINS = %w[
      gmail.com googlemail.com outlook.com hotmail.com live.com msn.com
      yahoo.com ymail.com rocketmail.com icloud.com me.com mac.com
      proton.me protonmail.com pm.me aol.com gmx.com gmx.net mail.com
      zoho.com yandex.com fastmail.com hey.com tutanota.com tuta.io
      sapo.pt clix.pt mail.pt
    ].freeze

    class << self
      # Link a service contact to its domain's organization. Returns the
      # OrganizationMembership (existing or created), or nil when skipped.
      def link(contact)
        return nil unless contact
        return nil unless contact.kind_service?

        person = contact.person
        return nil if person.nil?
        return nil if person.organization_memberships.exists?

        domain = registrable_domain(contact.email)
        return nil if domain.blank?
        return nil if WEBMAIL_DOMAINS.include?(domain)
        return nil if own_account_domain?(contact.workspace, domain)

        org = find_or_create_org(contact.workspace, domain)
        OrganizationMembership.find_or_create_by!(person: person, organization: org) { |m| m.status = :active }
      rescue ActiveRecord::RecordNotUnique, ActiveRecord::RecordInvalid
        retry_link(contact)
      rescue StandardError => e
        Rails.logger.warn("[Organizations::FromDomain] #{contact&.id}: #{e.class}: #{e.message}")
        nil
      end

      # cloudhost.example → "cloudhost.example"; billing.cloudhost.example →
      # "cloudhost.example"; amazon.co.uk → "amazon.co.uk".
      def registrable_domain(email)
        labels = domain_labels(email)
        return nil if labels.size < 2

        take = Emails::Categorizer::COMPOUND_TLDS.include?(labels.last(2).join(".")) ? 3 : 2
        labels.last([ take, labels.size ].min).join(".")
      end

      private

      # Accepts a full address ("rui@cloudhost.example") or a bare host
      # ("billing.cloudhost.example") — the part after the last "@", else the value.
      def domain_labels(value)
        raw = value.to_s.downcase.strip
        host = raw.include?("@") ? raw.split("@").last.to_s : raw
        host.sub(/[>\s].*/, "").split(".").reject(&:empty?)
      end

      # "Cloudhost" from cloudhost.example — the brand label left of the public suffix.
      def brand_name(domain)
        labels = domain.split(".")
        suffix = Emails::Categorizer::COMPOUND_TLDS.include?(labels.last(2).join(".")) ? 2 : 1
        labels[-(suffix + 1)].to_s.tr("-_", "  ").split.map(&:capitalize).join(" ").presence || domain
      end

      def own_account_domain?(workspace, domain)
        workspace.email_accounts.pluck(:email_address).any? do |addr|
          registrable_domain(addr) == domain
        end
      end

      # An org already claiming this domain wins; otherwise create one named after
      # the brand. A pre-existing same-name org (from the people backfill) adopts
      # the domain rather than colliding on the unique name index.
      def find_or_create_org(workspace, domain)
        existing = workspace.organizations.where(domain: domain).first
        return existing if existing

        name = brand_name(domain)
        by_name = workspace.organizations.find_by(name: name)
        if by_name
          by_name.update!(domain: domain) if by_name.domain.blank?
          return by_name
        end

        workspace.organizations.create!(name: name, domain: domain)
      end

      def retry_link(contact)
        org = contact.workspace.organizations.where(domain: registrable_domain(contact.email)).first
        return nil unless org

        OrganizationMembership.find_or_create_by!(person: contact.person, organization: org) { |m| m.status = :active }
      rescue StandardError
        nil
      end
    end
  end
end
