module Tools
  class QueryContacts
    def self.call(args = {})
      limit = [ args["limit"].to_i, 50 ].min
      limit = 20 if limit <= 0

      # Contacts are workspace-scoped; fail closed if no workspace is established.
      return { count: 0, contacts: [], search_method: "none" } unless Current.workspace

      # Determine if any search text is provided across multiple filter fields
      search_text = args.values_at("search_text", "name", "organization").compact.join(" ").strip

      # Try semantic search if search text is present and embeddings exist
      if search_text.present? && SearchRecord.by_type("Contact").exists?
        filters = build_filters(args)
        results = SearchService.search(
          search_text,
          workspace: Current.workspace,
          filters: filters.merge(searchable_type: "Contact"),
          options: { limit: limit }
        )

        if results.any?
          contact_ids = results.map(&:searchable_id)
          contacts = Current.workspace.contacts.where(id: contact_ids)
                            .order(last_email_at: :desc)
                            .limit(limit)
                            .map { |c| format_contact(c) }

          return { count: contacts.size, contacts: contacts, search_method: "semantic" }
        end
      end

      # Fallback to ILIKE search
      scope = Current.workspace.contacts
      scope = apply_filters(scope, args)

      total = scope.count
      contacts = scope.order(last_email_at: :desc).limit(limit).map { |c| format_contact(c) }

      { count: total, contacts: contacts, search_method: "text" }
    end

    def self.apply_filters(scope, args)
      scope = scope.where("name ILIKE ?", "%#{args["name"]}%") if args["name"].present?
      scope = scope.where("email ILIKE ?", "%#{args["email"]}%") if args["email"].present?
      scope = scope.where("organization ILIKE ?", "%#{args["organization"]}%") if args["organization"].present?
      scope = scope.where(relationship_type: args["relationship_type"]) if args["relationship_type"].present?
      scope = scope.where("email_count >= ?", args["has_email_count_min"].to_i) if args["has_email_count_min"].present?

      if args["last_email_before"].present?
        date = Date.parse(args["last_email_before"]) rescue nil
        scope = scope.where("last_email_at < ?", date) if date
      end

      if args["last_email_after"].present?
        date = Date.parse(args["last_email_after"]) rescue nil
        scope = scope.where("last_email_at >= ?", date) if date
      end

      scope
    end

    def self.build_filters(args)
      filters = {}
      filters[:email] = args["email"] if args["email"].present?
      filters[:relationship_type] = args["relationship_type"] if args["relationship_type"].present?
      filters
    end

    def self.format_contact(c)
      {
        id: c.id,
        name: c.name,
        email: c.email,
        workspace: c.organization,
        relationship_type: c.relationship_type,
        email_count: c.email_count,
        last_email_at: c.last_email_at&.iso8601
      }
    end
  end
end
