module Contacts
  class Identifier
    FIRST_ANALYSIS_THRESHOLD = 5
    REANALYSIS_INTERVAL = 20

    # Resolve (find-or-create + link) the Contact for a message without the
    # analysis-threshold bookkeeping #identify! performs. Used by sender actions
    # that need the Contact even for legacy mail that was never linked to one.
    def self.contact_for(email_message)
      return nil if email_message.from_address.blank?

      ident = new(email_message)
      contact = ident.send(:find_or_create_contact, email_message.from_address)
      email_message.update!(contact: contact) if contact && email_message.contact_id != contact.id
      contact
    end

    def initialize(email_message)
      @email = email_message
    end

    def identify!
      from = @email.from_address
      return :none if from.blank?

      contact = find_or_create_contact(from)
      was_new = contact.previously_new_record?
      new_count = associate_and_count(contact)

      if threshold_reached?(new_count, contact.analyzed_at)
        :threshold_reached
      elsif was_new
        :created
      else
        :none
      end
    end

    private

    def find_or_create_contact(email_address)
      alias_record = ContactEmailAlias.includes(:contact).find_by(email: email_address)
      return alias_record.contact if alias_record

      contact = Contact.find_by(email: email_address)

      if contact
        if contact.email_account_id.present? && contact.email_account_id != @email.email_account_id
          contact.promote_to_global!
        end
        return contact
      end

      # Every Contact is one address of a Person (the de-duplicated human the
      # contacts directory lists). Give the new address its own Person up front so
      # it's immediately visible on /contacts — AI dedup merges Persons later. A
      # Person isn't lazily minted only when analysis runs, which left most
      # contacts person-less (and invisible on the directory).
      workspace = @email.email_account.workspace
      Contact.create!(
        email: email_address,
        email_account: @email.email_account,
        workspace: workspace,
        person: Person.create!(workspace: workspace),
        email_count: 0,
        last_email_at: @email.received_at
      )
    rescue ActiveRecord::RecordNotUnique
      Contact.find_by!(email: email_address)
    end

    def associate_and_count(contact)
      @email.update!(contact: contact) unless @email.contact_id == contact.id

      new_count = contact.email_messages.count
      last_email = contact.last_email_at
      new_last = if last_email.nil? || (@email.received_at && @email.received_at > last_email)
                   @email.received_at
      else
                   last_email
      end

      contact.update_columns(
        email_count: new_count,
        last_email_at: new_last
      )

      new_count
    end

    def threshold_reached?(count, analyzed_at)
      return false if count < FIRST_ANALYSIS_THRESHOLD

      if analyzed_at.nil?
        count == FIRST_ANALYSIS_THRESHOLD
      else
        (count - FIRST_ANALYSIS_THRESHOLD) % REANALYSIS_INTERVAL == 0
      end
    end
  end
end
