class EmptyStateComponentPreview < ViewComponent::Preview
  def card
    render Campbooks::EmptyState.new(
      variant: :card,
      title: "No documents found",
      description: "Upload a document or connect an email account to get started."
    )
  end

  def standalone
    render Campbooks::EmptyState.new(
      variant: :standalone,
      title: "Connect your email",
      description: "Link an email account to start building your contact list automatically from incoming emails."
    ) do |state|
      state.with_icon(svg: '<svg class="w-7 h-7 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0z"/></svg>')
    end
  end

  def inline
    render Campbooks::EmptyState.new(
      variant: :inline,
      title: "No items match your filter."
    )
  end

  def with_icon_and_actions
    render Campbooks::EmptyState.new(
      variant: :card,
      title: "No documents found",
      description: "Upload a document or connect an email account."
    ) do |state|
      state.with_icon(svg: '<svg class="mx-auto w-12 h-12 text-gray-300" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/></svg>')
      state.with_actions(html: '<a href="#" class="inline-flex items-center px-4 py-2 bg-accent-600 text-white text-sm font-medium rounded-lg hover:bg-accent-700 transition-colors">Upload document</a><a href="#" class="inline-flex items-center px-4 py-2 border border-gray-300 text-sm font-medium rounded-lg text-gray-700 hover:bg-gray-50 transition-colors">Connect email</a>')
    end
  end

  def standalone_with_actions
    render Campbooks::EmptyState.new(
      variant: :standalone,
      title: "Connect your email",
      description: "Link an email account to automatically build your contact list."
    ) do |state|
      state.with_icon(svg: '<svg class="w-7 h-7 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"/></svg>')
      state.with_actions(html: '<a href="#" class="inline-flex items-center gap-2 px-4 py-2 bg-accent-600 text-white rounded-lg text-sm font-medium hover:bg-accent-700 shadow-sm transition-colors">Add email account</a>')
    end
  end
end
