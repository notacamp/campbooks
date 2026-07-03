# Contacts — a first-class people directory and per-contact profile pages,
# promoted out of the inbox-settings modal into the primary navigation. This
# controller also serves the app-wide hover card (#popover), the compose
# autocomplete (#search), and an email→profile redirect (#lookup) used by avatar
# taps on touch devices, where there is no hover popover.
#
# A Contact is one email address Scout has seen; a Person is the de-duplicated
# human (one Person ↔ many Contacts). The index lists People; a profile page is
# keyed by a Contact, matching the popover's "View profile" link.
class ContactsController < ApplicationController
  before_action :set_contact, only: [ :show, :update, :analyze, :resolve_duplicate, :set_state ]

  def index
    # Self-heal: enqueue analysis for any contact with enough history that was never
    # analyzed (e.g. mail ingested before a text-AI provider was configured), so the
    # directory — and Person#organization behind it — fills in over repeat visits.
    Contacts::PendingAnalysisCatchUp.run(Current.workspace)

    @searching = params[:q].present?
    contacts = Current.workspace.contacts

    base = Current.workspace.people.joins(:contacts).group("people.id")
    if @searching
      like = "%#{params[:q].to_s.strip}%"
      base = base.where("people.name ILIKE :q OR contacts.email ILIKE :q OR contacts.name ILIKE :q", q: like)
    else
      # Starred and blocked senders get their own sections, so keep them out of
      # the main browse list (a person is sectioned out by any starred/blocked
      # address).
      sectioned_ids = contacts.where("starred_at IS NOT NULL OR list_status = ?", Contact.list_statuses[:blocked])
                              .distinct.pluck(:person_id).compact
      base = base.where.not("people.id" => sectioned_ids) if sectioned_ids.any?
    end

    @pagy, @people = pagy(
      base.select("people.*", "MAX(contacts.last_email_at) AS contact_last_email_at")
          .order("MAX(contacts.last_email_at) DESC NULLS LAST"),
      items: 30
    )
    person_ids = @people.map(&:id)
    @people_with_contacts = Current.workspace.people.where(id: person_ids)
                                   .includes(:contacts, contacts: :contact_email_aliases)
                                   .sort_by { |p| person_ids.index(p.id) }

    # Pinned sections + the contact-skim CTA (skipped while searching).
    @starred_contacts = contacts.starred.includes(:person).by_last_email.limit(100)
    @blocked_contacts = contacts.blocked.includes(:person).by_last_email.limit(100)
    @blocked_count    = contacts.blocked.count
    @pending_count    = contacts.pending.count
    @show_blocked     = params[:show_blocked] == "1"
    @flagged = flagged_duplicates

    respond_to do |format|
      format.html
      format.turbo_stream # pagination append -> index.turbo_stream.erb
    end
  end

  def show
    @person = @contact.person
    @email_messages = @contact.email_messages.includes(:email_account, :tags).order(received_at: :desc).limit(25)
    @documents = @contact.related_documents.includes(:classification).order(created_at: :desc).limit(8)
  end

  def update
    person = @contact.person || Person.create!(workspace: Current.workspace)
    @contact.update!(person: person) unless @contact.person_id == person.id

    if person.update(person_params)
      @contact.update_columns(name: person.name, relationship_type: person.relationship_type)
      @person = person
      # -> update.turbo_stream.erb
    else
      render turbo_stream: notify_stream(person.errors.full_messages.to_sentence, severity: :error)
    end
  end

  def analyze
    return if require_ai_provider!(:text)

    ContactAnalysisJob.perform_later(@contact.id, force: true, prompt: params[:prompt])
    # -> analyze.turbo_stream.erb
  end

  def resolve_duplicate
    if params[:approve] == "true"
      @contact.update!(person: @contact.suggested_person, suggested_person_id: nil, suggested_reason: nil, suggested_confidence: nil)
      @message = t(".merged", name: @contact.person.display_name)
    else
      @contact.update!(suggested_person_id: nil, suggested_reason: nil, suggested_confidence: nil)
      @message = t(".dismissed")
    end
    @flagged = flagged_duplicates
    render :dedup_stream
  end

  def scan_dedup
    return if require_ai_provider!(:text)

    matches = Contacts::AiDeduplicator.new.scan!
    @message = t(".complete", count: matches.length)
    @flagged = flagged_duplicates
    render :dedup_stream
  end

  def consolidate_all
    merged = 0
    Current.workspace.people.where.not(analyzed_at: nil).find_each do |person|
      merged += 1 if Contacts::Consolidator.consolidate!(person)
    end
    @message = t(".complete", count: merged)
    @flagged = flagged_duplicates
    render :dedup_stream
  end

  # App-wide hover card. Served at /contacts/:id/popover and /contacts/popover.
  def popover
    @contact =
      if params[:id].present?
        Current.workspace.contacts.find_by(id: params[:id])
      elsif params[:email].present?
        Current.workspace.contacts.find_by(email: params[:email])
      end
    return head :not_found if @contact.nil?

    @person = @contact.person
    render layout: false
  end

  # Email → profile redirect for avatar taps on touch devices (no hover popover).
  # Falls back to the index when the address isn't a known contact.
  def lookup
    contact = Current.workspace.contacts.find_by(email: params[:email].to_s.strip)
    redirect_to(contact ? contact_path(contact) : contacts_path)
  end

  # JSON autocomplete for compose (to/cc/bcc) — backs Campbooks::ContactPillInput.
  # Plain ILIKE (not searchkick) so it works even when OpenSearch is unavailable.
  def search
    q = params[:q].to_s.strip
    scope = Current.workspace.contacts
    if q.present?
      like = "%#{q}%"
      scope = scope.where("email ILIKE :q OR name ILIKE :q", q: like)
    end
    results = scope.order(email_count: :desc).limit(8).map do |c|
      { email: c.email, name: c.name, display_name: c.display_name }
    end
    render json: results
  end

  # Sender-list state toggles driven directly from the contacts page — the
  # profile header, index row quick-actions, and the contact skim. Unlike the
  # EmailActions sender tools (which resolve a Contact from an email_message),
  # these act on the Contact record directly. Block/unblock route through
  # Contacts::Block/Unblock so the inbox auto-archives (and an undo restores it).
  def set_state
    case params[:state]
    when "star"    then @contact.star!
    when "unstar"  then @contact.unstar!
    when "allow"   then @contact.allow!
    when "block"   then Contacts::Block.call(@contact, user: Current.user)
    when "unblock" then Contacts::Unblock.call(@contact, user: Current.user)
    else return head(:unprocessable_entity)
    end

    respond_to do |format|
      format.turbo_stream # -> set_state.turbo_stream.erb (streams no-op when off-page)
      format.json { head :ok } # contact skim advances client-side
      format.html { redirect_back fallback_location: contact_path(@contact) }
    end
  end

  # Tinder-style triage of "new" senders: undecided (pending) contacts, most
  # recently active first. Approve (allow) / block from the deck POST to
  # #set_state. Useful in both blocklist and whitelist mode.
  def skim
    @contacts = Current.workspace.contacts.pending
                       .includes(:person)
                       .by_last_email
                       .limit(30)
    @pending_total = Current.workspace.contacts.pending.count
  end

  private

  def flagged_duplicates
    Current.workspace.contacts.flagged_as_duplicate.includes(:suggested_person).order(suggested_confidence: :desc)
  end

  def set_contact
    @contact = Current.workspace.contacts.find(params[:id])
  end

  def person_params
    params.require(:person).permit(:name, :relationship_type)
  end
end
