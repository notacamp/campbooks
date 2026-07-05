# frozen_string_literal: true

# CRUD for ScheduledDigest (model_name = "Digest"). Scoped to the current user
# (digests are personal). Missing or foreign-workspace digest IDs 404 per the app
# convention (invisible resources are never 403).
class DigestsController < ApplicationController
  include ActionView::RecordIdentifier

  before_action :require_authentication
  before_action :require_digests_enabled
  before_action :set_digest, only: %i[show edit update destroy run_now]

  def index
    @digests = current_user.scheduled_digests.order(created_at: :asc)
  end

  def new
    if params[:preset].present?
      preset = Digests::Presets.find(params[:preset])
      return head(:not_found) unless preset

      @preset = preset
      @digest = ScheduledDigest.new(
        name: t("digests.presets.#{preset.key}.label"),
        rrule: preset.rrule,
        config: { "sources" => preset.sources }
      )
    end
    # Without preset param: renders the gallery (no @digest set).
  end

  def create
    return if require_entitlement!(:digests)

    @digest = current_user.scheduled_digests.new(assembled_digest_params)
    @digest.workspace = Current.workspace

    first_run = parsed_first_run_at
    @digest.next_run_at = first_run unless first_run == :invalid

    if first_run != :invalid && @digest.save
      redirect_to @digest, success: t(".created")
    else
      @digest.errors.add(:next_run_at, :invalid) if first_run == :invalid
      @preset = Digests::Presets.find(params[:digest][:preset_key].to_s)
      render :new, status: :unprocessable_entity
    end
  end

  def show
    @issues = @digest.issues.order(created_at: :desc).limit(20)
    @latest_issue = @issues.first
  end

  def edit
  end

  def update
    return if require_entitlement!(:digests)

    attrs = assembled_digest_params
    first_run = parsed_first_run_at
    if first_run == :invalid
      @digest.errors.add(:next_run_at, :invalid)
      return render :edit, status: :unprocessable_entity
    end
    attrs[:next_run_at] = first_run if first_run

    if @digest.update(attrs)
      respond_to do |format|
        format.turbo_stream do
          # Re-render the row so the toggle/badge reflect the new state.
          render turbo_stream: [
            turbo_stream.replace(
              dom_id(@digest),
              render_to_string(Campbooks::Digests::DigestRow.new(digest: @digest), layout: false)
            ),
            notify_stream(t(".updated"))
          ]
        end
        format.html { redirect_to @digest, success: t(".updated") }
      end
    else
      render :edit, status: :unprocessable_entity
    end
  end

  # The Stimulus schedule picker writes an ISO-8601 datetime into the hidden
  # digest[first_run_at]. nil = not submitted; :invalid = unparseable.
  def parsed_first_run_at
    raw = params.dig(:digest, :first_run_at)
    return nil if raw.blank?

    Time.iso8601(raw)
  rescue ArgumentError
    :invalid
  end

  def destroy
    @digest.destroy
    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: [
          turbo_stream.remove(dom_id(@digest)),
          notify_stream(t(".deleted"))
        ]
      end
      format.html { redirect_to digests_path, success: t(".deleted") }
    end
  end

  # Enqueue a manual run for this digest and notify immediately.
  def run_now
    DigestRunJob.perform_later(@digest.id, Time.current.iso8601, manual: true)
    respond_to do |format|
      format.turbo_stream { render turbo_stream: notify_stream(t(".generating")) }
      format.html { redirect_to @digest, success: t(".generating") }
    end
  end

  private

  def set_digest
    @digest = current_user.scheduled_digests.find(params[:id])
  rescue ActiveRecord::RecordNotFound
    head :not_found
  end

  BOOLEAN = ActiveModel::Type::Boolean.new

  # Build the config jsonb from per-source form params. NORMALIZES types
  # (window_days -> Integer, include_overdue -> boolean, document_types -> array
  # of strings). Only checked sources are included in the sources array.
  #
  # Partial submits (the row's inline enabled toggle) carry neither the
  # sources_submitted marker nor the delivery/AI keys — those digests keep their
  # existing config and flags untouched.
  def assembled_digest_params
    p = raw_digest_params
    # Full-form submits carry the marker (and the source checkboxes); the row's
    # inline enabled toggle carries neither, so its digest keeps its config.
    sources_submitted = BOOLEAN.cast(p.delete(:sources_submitted)) ||
      p.keys.any? { |k| k.to_s.start_with?("source_") }

    sources = []

    if checked?(p.delete(:"source_emails"))
      sources << { "type" => "emails", "query" => p.delete(:emails_query).to_s.strip }
    else
      p.delete(:emails_query)
    end

    if checked?(p.delete(:"source_calendar"))
      sources << { "type" => "calendar", "window_days" => window_from(p.delete(:calendar_window_days)) }
    else
      p.delete(:calendar_window_days)
    end

    if checked?(p.delete(:"source_tasks"))
      window  = window_from(p.delete(:tasks_window_days))
      overdue = checked?(p.delete(:tasks_include_overdue))
      sources << { "type" => "tasks", "window_days" => window, "include_overdue" => overdue }
    else
      p.delete(:tasks_window_days)
      p.delete(:tasks_include_overdue)
    end

    if checked?(p.delete(:"source_reminders"))
      sources << { "type" => "reminders", "window_days" => window_from(p.delete(:reminders_window_days)) }
    else
      p.delete(:reminders_window_days)
    end

    if checked?(p.delete(:"source_documents"))
      types = Array(p.delete(:document_types)).reject(&:blank?).map(&:downcase)
      sources << { "type" => "documents", "document_types" => types }
    else
      p.delete(:document_types)
    end

    # Cast boolean columns only when submitted (hidden "0" companions in the
    # form make every unchecked toggle explicit, so absence means "not on the
    # submitted form", never "unchecked").
    %i[ai_enabled deliver_by_email show_in_feed enabled].each do |key|
      p[key] = checked?(p[key]) if p.key?(key)
    end

    p[:config] = { "sources" => sources } if sources_submitted

    p.to_h
  end

  def checked?(value)
    !!BOOLEAN.cast(value)
  end

  def window_from(value)
    window = value.to_i
    [ 7, 14, 30 ].include?(window) ? window : 7
  end

  def raw_digest_params
    params.require(:digest).permit(
      :name, :preset_key, :rrule, :ai_enabled, :ai_instructions,
      :deliver_by_email, :show_in_feed, :enabled, :sources_submitted,
      :source_emails, :emails_query,
      :source_calendar, :calendar_window_days,
      :source_tasks, :tasks_window_days, :tasks_include_overdue,
      :source_reminders, :reminders_window_days,
      :source_documents,
      document_types: []
    )
  end
end
