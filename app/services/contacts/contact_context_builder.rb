module Contacts
  class ContactContextBuilder
    def initialize(from_address)
      @contact = find_contact(from_address)
      @person = @contact&.person
    end

    def context_for_prompt
      return nil unless @person&.context_summary.present?

      parts = [ "This sender (#{@contact.email}) is #{@person.context_summary}" ]
      parts << "They represent #{@person.organization}." if @person.organization.present?
      parts << "Relationship: #{@person.relationship_type}." if @person.relationship_type.present?

      "<contact_context>\n#{parts.join(" ")}\n</contact_context>"
    end

    private

    def find_contact(email_address)
      Contact.find_by(email: email_address) ||
        ContactEmailAlias.includes(:contact).find_by(email: email_address)&.contact
    end
  end
end
