# frozen_string_literal: true

module Accounting
  # The drawer ships closed (invisible) and is opened by the document-preview
  # Stimulus controller when a document link inside a previewable region is
  # clicked. This preview renders it with the panel forced visible.
  class DocumentPreviewDrawerPreview < Lookbook::Preview
    def default
      render Campbooks::Accounting::DocumentPreviewDrawer.new
    end
  end
end
