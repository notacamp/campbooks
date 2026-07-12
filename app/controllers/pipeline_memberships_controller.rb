# frozen_string_literal: true

# Adds documents/emails to a pipeline (and removes them) — the board's entry
# point. The picker (#new) lists items not yet in the pipeline that match the
# pipeline's applies_to and that the current user may access.
class PipelineMembershipsController < ApplicationController
  include PipelineColumns

  before_action -> { require_entitlement!(:pipelines, ignore_limit: true) }
  before_action :set_pipeline

  PICKER_LIMIT = 25

  def new
    @query = params[:q].to_s.strip
    @items = addable_items(@query)
  end

  def create
    item = resolve_item(params[:item_type], params[:item_id])
    raise ActiveRecord::RecordNotFound unless item

    @membership = item.assign_to_pipeline!(@pipeline, user: Current.user)
    @item = item
    @columns = build_columns(@pipeline)
    respond_to do |format|
      format.turbo_stream
      format.json { render json: { ok: true } }
    end
  rescue ActiveRecord::RecordNotFound
    respond_to do |format|
      format.turbo_stream { head :not_found }
      format.json { render json: { ok: false }, status: :not_found }
    end
  end

  def destroy
    membership = @pipeline.memberships.find(params[:id])
    raise ActiveRecord::RecordNotFound unless accessible_item?(membership.item)

    membership.destroy!
    @columns = build_columns(@pipeline)
    respond_to do |format|
      format.turbo_stream
      format.json { render json: { ok: true } }
    end
  rescue ActiveRecord::RecordNotFound
    respond_to do |format|
      format.turbo_stream { head :not_found }
      format.json { render json: { ok: false }, status: :not_found }
    end
  end

  private

  def set_pipeline
    @pipeline = Current.workspace.pipelines.find(params[:pipeline_id])
  end

  # The item must be of a type the pipeline accepts AND visible to the user.
  def resolve_item(type, id)
    case type
    when "Document"
      return nil unless @pipeline.documents? || @pipeline.both?

      Current.workspace.documents.find_by(id: id)
    when "EmailMessage"
      return nil unless @pipeline.emails? || @pipeline.both?

      EmailMessage.accessible_to(Current.user).find_by(id: id)
    end
  end

  def addable_items(query)
    items = []
    items.concat(addable_documents(query)) if @pipeline.documents? || @pipeline.both?
    items.concat(addable_emails(query)) if @pipeline.emails? || @pipeline.both?
    items
  end

  def addable_documents(query)
    scope = Current.workspace.documents
      .where.not(id: assigned_item_ids("Document"))
      .order(created_at: :desc)
    if query.present?
      like = "%#{query}%"
      scope = scope.where(
        "documents.canonical_filename ILIKE :q OR documents.metadata->>'vendor_name' ILIKE :q " \
        "OR documents.metadata->>'client_name' ILIKE :q OR documents.description ILIKE :q",
        q: like
      )
    end
    scope.limit(PICKER_LIMIT).to_a
  end

  def addable_emails(query)
    scope = EmailMessage.accessible_to(Current.user)
      .where.not(id: assigned_item_ids("EmailMessage"))
      .order(received_at: :desc)
    scope = scope.where("subject ILIKE :q OR from_address ILIKE :q", q: "%#{query}%") if query.present?
    scope.limit(PICKER_LIMIT).to_a
  end

  def assigned_item_ids(type)
    @pipeline.memberships.where(item_type: type).select(:item_id)
  end
end
