# Tasks

## 1. Sending

- [x] 1.1 `send_draft` and `cancel_send`: no longer `.assistant()`
- [x] 1.2 Both handlers refuse an invoked request (403 `send_needs_you`) after `drafts::load`, before the 409s, with a WARN of ids and caller only
- [x] 1.3 `errors.send_needs_you` in the conversations catalogue, all four locales
- [x] 1.4 OpenAPI: the two operations' flags and 403 descriptions; `schema.gen.ts` descriptions

## 2. The confirmed flag

- [x] 2.1 `cargo xtask arch`: a high-risk operation may not opt the assistant or automation in, with a unit test
- [x] 2.2 Docs of `Risk` and `Invocation::confirmed` say the caller sets it and it proves no click

## 3. Trusted images (item 18)

- [x] 3.1 An invoked `create_rule` showing images, `update_rule` reshaping a rule into or out of showing images, and `restore_rule` of one answer 403 `images_need_you`
- [x] 3.2 `errors.images_need_you`, all four locales; the three 403 descriptions

## 4. Reply dialog (item 17)

- [x] 4.1 The replaced draft is discarded while it is `draft` or `failed`

## 5. Tests

- [x] 5.1 The assistant writes a draft and reads its token, but its `send_draft` is refused, confirmed or not; listed by mistake, the handler still refuses a confirmed call; nothing is scheduled, bound or requested; the member's own Send then leaves once
- [x] 5.2 The assistant's `cancel_send` is refused (kernel, and the handler when listed); the send still leaves
- [x] 5.3 Server end to end: Scout drafts, its send and cancel are refused with the member's token, the draft stays unsigned and no send is requested; the member's Send and Undo work
- [x] 5.4 Rate limits: invoked sends are refused, not counted; the person's own sends keep their budget
- [x] 5.5 Item 18: the assistant's image rule (proposed or active), its restore and its reshape into one are refused; the member's own image rule is saved
- [x] 5.6 ReplyDialog discards a replaced `failed` draft
