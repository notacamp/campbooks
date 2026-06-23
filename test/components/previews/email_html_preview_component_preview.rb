# frozen_string_literal: true

class EmailHtmlPreviewComponentPreview < ViewComponent::Preview
  # A long reply with formatting (heading, list, link, bold) followed by a quoted
  # blockquote + "From:/Subject:" attribution — the quote is stripped, and the
  # visible reply overflows the cap so "Read more" appears.
  LONG = <<~HTML
    <div style="font-family: Verdana, sans-serif">
      <p>Hi Jamie,</p>
      <p>Thanks for the detailed update on the <b>venue contract</b>. A few things stand out:</p>
      <ul>
        <li>The deposit is due <b>two weeks earlier</b> than we discussed.</li>
        <li>The cancellation window shrank from 30 days to 14.</li>
        <li>They added a separate cleaning fee that was not in the original quote.</li>
      </ul>
      <p>None of these are dealbreakers on their own, but together they shift the
      total by a little over twelve percent, so it is worth a quick call before you
      sign. I would also double-check the insurance clause on page four — see the
      <a href="https://example.com/contract">contract draft</a> for the exact wording.</p>
      <p>Everything else looks standard and matches what they sent in May.</p>
      <p>Best,<br>Alex</p>
      <div>From: jamie@example.com<br>Date: Fri, 19 Jun 2026 14:47:45 +0100<br>Subject: Re: Venue contract</div>
      <blockquote id="x_blockquote_zmail" style="margin:0">
        <p>Hi Alex, please find the updated contract attached. Let me know your
        thoughts on the revised terms — happy to jump on a call this week.</p>
        <p>Thanks,<br>Jamie</p>
      </blockquote>
    </div>
  HTML

  # A short HTML reply: no quote, fits within the cap, so no toggle appears.
  SHORT = "<p>Sounds good — see you at <b>3pm</b> on Thursday. 🎟️</p>"

  def default
    render(Campbooks::EmailHtmlPreview.new(message: build(LONG), class: "max-w-lg"))
  end

  def short
    render(Campbooks::EmailHtmlPreview.new(message: build(SHORT), class: "max-w-lg"))
  end

  private

  def build(body)
    EmailMessage.new(body: body)
  end
end
